//! An encrypted vault: its files are kept on disk only sealed (see `crypto`), so the cloud storage the vault is in, and
//! anyone who can read the folder, see neither what is in them nor what they are called. They are plain only in
//! Ledger's memory, and in a work folder outside the synced folder for the moments a program outside Ledger needs a
//! real file (opening a picture or a PDF with its own program, editing a PDF layout, a Claude Code run).
//!
//! ```text
//! ledger.json                  the marker, with "encryption": the header below
//! data/3f/3f9a…e1.lgr          one sealed file per file of the vault, named after its path with the key, in a folder
//!                              per first two digits of the name
//! ```
//!
//! There is no list of the files that every change would touch (which two machines would keep changing at once): a
//! sealed file holds its own path, sealed, and listing the vault reads only the start of each file. A copy the cloud
//! storage made of one (`3f9a…e1-LAPTOP.lgr`) holds the same path, so it is a conflict copy like any other (see
//! `conflicts`).
//!
//! **The key.** Each vault has its own random key. The marker holds it sealed with a key made from the passphrase
//! (Argon2id), so another machine opens the vault with the passphrase; the recovery key (the key itself, as text) opens
//! it too, when the passphrase is lost. Once opened on a machine the key is kept in the system's keychain, so Ledger
//! doesn't ask at every start; without a keychain it asks.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use aes_gcm::aead::Generate;
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::{self, KEY_BYTES, Key};
use crate::error::{Error, Result};
use crate::store::{self, Fingerprint};

/// The folder the sealed files are in, at the vault's root.
pub const DATA: &str = "data";
const BLOB_EXT: &str = ".lgr";
pub const CIPHER: &str = "aes-256-gcm";
/// The shortest passphrase taken.
pub const MIN_PASSPHRASE: usize = 10;

// ---- the header in ledger.json ------------------------------------------------------------------------

/// How the vault is encrypted: in its marker, readable by anyone, and saying nothing that helps without the
/// passphrase or the recovery key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub cipher: String,
    pub kdf: Kdf,
    /// The salt of the passphrase, as base64.
    pub salt: String,
    /// The vault's key, sealed with the key the passphrase makes: base64 of a nonce, the ciphertext and its tag.
    pub wrapped_key: String,
    /// A known text sealed with the vault's key, so that a recovery key can be checked before it is used.
    pub check: String,
    /// The key's one-byte label, as two hex digits.
    pub key_id: String,
}

/// How the passphrase is made into a key: Argon2id, with its costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kdf {
    pub name: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Kdf {
    /// 64 MiB and three passes: a second or so on a laptop, and costly to guess at.
    fn default_costs() -> Kdf {
        if cfg!(test) {
            return Kdf { name: "argon2id".into(), memory_kib: 64, iterations: 1, parallelism: 1 };
        }
        Kdf { name: "argon2id".into(), memory_kib: 64 * 1024, iterations: 3, parallelism: 1 }
    }
}

fn context(vault_id: &str, what: &str) -> Vec<u8> {
    format!("ledger vault {what} {vault_id}").into_bytes()
}

/// The key a passphrase makes, with the header's salt and costs.
fn passphrase_key(passphrase: &str, salt: &[u8], kdf: &Kdf) -> Result<Key> {
    if kdf.name != "argon2id" {
        return Err(Error::invalid(format!("This vault's key is made in a way this Ledger doesn't know ({})", kdf.name)));
    }
    let params = Params::new(kdf.memory_kib, kdf.iterations, kdf.parallelism, Some(KEY_BYTES))
        .map_err(|e| Error::invalid(format!("The vault's encryption settings can't be used: {e}")))?;
    let mut out = Zeroizing::new([0u8; KEY_BYTES]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, &mut out[..])
        .map_err(|e| Error::invalid(format!("The passphrase can't be used: {e}")))?;
    Ok(Key::from_bytes(*out))
}

fn checked_passphrase(passphrase: &str) -> Result<&str> {
    if passphrase.chars().count() < MIN_PASSPHRASE {
        return Err(Error::invalid(format!("Choose a passphrase of at least {MIN_PASSPHRASE} characters: a few words you will remember")));
    }
    Ok(passphrase)
}

