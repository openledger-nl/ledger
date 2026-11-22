//! Sealing and opening files, and keys as text.

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

fn sealed(key: &Key, path: &str, bytes: &[u8]) -> Vec<u8> {
    crypto::seal(key, path, bytes).unwrap()
}

#[test]
fn a_file_and_its_path_survive_the_round_trip() {
    let key = key(1);
    for plaintext in [&b""[..], b"hello", "naïve — ünïcode ✓".as_bytes(), &[0u8, 159, 146, 150, 255], &vec![42u8; 100_000]] {
        for path in ["projects/p/README.md", "projects/p/attachments/chart.png"] {
            let blob = sealed(&key, path, plaintext);
            assert_eq!(crypto::open(&key, &blob).unwrap(), (path.to_string(), plaintext.to_vec()));
        }
    }
}

#[test]
fn the_path_is_read_from_the_start_of_the_file_alone() {
    let key = key(2);
    let path = "projects/acquisition-company-x/threads/layoffs-q1/thread.md";
    let blob = sealed(&key, path, &vec![7u8; 50_000]);
    let needed = crypto::path_bytes_needed(&blob[..32]).unwrap();
    assert!(needed < 200, "{needed}");
    assert_eq!(crypto::open_path(&key, &blob[..needed]).unwrap(), path);
    assert!(crypto::path_bytes_needed(&blob[..10]).is_none(), "too short to say");
}

#[test]
fn what_is_sealed_looks_nothing_like_the_file_and_is_new_every_time() {
    let key = key(2);
    let text = b"the quick brown fox jumps over the lazy dog, again and again and again";
    let path = "projects/secret-client/notes.md";
    let (a, b) = (sealed(&key, path, text), sealed(&key, path, text));
    assert_ne!(a, b, "fresh nonces for every write");
    for needle in [&text[..12], b"secret-client"] {
        assert!(!a.windows(needle.len()).any(|w| w == needle), "no plaintext shows through, the path neither");
    }
    assert_eq!(&a[..4], &[0x89, b'L', b'D', b'G'], "magic");
    assert_eq!(a[4], 3, "format version");
    assert_eq!(a[5], key.id(), "key id");
}

#[test]
fn text_is_compressed_before_it_is_sealed_and_pictures_are_not() {
    let key = key(3);
    let text = "the same line over and over\n".repeat(2_000);
    let blob = sealed(&key, "notes.md", text.as_bytes());
    assert!(blob.len() < text.len() / 10, "{} bytes sealed from {}", blob.len(), text.len());
    assert_eq!(blob[6] & 1, 1, "marked as deflated");
    let picture = sealed(&key, "chart.png", text.as_bytes());
    assert_eq!(picture[6] & 1, 0, "a picture is compressed already");
    assert_eq!(crypto::open(&key, &picture).unwrap().1, text.as_bytes());
}

#[test]
fn the_wrong_key_is_recognised_and_says_which_key_it_was() {
    let (right, wrong) = (key(4), different_from(&key(4)));
    let blob = sealed(&right, "a.md", b"secret");
    assert_eq!(crypto::open(&wrong, &blob), Err(CryptoError::WrongKey { found: right.id(), expected: wrong.id() }));
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
    let blob = sealed(&right, "a.md", b"secret");
    assert_eq!(crypto::open(&twin, &blob), Err(CryptoError::Failed));
}

#[test]
fn a_tampered_file_is_refused_wherever_it_was_touched() {
    let key = key(6);
    let blob = sealed(&key, "a.md", b"some words worth protecting");
    // Every byte after the header, the length of the sealed path apart (changing it cuts the file up differently).
    let length_at = crypto::HEADER_BYTES + 12;
    for i in crypto::HEADER_BYTES..blob.len() {
        if i == length_at || i == length_at + 1 {
            continue;
        }
        let mut bad = blob.clone();
        bad[i] ^= 0x01;
        assert!(crypto::open(&key, &bad).is_err(), "byte {i}");
    }
    let mut longer = blob.clone();
    longer[length_at] ^= 0x01;
    assert!(crypto::open(&key, &longer).is_err());

    let mut newer = blob.clone();
    newer[4] = 4;
    assert_eq!(crypto::open(&key, &newer), Err(CryptoError::UnsupportedVersion(4)));
    let mut other_label = blob.clone();
    other_label[5] ^= 0x01;
    assert!(matches!(crypto::open(&key, &other_label), Err(CryptoError::WrongKey { .. })));
    let mut flags = blob.clone();
    flags[6] ^= 0x01;
    assert_eq!(crypto::open(&key, &flags), Err(CryptoError::Failed), "the flags are authenticated");

    assert!(crypto::open(&key, &blob[..blob.len() - 1]).is_err(), "a clipped tag");
    assert_eq!(crypto::open(&key, &blob[..10]), Err(CryptoError::Corrupt), "cut short");
    assert!(crypto::open(&key, &blob).is_ok(), "the original is untouched");
}

#[test]
fn a_file_is_kept_under_a_name_only_the_key_can_make() {
    let name = crypto::blob_name(&key(12), "projects/acquisition-company-x/threads/layoffs-q1/thread.md");
    assert_eq!(name.len(), 32);
    assert!(name.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)), "{name}");
    assert_eq!(name, crypto::blob_name(&key(12), "projects/acquisition-company-x/threads/layoffs-q1/thread.md"), "the same on every machine");
    assert_ne!(name, crypto::blob_name(&key(13), "projects/acquisition-company-x/threads/layoffs-q1/thread.md"), "another key, another name");
    assert_ne!(crypto::blob_name(&key(12), "a.md"), crypto::blob_name(&key(12), "b.md"));
}

#[test]
fn a_plain_file_is_not_mistaken_for_a_sealed_one() {
    let key = key(8);
    assert_eq!(crypto::open(&key, b"# just markdown"), Err(CryptoError::NotEncrypted));
    assert_eq!(crypto::open(&key, b""), Err(CryptoError::NotEncrypted));
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
    assert_eq!(crypto::seal(&key(9), "huge.bin", &too_big), Err(CryptoError::TooLarge));
}

#[test]
fn a_key_sealed_with_another_opens_only_with_it_and_its_context() {
    let (outer, inner) = (key(20), key(21));
    let wrapped = crypto::wrap(&outer, b"ledger vault key 1234", inner.as_bytes()).unwrap();
    assert_eq!(&crypto::unwrap(&outer, b"ledger vault key 1234", &wrapped).unwrap()[..], &inner.as_bytes()[..]);
    assert!(crypto::unwrap(&outer, b"ledger vault key 9999", &wrapped).is_err(), "bound to the vault");
    assert!(crypto::unwrap(&different_from(&outer), b"ledger vault key 1234", &wrapped).is_err());
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
