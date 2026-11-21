//! Sealing, opening and keeping keys. Keys go into memory, or into the `.env` of a temporary vault.

use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use flate2::Compression;
use flate2::write::DeflateEncoder;

use crate::crypto::{self, CryptoError, Key, KeyError};
use crate::crypto::MAX_FILE_BYTES;

fn key(byte: u8) -> Key {
    Key::from_bytes([byte; 32])
}

/// A key whose label differs from `other`'s, so that "a different key" is recognised by its header.
fn different_from(other: &Key) -> Key {
    (1u8..=255).map(key).find(|k| k.id() != other.id()).unwrap()
}

// ---- sealing and opening ----------------------------------------------------------------------------

#[test]
fn a_file_survives_the_round_trip() {
    let key = key(1);
    for plaintext in [&b""[..], b"hello", "naïve — ünïcode ✓".as_bytes(), &[0u8, 159, 146, 150, 255], &vec![42u8; 100_000]] {
        let sealed = crypto::encrypt(&key, "projects/p/readme.md", plaintext).unwrap();
        assert_eq!(crypto::decrypt(&key, "projects/p/readme.md", &sealed).unwrap(), plaintext);
    }
}

#[test]
fn what_is_sealed_looks_nothing_like_the_file_and_is_new_every_time() {
    let key = key(2);
    let text = b"the quick brown fox jumps over the lazy dog, again and again and again";
    let (a, b) = (crypto::encrypt(&key, "a.md", text).unwrap(), crypto::encrypt(&key, "a.md", text).unwrap());
    assert_ne!(a, b, "a fresh nonce for every encryption");
    assert!(!a.windows(text.len().min(12)).any(|w| w == &text[..w.len()]), "no plaintext shows through");
    assert_eq!(&a[..4], &[0x89, b'L', b'D', b'G'], "magic");
    assert_eq!(a[4], 2, "format version");
    assert_eq!(a[5], key.id(), "key id");
}

#[test]
fn files_are_compressed_before_they_are_sealed() {
    let key = key(3);
    let text = "the same line over and over\n".repeat(2_000);
    let sealed = crypto::encrypt(&key, "notes.md", text.as_bytes()).unwrap();
    assert!(sealed.len() < text.len() / 10, "{} bytes sealed from {}", sealed.len(), text.len());
}

#[test]
fn the_wrong_key_is_recognised_and_says_which_key_it_was() {
    let (right, wrong) = (key(4), different_from(&key(4)));
    let sealed = crypto::encrypt(&right, "a.md", b"secret").unwrap();
    assert_eq!(crypto::decrypt(&wrong, "a.md", &sealed), Err(CryptoError::WrongKey { found: right.id(), expected: wrong.id() }));
}

#[test]
fn a_key_with_the_same_label_still_cannot_open_it() {
    // Labels are one byte, so two keys can share one; the cipher itself must still say no.
    let right = key(5);
    let twin = (0u32..100_000)
        .map(|n| {
            let mut bytes = [0u8; 32];
            bytes[..4].copy_from_slice(&n.to_le_bytes());
            Key::from_bytes(bytes)
        })
        .find(|k| k.id() == right.id())
        .expect("some other key shares the label");
    let sealed = crypto::encrypt(&right, "a.md", b"secret").unwrap();
    assert_eq!(crypto::decrypt(&twin, "a.md", &sealed), Err(CryptoError::Failed));
}

#[test]
fn a_tampered_file_is_refused_wherever_it_was_touched() {
    let key = key(6);
    let sealed = crypto::encrypt(&key, "a.md", b"some words worth protecting").unwrap();

    // Every byte after the header: nonce, ciphertext, tag.
    for i in 6..sealed.len() {
        let mut bad = sealed.clone();
        bad[i] ^= 0x01;
        assert_eq!(crypto::decrypt(&key, "a.md", &bad), Err(CryptoError::Failed), "byte {i}");
    }

    let mut newer = sealed.clone();
    newer[4] = 3;
    assert_eq!(crypto::decrypt(&key, "a.md", &newer), Err(CryptoError::UnsupportedVersion(3)));
    assert!(CryptoError::UnsupportedVersion(3).to_string().contains("newer version"));
    // Format 1 was sealed under readable paths; it is not read any more.
    let mut earlier = sealed.clone();
    earlier[4] = 1;
    assert_eq!(crypto::decrypt(&key, "a.md", &earlier), Err(CryptoError::UnsupportedVersion(1)));
    assert!(CryptoError::UnsupportedVersion(1).to_string().contains("earlier version"));

    let mut other_label = sealed.clone();
    other_label[5] ^= 0x01;
    assert!(matches!(crypto::decrypt(&key, "a.md", &other_label), Err(CryptoError::WrongKey { .. })));

    assert_eq!(crypto::decrypt(&key, "a.md", &sealed[..sealed.len() - 1]), Err(CryptoError::Failed), "a clipped tag");
    assert_eq!(crypto::decrypt(&key, "a.md", &sealed[..10]), Err(CryptoError::Corrupt), "cut short");
    assert!(crypto::decrypt(&key, "a.md", &sealed).is_ok(), "the original is untouched");
}