fn sealed_with(key: &Key, passphrase: &str, vault_id: &str) -> Result<(String, String, Kdf)> {
    let salt = <[u8; 16]>::generate();
    let kdf = Kdf::default_costs();
    let wrapping = passphrase_key(checked_passphrase(passphrase)?, &salt, &kdf)?;
    let wrapped = crypto::wrap(&wrapping, &context(vault_id, "key"), &key.as_bytes()[..]).map_err(|e| Error::invalid(format!("The key can't be sealed: {e}")))?;
    Ok((STANDARD.encode(salt), STANDARD.encode(wrapped), kdf))
}

/// A new vault's header: a new random key, sealed with the passphrase. Returns the header and the key.
pub fn new_header(vault_id: &str, passphrase: &str) -> Result<(Header, Key)> {
    let key = Key::generate();
    let (salt, wrapped_key, kdf) = sealed_with(&key, passphrase, vault_id)?;
    let check = crypto::wrap(&key, &context(vault_id, "check"), b"ledger").map_err(|e| Error::invalid(format!("The key can't be checked: {e}")))?;
    Ok((Header { cipher: CIPHER.into(), kdf, salt, wrapped_key, check: STANDARD.encode(check), key_id: key.id_hex() }, key))
}

/// The same header with the key sealed under another passphrase.
pub fn rewrap(header: &Header, vault_id: &str, key: &Key, passphrase: &str) -> Result<Header> {
    let (salt, wrapped_key, kdf) = sealed_with(key, passphrase, vault_id)?;
    Ok(Header { salt, wrapped_key, kdf, ..header.clone() })
}

/// The vault's key from what someone typed: the passphrase, or the recovery key.
pub fn unlock(header: &Header, vault_id: &str, secret: &str) -> Result<Key> {
    if header.cipher != CIPHER {
        return Err(Error::invalid(format!("This vault is encrypted in a way this Ledger doesn't know ({})", header.cipher)));
    }
    let wrong = || Error::invalid("That isn't the passphrase or the recovery key of this vault");
    if secret.trim_start().starts_with("ledger-key-") {
        let key = Key::parse(secret).map_err(|e| Error::invalid(e.to_string()))?;
        let check = STANDARD.decode(&header.check).map_err(|_| wrong())?;
        crypto::unwrap(&key, &context(vault_id, "check"), &check).map_err(|_| wrong())?;
        return Ok(key);
    }
    let salt = STANDARD.decode(&header.salt).map_err(|_| wrong())?;
    let wrapped = STANDARD.decode(&header.wrapped_key).map_err(|_| wrong())?;
    let wrapping = passphrase_key(secret, &salt, &header.kdf)?;
    let bytes = crypto::unwrap(&wrapping, &context(vault_id, "key"), &wrapped).map_err(|_| wrong())?;
    let bytes: [u8; KEY_BYTES] = bytes[..].try_into().map_err(|_| wrong())?;
    Ok(Key::from_bytes(bytes))
}

// ---- keys that are open on this machine ------------------------------------------------------------

static OPEN: LazyLock<Mutex<HashMap<String, Arc<Key>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Keeps the vault's key while Ledger runs, and in the system's keychain with `keep` so the next start doesn't ask.
pub fn remember(vault_id: &str, key: Key, keep: bool) -> Arc<Key> {
    if keep {
        keychain::save(vault_id, &key);
    }
    let key = Arc::new(key);
    OPEN.lock().unwrap_or_else(PoisonError::into_inner).insert(vault_id.to_string(), key.clone());
    key
}

/// The vault's key when this machine has it: opened since Ledger started, or kept in the keychain.
pub fn known(vault_id: &str) -> Option<Arc<Key>> {
    if let Some(key) = OPEN.lock().unwrap_or_else(PoisonError::into_inner).get(vault_id) {
        return Some(key.clone());
    }
    let key = Arc::new(keychain::load(vault_id)?);
    OPEN.lock().unwrap_or_else(PoisonError::into_inner).insert(vault_id.to_string(), key.clone());
    Some(key)
}

/// Whether the keychain keeps the vault's key, so a start doesn't ask for the passphrase.
pub fn kept(vault_id: &str) -> bool {
    keychain::load(vault_id).is_some()
}

/// Takes the key out of the keychain: the next start asks for the passphrase. While Ledger runs it stays open.
pub fn forget(vault_id: &str) {
    keychain::forget(vault_id);
}

/// The system's keychain, as for the other secrets; tests keep keys in memory only.
mod keychain {
    use crate::crypto::Key;

