//! Sealing a vault's files with its key: AES-256-GCM, the file's path sealed with it, and the key kept as a recovery
//! string that carries a checksum. Used by an encrypted vault (see `vault_crypto`), whose files are kept on disk only
//! sealed, each under a name made from its path with the key ([`blob_name`]), which says nothing about the path to
//! anyone without the key.
//!
//! A sealed file is, byte for byte:
//!
//! ```text
//! 0x89 'L' 'D' 'G'     magic (the first byte is not valid UTF-8, so no text file starts like this)
//! version              3
//! key id               one byte taken from the key, so a wrong key is recognised before anything is decrypted
//! flags                bit 0: the file was deflated before it was sealed
//! nonce                12 random bytes, for the path
//! path length          2 bytes, little-endian: the length of the sealed path that follows
//! sealed path          AES-256-GCM over the file's path in the vault (`projects/acme/project.md`), with its tag
//! nonce                12 random bytes, new for every write, for the contents
//! sealed contents      AES-256-GCM over the file (deflated, when that made it smaller), with its tag
//! ```
//!
//! The header is the associated data of the path, and the header and the path are that of the contents: a file
//! renamed to another's name, or with its header altered, doesn't open. The path is sealed on its own so that listing
//! the vault reads only the start of each file. Text is deflated *before* it is sealed, because ciphertext does not
//! compress; pictures and PDFs, which are compressed already, are kept as they are.

use std::fmt;
use std::io::{Read, Write};

use aes_gcm::aead::{Aead, Generate, Nonce, Payload};
use aes_gcm::{Aes256Gcm, KeyInit};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// The largest file that is sealed, so that a damaged or hostile one can't make this machine inflate gigabytes.
pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

pub const KEY_BYTES: usize = 32;
const MAGIC: [u8; 4] = [0x89, b'L', b'D', b'G'];
const VERSION: u8 = 3;
/// Magic, version, key id and flags: the part of the header that is also authenticated.
pub const HEADER_BYTES: usize = MAGIC.len() + 3;
const NONCE_BYTES: usize = 12;
const TAG_BYTES: usize = 16;
const DEFLATED: u8 = 1;
/// The longest path a sealed file holds.
const MAX_PATH_BYTES: usize = 4096;
/// Where a recovery string starts, and the checksum that follows the key inside it.
const RECOVERY_PREFIX: &str = "ledger-key-1:";
const CHECK_BYTES: usize = 4;

// ---- errors -----------------------------------------------------------------------------------------

/// Why a file could not be sealed or opened. Deliberately vague about *which* check failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// Not one of ours: no header.
    NotEncrypted,
    /// Sealed in a format this app doesn't read.
    UnsupportedVersion(u8),
    /// Sealed with another key (by its id).
    WrongKey { found: u8, expected: u8 },
    /// The key is wrong after all (ids can coincide), or the file was altered.
    Failed,
    /// Cut short or otherwise not a sealed file.
    Corrupt,
    /// Bigger than Ledger seals.
    TooLarge,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CryptoError::NotEncrypted => f.write_str("isn't encrypted"),
            CryptoError::UnsupportedVersion(v) => write!(f, "was encrypted in a format this Ledger doesn't read (format {v})"),
            CryptoError::WrongKey { found, expected } => write!(f, "was encrypted with a different key (id {found:02x}; this vault's key is {expected:02x})"),
            CryptoError::Failed => f.write_str("can't be decrypted: the key is wrong, or the file was altered"),
            CryptoError::Corrupt => f.write_str("is damaged"),
            CryptoError::TooLarge => f.write_str("is larger than Ledger encrypts (512 MB)"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Why text offered as a key was refused. Never says what was typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    /// Not a recognisable key.
    Malformed,
    /// A recovery string whose check failed: a character was mistyped or lost.
    BadChecksum,
    /// A recovery string from a newer format.
    UnsupportedVersion,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            KeyError::Malformed => "That doesn't look like a Ledger key. Paste the recovery key exactly as it was shown.",
            KeyError::BadChecksum => "That recovery key has a typo or is cut short: its check failed. Copy it again in full.",
            KeyError::UnsupportedVersion => "That recovery key was made by a newer version of Ledger.",
        })
    }
}