#[test]
fn a_file_moved_to_another_path_no_longer_opens() {
    let key = key(7);
    let sealed = crypto::encrypt(&key, "projects/a/readme.md", b"for a").unwrap();
    assert_eq!(crypto::decrypt(&key, "projects/b/readme.md", &sealed), Err(CryptoError::Failed));
    assert_eq!(crypto::decrypt(&key, "projects/a/readme.md", &sealed).unwrap(), b"for a");
}

#[test]
fn a_path_is_stored_under_a_name_only_the_key_can_make() {
    let name = crypto::remote_name(&key(12), "projects/acquisition-company-x/threads/layoffs-q1/thread.md");
    assert_eq!(name.len(), 32);
    assert!(name.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)), "{name}");
    assert_eq!(name, crypto::remote_name(&key(12), "projects/acquisition-company-x/threads/layoffs-q1/thread.md"), "the same on every machine");
    assert_ne!(name, crypto::remote_name(&key(13), "projects/acquisition-company-x/threads/layoffs-q1/thread.md"), "another key, another name");
    assert_ne!(crypto::remote_name(&key(12), "a.md"), crypto::remote_name(&key(12), "b.md"));
}

#[test]
fn a_plain_file_is_not_mistaken_for_a_sealed_one() {
    let key = key(8);
    assert_eq!(crypto::decrypt(&key, "a.md", b"# just markdown"), Err(CryptoError::NotEncrypted));
    assert_eq!(crypto::decrypt(&key, "a.md", b""), Err(CryptoError::NotEncrypted));
}

#[test]
fn a_decompression_bomb_is_stopped_and_so_is_an_oversized_file() {
    let bomb = {
        let mut packer = DeflateEncoder::new(Vec::new(), Compression::best());
        packer.write_all(&vec![0u8; 1_000_000]).unwrap();
        packer.finish().unwrap()
    };
    assert!(bomb.len() < 5_000, "a megabyte of zeros packs small");
    assert_eq!(crypto::inflate_for_test(&bomb, 999_999), Err(CryptoError::TooLarge));
    assert_eq!(crypto::inflate_for_test(&bomb, 1_000_000).unwrap().len(), 1_000_000);

    let too_big = vec![0u8; MAX_FILE_BYTES as usize + 1];
    assert_eq!(crypto::encrypt(&key(9), "huge.bin", &too_big), Err(CryptoError::TooLarge));
}

// ---- keys as text -----------------------------------------------------------------------------------

#[test]
fn a_recovery_string_brings_the_same_key_back() {
    let key = Key::generate();
    let recovery = key.to_recovery();
    assert!(recovery.starts_with("ledger-key-1:"));
    let back = Key::parse(&recovery).unwrap();
    assert_eq!(back.as_bytes(), key.as_bytes());
    assert_eq!(back.id(), key.id());

    // Pasting adds spaces and line breaks.
    let wrapped = format!("  {}\n{}  \n", &recovery[..20], &recovery[20..]);
    assert_eq!(Key::parse(&wrapped).unwrap().as_bytes(), key.as_bytes());
}

#[test]
fn a_bare_key_is_accepted_as_hex_or_base64() {
    let key = key(0xab);
    let bytes = key.as_bytes();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(Key::parse(&hex).unwrap().as_bytes(), bytes);
    assert_eq!(Key::parse(&hex.to_uppercase()).unwrap().as_bytes(), bytes);
    for encoded in [STANDARD.encode(bytes), STANDARD.encode(bytes).trim_end_matches('=').to_string(), URL_SAFE_NO_PAD.encode(bytes)] {
        assert_eq!(Key::parse(&encoded).unwrap().as_bytes(), bytes, "{encoded}");
    }
}

#[test]
fn text_that_is_not_the_key_is_refused_without_repeating_it() {
    let recovery = key(11).to_recovery();

    // One character changed: the checksum notices.
    let mut typo: Vec<char> = recovery.chars().collect();
    let at = typo.len() - 8;
    typo[at] = if typo[at] == 'A' { 'B' } else { 'A' };
    assert_eq!(Key::parse(&typo.into_iter().collect::<String>()).err(), Some(KeyError::BadChecksum));

    assert_eq!(Key::parse(&recovery[..recovery.len() - 6]).err(), Some(KeyError::Malformed), "cut short");
    assert_eq!(Key::parse("ledger-key-9:AAAA").err(), Some(KeyError::UnsupportedVersion));
    for junk in ["", "hello world", "ledger-key-1:", "0123456789", &"g".repeat(64)] {
        let error = Key::parse(junk).err().unwrap_or_else(|| panic!("{junk:?} was accepted"));
        assert!(!error.to_string().contains(junk) || junk.is_empty(), "the message must not echo what was typed");
    }
}

#[test]
fn a_key_never_shows_in_debug_output() {
    let key = key(0x5c);
    let recovery = key.to_recovery();
    let hex: String = key.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    let shown = format!("{key:?} {:?}", Some(&key));
    assert!(!shown.contains(&hex) && !shown.contains(&recovery[13..]) && !shown.contains("92, 92, 92"), "{shown}");
    assert!(shown.contains("Key(id"), "{shown}");
}