    #[cfg(not(test))]
    const SERVICE: &str = "nl.openledger.ledger";

    #[cfg(not(test))]
    fn entry(vault_id: &str) -> Option<keyring::Entry> {
        keyring::Entry::new(SERVICE, &format!("{vault_id}/vault-key")).ok()
    }

    #[cfg(not(test))]
    pub fn load(vault_id: &str) -> Option<Key> {
        Key::parse(&entry(vault_id)?.get_password().ok()?).ok()
    }

    #[cfg(not(test))]
    pub fn save(vault_id: &str, key: &Key) {
        if let Some(entry) = entry(vault_id)
            && let Err(e) = entry.set_password(&key.to_recovery())
        {
            crate::applog::info("vault", format!("The vault's key wasn't kept in the keychain ({e}): Ledger asks for the passphrase at the next start"));
        }
    }

    #[cfg(not(test))]
    pub fn forget(vault_id: &str) {
        if let Some(entry) = entry(vault_id) {
            let _ = entry.delete_credential();
        }
    }

    #[cfg(test)]
    static KEPT: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, String>>> = std::sync::LazyLock::new(Default::default);

    #[cfg(test)]
    pub fn load(vault_id: &str) -> Option<Key> {
        Key::parse(KEPT.lock().ok()?.get(vault_id)?).ok()
    }

    #[cfg(test)]
    pub fn save(vault_id: &str, key: &Key) {
        if let Ok(mut kept) = KEPT.lock() {
            kept.insert(vault_id.to_string(), key.to_recovery().to_string());
        }
    }

    #[cfg(test)]
    pub fn forget(vault_id: &str) {
        if let Ok(mut kept) = KEPT.lock() {
            kept.remove(vault_id);
        }
    }

    /// For tests: Ledger started again, so only what the keychain kept is known.
    #[cfg(test)]
    pub fn restart() {
        super::OPEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
    }
}

#[cfg(test)]
pub fn restart_for_test() {
    keychain::restart();
}

// ---- the sealed files ---------------------------------------------------------------------------------

/// What `Store` does in an encrypted vault: the key, what it knows of the sealed files, and the work folder.
pub struct Sealed {
    key: Arc<Key>,
    /// Each sealed file seen, by its place under the vault (`data/3f/3f9a…e1.lgr`): its fingerprint when its path was
    /// read, and that path, so a listing decrypts only what is new or changed.
    seen: Mutex<HashMap<String, (Fingerprint, String)>>,
    /// Where plain copies are made for programs outside Ledger: outside the synced folder, wiped when the vault is
    /// opened.
    work: PathBuf,
    /// The copies made there, by their path in the vault, with their fingerprint when they were made or last taken
    /// back in; and the folders copied whole (for a Claude Code run), whose new files are taken in too.
    copies: Mutex<(BTreeMap<String, Fingerprint>, Vec<String>)>,
}

impl Sealed {
    pub fn new(key: Arc<Key>, vault_id: &str) -> Sealed {
        let work = std::env::temp_dir().join("ledger-work").join(vault_id);
        let _ = fs::remove_dir_all(&work);
        Sealed { key, seen: Mutex::new(HashMap::new()), work, copies: Mutex::new((BTreeMap::new(), Vec::new())) }
    }

    /// Where the sealed file of the vault path `path` belongs.
    fn blob_of(&self, path: &str) -> String {
        let name = crypto::blob_name(&self.key, path);
        format!("{DATA}/{}/{name}{BLOB_EXT}", &name[..2])
    }

    /// Every sealed file under `data/`, by its place, with its fingerprint.
    fn blobs(root: &Path) -> BTreeMap<String, Fingerprint> {
        store::Store::plain(root.to_path_buf()).list(&[DATA], |_| false).into_iter().filter(|(rel, _)| rel.ends_with(BLOB_EXT)).collect()
    }