// ---- the key ----------------------------------------------------------------------------------------

/// 32 random bytes. Wiped from memory when dropped; not `Clone`, and its `Debug` shows nothing.
pub struct Key(Zeroizing<[u8; KEY_BYTES]>);

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Key(id {:02x})", self.id())
    }
}

impl Key {
    pub fn generate() -> Key {
        Key(Zeroizing::new(<[u8; KEY_BYTES]>::generate()))
    }

    pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Key {
        Key(Zeroizing::new(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }

    /// A one-byte label for this key: enough to tell keys apart in a header and on screen, far too little to
    /// say anything about the key.
    pub fn id(&self) -> u8 {
        digest(b"ledger key id", &self.0[..])[0]
    }

    pub fn id_hex(&self) -> String {
        format!("{:02x}", self.id())
    }

    /// The text to keep somewhere safe: `ledger-key-1:` and the key, with a checksum so a typo is caught.
    pub fn to_recovery(&self) -> Zeroizing<String> {
        let mut payload = Zeroizing::new(Vec::with_capacity(KEY_BYTES + CHECK_BYTES));
        payload.extend_from_slice(&self.0[..]);
        payload.extend_from_slice(&digest(b"ledger recovery check", &self.0[..])[..CHECK_BYTES]);
        Zeroizing::new(format!("{RECOVERY_PREFIX}{}", URL_SAFE_NO_PAD.encode(&payload[..])))
    }

    /// A recovery string, or the bare key as 64 hex digits or as base64.
    pub fn parse(text: &str) -> std::result::Result<Key, KeyError> {
        // Pasting often brings spaces and line breaks along.
        let text: Zeroizing<String> = Zeroizing::new(text.chars().filter(|c| !c.is_whitespace()).collect());

        if let Some(encoded) = text.strip_prefix(RECOVERY_PREFIX) {
            let payload = Zeroizing::new(URL_SAFE_NO_PAD.decode(encoded.trim_end_matches('=')).map_err(|_| KeyError::Malformed)?);
            if payload.len() != KEY_BYTES + CHECK_BYTES {
                return Err(KeyError::Malformed);
            }
            let key = Key::from_slice(&payload[..KEY_BYTES]).ok_or(KeyError::Malformed)?;
            if digest(b"ledger recovery check", &key.0[..])[..CHECK_BYTES] != payload[KEY_BYTES..] {
                return Err(KeyError::BadChecksum);
            }
            return Ok(key);
        }
        if text.starts_with("ledger-key-") {
            return Err(KeyError::UnsupportedVersion);
        }

        if text.len() == KEY_BYTES * 2 && text.chars().all(|c| c.is_ascii_hexdigit()) {
            let mut bytes = Zeroizing::new([0u8; KEY_BYTES]);
            for (i, pair) in text.as_bytes().chunks(2).enumerate() {
                bytes[i] = u8::from_str_radix(std::str::from_utf8(pair).map_err(|_| KeyError::Malformed)?, 16).map_err(|_| KeyError::Malformed)?;
            }
            return Ok(Key::from_bytes(*bytes));
        }
        let unpadded = text.trim_end_matches('=');
        for engine in [STANDARD_NO_PAD, URL_SAFE_NO_PAD] {
            if let Ok(bytes) = engine.decode(unpadded).map(Zeroizing::new)
                && let Some(key) = Key::from_slice(&bytes)
            {
                return Ok(key);
            }
        }
        Err(KeyError::Malformed)
    }

    fn from_slice(bytes: &[u8]) -> Option<Key> {
        Some(Key::from_bytes(<[u8; KEY_BYTES]>::try_from(bytes).ok()?))
    }

    fn cipher(&self) -> std::result::Result<Aes256Gcm, CryptoError> {
        Aes256Gcm::new_from_slice(&self.0[..]).map_err(|_| CryptoError::Failed)
    }
}

/// The name the file at the vault path `path` is kept under: 32 hex digits, the same on every machine with the key,
/// and saying nothing about the path to anyone without it.
pub fn blob_name(key: &Key, path: &str) -> String {
    let name_key = Zeroizing::new(digest(b"ledger blob names", &key.0[..]));
    #[expect(clippy::expect_used, reason = "HMAC takes a key of any length: it can't fail")]
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&name_key[..]).expect("HMAC takes a key of any length");
    mac.update(path.as_bytes());
    mac.finalize().into_bytes()[..16].iter().map(|b| format!("{b:02x}")).collect()
}

fn digest(domain: &[u8], key: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(key);
    hasher.finalize().into()
}

// ---- sealing and opening ----------------------------------------------------------------------------

/// Whether deflating is worth trying: not for what is compressed already.
fn worth_deflating(path: &str) -> bool {
    const PACKED: [&str; 14] = ["png", "jpg", "jpeg", "gif", "webp", "jxl", "pdf", "zip", "docx", "xlsx", "pptx", "mp4", "mp3", "gz"];
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    !PACKED.contains(&ext.as_str())
}

/// Seals the file at the vault path `path`: its path, and its contents (deflated first when that makes them smaller).
pub fn seal(key: &Key, path: &str, plaintext: &[u8]) -> std::result::Result<Vec<u8>, CryptoError> {
    if plaintext.len() as u64 > MAX_FILE_BYTES {
        return Err(CryptoError::TooLarge);
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(CryptoError::Corrupt);
    }
    let packed: Option<Zeroizing<Vec<u8>>> = if worth_deflating(path) {
        let mut deflater = DeflateEncoder::new(Vec::new(), Compression::default());
        deflater.write_all(plaintext).map_err(|_| CryptoError::Corrupt)?;
        let packed = Zeroizing::new(deflater.finish().map_err(|_| CryptoError::Corrupt)?);
        (packed.len() < plaintext.len()).then_some(packed)
    } else {
        None
    };
    let (body, flags): (&[u8], u8) = match &packed {
        Some(p) => (&p[..], DEFLATED),
        None => (plaintext, 0),
    };

    let cipher = key.cipher()?;
    let mut out = Vec::with_capacity(HEADER_BYTES + 2 * NONCE_BYTES + 2 + path.len() + body.len() + 2 * TAG_BYTES);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(key.id());
    out.push(flags);
    let header = out.clone();

    let path_nonce = Nonce::<Aes256Gcm>::generate();
    let sealed_path = cipher.encrypt(&path_nonce, Payload { msg: path.as_bytes(), aad: &header }).map_err(|_| CryptoError::Failed)?;
    out.extend_from_slice(&path_nonce);
    out.extend_from_slice(&(sealed_path.len() as u16).to_le_bytes());
    out.extend_from_slice(&sealed_path);

    let nonce = Nonce::<Aes256Gcm>::generate();
    let sealed = cipher.encrypt(&nonce, Payload { msg: body, aad: &associated_data(&header, path) }).map_err(|_| CryptoError::Failed)?;
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// How many bytes from the start of a sealed file are needed to read its path; `None` when `start` is too short to
/// say (it holds fewer than the header and the path's length).
pub fn path_bytes_needed(start: &[u8]) -> Option<usize> {
    let at = HEADER_BYTES + NONCE_BYTES;
    let len = u16::from_le_bytes([*start.get(at)?, *start.get(at + 1)?]) as usize;
    Some(at + 2 + len)
}

/// The vault path a sealed file holds, read from its start (see [`path_bytes_needed`]).
pub fn open_path(key: &Key, start: &[u8]) -> std::result::Result<String, CryptoError> {
    let (header, _) = check_header(key, start)?;
    let at = HEADER_BYTES;
    let nonce = Nonce::<Aes256Gcm>::try_from(start.get(at..at + NONCE_BYTES).ok_or(CryptoError::Corrupt)?).map_err(|_| CryptoError::Corrupt)?;
    let len = u16::from_le_bytes([*start.get(at + NONCE_BYTES).ok_or(CryptoError::Corrupt)?, *start.get(at + NONCE_BYTES + 1).ok_or(CryptoError::Corrupt)?]) as usize;
    let from = at + NONCE_BYTES + 2;
    let sealed = start.get(from..from + len).ok_or(CryptoError::Corrupt)?;
    let path = key.cipher()?.decrypt(&nonce, Payload { msg: sealed, aad: header }).map_err(|_| CryptoError::Failed)?;
    String::from_utf8(path).map_err(|_| CryptoError::Corrupt)
}

/// Opens a file sealed by [`seal`]: its path and its contents.
pub fn open(key: &Key, blob: &[u8]) -> std::result::Result<(String, Vec<u8>), CryptoError> {
    let (header, flags) = check_header(key, blob)?;
    let path = open_path(key, blob)?;
    let from = path_bytes_needed(blob).ok_or(CryptoError::Corrupt)?;
    if blob.len() < from + NONCE_BYTES + TAG_BYTES {
        return Err(CryptoError::Corrupt);
    }
    let (nonce, sealed) = blob[from..].split_at(NONCE_BYTES);
    let nonce = Nonce::<Aes256Gcm>::try_from(nonce).map_err(|_| CryptoError::Corrupt)?;
    let body = Zeroizing::new(key.cipher()?.decrypt(&nonce, Payload { msg: sealed, aad: &associated_data(header, &path) }).map_err(|_| CryptoError::Failed)?);
    let contents = if flags & DEFLATED != 0 { inflate(&body, MAX_FILE_BYTES)? } else { body.to_vec() };
    Ok((path, contents))
}

/// The header of a sealed file, checked: its magic, its version and that it is this key's. Returns it and its flags.
fn check_header<'a>(key: &Key, blob: &'a [u8]) -> std::result::Result<(&'a [u8], u8), CryptoError> {
    if !blob.starts_with(&MAGIC) {
        return Err(CryptoError::NotEncrypted);
    }
    if blob.len() < HEADER_BYTES + NONCE_BYTES + 2 {
        return Err(CryptoError::Corrupt);
    }
    let header = &blob[..HEADER_BYTES];
    if header[MAGIC.len()] != VERSION {
        return Err(CryptoError::UnsupportedVersion(header[MAGIC.len()]));
    }
    let found = header[MAGIC.len() + 1];
    if found != key.id() {
        return Err(CryptoError::WrongKey { found, expected: key.id() });
    }
    Ok((header, header[MAGIC.len() + 2]))
}

/// What the contents are bound to: the header, then the path.
fn associated_data(header: &[u8], path: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(header.len() + path.len());
    aad.extend_from_slice(header);
    aad.extend_from_slice(path.as_bytes());
    aad
}

/// Unpacks deflated data into at most `limit` bytes. What was sealed is authentic by the time this runs, but a
/// bound keeps even someone holding the key from making this machine inflate gigabytes.
fn inflate(packed: &[u8], limit: u64) -> std::result::Result<Vec<u8>, CryptoError> {
    let mut plaintext = Vec::new();
    DeflateDecoder::new(packed).take(limit + 1).read_to_end(&mut plaintext).map_err(|_| CryptoError::Corrupt)?;
    if plaintext.len() as u64 > limit {
        return Err(CryptoError::TooLarge);
    }
    Ok(plaintext)
}

#[cfg(test)]
pub(crate) fn inflate_for_test(packed: &[u8], limit: u64) -> std::result::Result<Vec<u8>, CryptoError> {
    inflate(packed, limit)
}

// ---- sealing a key with another ---------------------------------------------------------------------

/// Seals `secret` (a key, or a check) with `key`, bound to `context`: a nonce, then the ciphertext and its tag.
pub fn wrap(key: &Key, context: &[u8], secret: &[u8]) -> std::result::Result<Vec<u8>, CryptoError> {
    let nonce = Nonce::<Aes256Gcm>::generate();
    let sealed = key.cipher()?.encrypt(&nonce, Payload { msg: secret, aad: context }).map_err(|_| CryptoError::Failed)?;
    Ok([&nonce[..], &sealed].concat())
}

/// Opens what [`wrap`] sealed, with the same key and context.
pub fn unwrap(key: &Key, context: &[u8], wrapped: &[u8]) -> std::result::Result<Zeroizing<Vec<u8>>, CryptoError> {
    if wrapped.len() < NONCE_BYTES + TAG_BYTES {
        return Err(CryptoError::Corrupt);
    }
    let (nonce, sealed) = wrapped.split_at(NONCE_BYTES);
    let nonce = Nonce::<Aes256Gcm>::try_from(nonce).map_err(|_| CryptoError::Corrupt)?;
    key.cipher()?.decrypt(&nonce, Payload { msg: sealed, aad: context }).map(Zeroizing::new).map_err(|_| CryptoError::Failed)
}