    /// The vault path the sealed file at `blob` holds, read from its start (or from what was seen of it).
    fn path_in(&self, root: &Path, blob: &str, fp: Fingerprint) -> Option<String> {
        if let Some((seen_fp, path)) = self.seen.lock().unwrap_or_else(PoisonError::into_inner).get(blob)
            && *seen_fp == fp
        {
            return Some(path.clone());
        }
        let mut file = fs::File::open(root.join(blob)).ok()?;
        let mut start = vec![0u8; 64];
        let got = file.read(&mut start).ok()?;
        start.truncate(got);
        let needed = crypto::path_bytes_needed(&start)?;
        if needed > start.len() {
            let mut rest = vec![0u8; needed - start.len()];
            file.read_exact(&mut rest).ok()?;
            start.extend_from_slice(&rest);
        }
        match crypto::open_path(&self.key, &start) {
            Ok(path) => {
                self.seen.lock().unwrap_or_else(PoisonError::into_inner).insert(blob.to_string(), (fp, path.clone()));
                Some(path)
            }
            Err(e) => {
                crate::applog::info("vault", format!("{blob} {e}; it is left out"));
                None
            }
        }
    }

    /// Every file of the vault, by its path, with the fingerprint of its sealed file. A second sealed file with the
    /// same path (a copy the cloud storage made) is listed as a conflict copy of it.
    pub fn list(&self, root: &Path) -> BTreeMap<String, Fingerprint> {
        let mut out = BTreeMap::new();
        let mut copies = Vec::new();
        for (blob, fp) in Self::blobs(root) {
            let Some(path) = self.path_in(root, &blob, fp) else { continue };
            if blob == self.blob_of(&path) || !out.contains_key(&path) && !Self::blobs_has(root, &self.blob_of(&path)) {
                out.insert(path, fp);
            } else {
                copies.push((path, blob, fp));
            }
        }
        for (path, blob, fp) in copies {
            out.insert(copy_name(&path, &blob), fp);
        }
        out
    }

    fn blobs_has(root: &Path, blob: &str) -> bool {
        root.join(blob).is_file()
    }

    /// The sealed file that holds `path`: its own, or, for a conflict copy, the copy's.
    fn locate(&self, root: &Path, path: &str) -> Option<String> {
        let own = self.blob_of(path);
        if root.join(&own).is_file() {
            return Some(own);
        }
        // A conflict copy, or a file only a copy holds: found by what the listing saw.
        let seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        seen.iter().find(|(blob, (_, p))| p == path || copy_name(p, blob) == path).map(|(blob, _)| blob.clone())
    }

    pub fn fingerprint(&self, root: &Path, path: &str) -> Option<Fingerprint> {
        store::fingerprint_of(&root.join(self.locate(root, path)?))
    }

    pub fn read(&self, root: &Path, path: &str) -> io::Result<Vec<u8>> {
        let blob = self.locate(root, path).ok_or_else(|| io::Error::from(ErrorKind::NotFound))?;
        let bytes = fs::read(root.join(&blob))?;
        let (_, contents) = crypto::open(&self.key, &bytes).map_err(|e| io::Error::new(ErrorKind::InvalidData, format!("{path} {e}")))?;
        Ok(contents)
    }

    pub fn write(&self, root: &Path, path: &str, bytes: &[u8]) -> io::Result<()> {
        let sealed = crypto::seal(&self.key, path, bytes).map_err(|e| io::Error::new(ErrorKind::InvalidData, format!("{path} {e}")))?;
        let blob = self.blob_of(path);
        // A file only a copy held is written to its own place from now on, and the copy goes.
        let held_by_copy = self.locate(root, path).filter(|b| *b != blob);
        store::Store::plain(root.to_path_buf()).write(&blob, &sealed)?;
        if let Some(fp) = store::fingerprint_of(&root.join(&blob)) {
            self.seen.lock().unwrap_or_else(PoisonError::into_inner).insert(blob, (fp, path.to_string()));
        }
        if let Some(copy) = held_by_copy {
            let _ = store::Store::plain(root.to_path_buf()).remove(&copy);
        }
        Ok(())
    }

    /// Moves the file at `from` to `to`: sealed again under the new path, as its name and its seal hold the path, and
    /// with the time it was last changed kept, as a move in a vault of plain files keeps it (a version's date can be
    /// that time).
    pub fn rename(&self, root: &Path, from: &str, to: &str) -> io::Result<()> {
        if from == to {
            return Ok(());
        }
        let old = self.locate(root, from).ok_or_else(|| io::Error::from(ErrorKind::NotFound))?;
        let modified = fs::metadata(root.join(&old))?.modified()?;
        let bytes = self.read(root, from)?;
        self.write(root, to, &bytes)?;
        let blob = self.blob_of(to);
        let file = root.join(&blob);
        store::retrying(|| fs::File::options().write(true).open(&file)?.set_modified(modified))?;
        if let Some(fp) = store::fingerprint_of(&file) {
            self.seen.lock().unwrap_or_else(PoisonError::into_inner).insert(blob, (fp, to.to_string()));
        }
        self.remove(root, from)
    }

    pub fn remove(&self, root: &Path, path: &str) -> io::Result<()> {
        let Some(blob) = self.locate(root, path) else { return Ok(()) };
        store::Store::plain(root.to_path_buf()).remove(&blob)?;
        self.seen.lock().unwrap_or_else(PoisonError::into_inner).remove(&blob);
        Ok(())
    }

    /// A plain copy of the file at `path` in the work folder, for a program outside Ledger. Changes made to it there
    /// are taken back in by [`Sealed::take_back`].
    pub fn materialize(&self, root: &Path, path: &str) -> io::Result<PathBuf> {
        let file = self.work.join(path);
        let bytes = self.read(root, path)?;
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&file, &bytes)?;
        if let Some(fp) = store::fingerprint_of(&file) {
            self.copies.lock().unwrap_or_else(PoisonError::into_inner).0.insert(path.to_string(), fp);
        }
        Ok(file)
    }

    /// Plain copies of every file under the folder `dir` of the vault (a project's, for a Claude Code run): the
    /// folder in the work folder. New files made in it are taken in too.
    pub fn materialize_dir(&self, root: &Path, dir: &str, files: &[String]) -> io::Result<PathBuf> {
        let folder = self.work.join(dir);
        fs::create_dir_all(&folder)?;
        for path in files {
            self.materialize(root, path)?;
        }
        let mut copies = self.copies.lock().unwrap_or_else(PoisonError::into_inner);
        if !copies.1.contains(&dir.to_string()) {
            copies.1.push(dir.to_string());
        }
        Ok(folder)
    }

    /// Takes in what changed in the work folder: copies edited by their program, and new files in a folder copied
    /// whole; a copy that is gone from such a folder goes from the vault too. Returns how many files were taken in.
    pub fn take_back(&self, root: &Path) -> io::Result<usize> {
        let (known, dirs) = {
            let copies = self.copies.lock().unwrap_or_else(PoisonError::into_inner);
            (copies.0.clone(), copies.1.clone())
        };
        let mut now: BTreeMap<String, Fingerprint> = BTreeMap::new();
        for path in known.keys() {
            if let Some(fp) = store::fingerprint_of(&self.work.join(path)) {
                now.insert(path.clone(), fp);
            }
        }
        for dir in &dirs {
            let plain = store::Store::plain(self.work.clone());
            let top = dir.split('/').next().unwrap_or(dir);
            for (path, fp) in plain.list(&[top], |_| false) {
                if path.starts_with(&format!("{dir}/")) {
                    now.insert(path, fp);
                }
            }
        }
        let mut taken = 0;
        for (path, fp) in &now {
            if known.get(path) != Some(fp) {
                let bytes = fs::read(self.work.join(path))?;
                self.write(root, path, &bytes)?;
                taken += 1;
            }
        }
        for path in known.keys().filter(|p| !now.contains_key(*p) && dirs.iter().any(|d| p.starts_with(&format!("{d}/")))) {
            self.remove(root, path)?;
            taken += 1;
        }
        self.copies.lock().unwrap_or_else(PoisonError::into_inner).0 = now;
        Ok(taken)
    }

    /// The work folder, wiped: the plain copies go.
    pub fn wipe(&self) {
        let _ = fs::remove_dir_all(&self.work);
        *self.copies.lock().unwrap_or_else(PoisonError::into_inner) = (BTreeMap::new(), Vec::new());
    }
}

impl Drop for Sealed {
    fn drop(&mut self) {
        self.wipe();
    }
}

/// The name a second sealed file with the same path is listed under: a conflict copy of the path.
fn copy_name(path: &str, blob: &str) -> String {
    let stem = blob.rsplit('/').next().unwrap_or(blob).trim_end_matches(BLOB_EXT);
    let tail: String = stem.chars().skip(32).filter(|c| c.is_ascii_alphanumeric()).take(24).collect();
    crate::conflicts::ledger_copy(path, &format!("from the cloud {}", if tail.is_empty() { &stem[..stem.len().min(8)] } else { &tail }))
}
