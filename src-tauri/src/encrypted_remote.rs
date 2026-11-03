//! A [`Remote`] that seals everything it uploads and opens everything it downloads, around whichever remote does
//! the actual talking (GitHub, or the fake in the tests).
//!
//! Paths are hidden as well as contents. Each file is stored under its [`crypto::remote_name`], all side by side,
//! and one more file, [`MANIFEST`], holds the real paths, sealed. Without the key the repository shows how many
//! files there are, how big they are and when they change, but not what they are called or how they are grouped.
//! A file's real path is still what its encryption is bound to, so a blob put under another file's name doesn't
//! open. The sync above only ever sees real paths.
//!
//! A branch encrypted by an earlier version stored its files under their readable paths, and has no manifest. It
//! is refused rather than converted, since its history would keep the names (see `sync::READABLE_NAMES`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::crypto::{self, CryptoError, Key};
use crate::error::{Error, Result};
use crate::sync::{self, Change, Head, Remote};

pub const MANIFEST: &str = "manifest";
/// What the manifest is sealed for in place of a path. No vault path can be this: `.ledger/` is never synced.
const MANIFEST_SEALED_AS: &str = ".ledger/manifest";

pub struct EncryptedRemote<R> {
    inner: R,
    key: Key,
    /// The paths in the manifest of the tree last read or written, with that tree's sha. A push needs its parent's,
    /// which is nearly always the tree this round has just read.
    manifest: Mutex<Option<(String, BTreeSet<String>)>>,
}

impl<R: Remote> EncryptedRemote<R> {
    pub fn new(inner: R, key: Key) -> EncryptedRemote<R> {
        EncryptedRemote { inner, key, manifest: Mutex::new(None) }
    }

    fn name(&self, path: &str) -> String {
        crypto::remote_name(&self.key, path)
    }

    fn seal(&self, path: &str, bytes: &[u8]) -> std::result::Result<Vec<u8>, CryptoError> {
        crypto::encrypt(&self.key, path, bytes).map(|sealed| crypto::armor(&sealed))
    }

    fn open(&self, path: &str, stored: &[u8]) -> std::result::Result<Vec<u8>, CryptoError> {
        // A plain file is never taken in: a branch that holds one can't be an encrypted one.
        crypto::unarmor(stored).and_then(|sealed| crypto::decrypt(&self.key, path, &sealed))
    }

    /// The real paths of the files in `head`, from its manifest. `stored` is the head's tree, if it was read already.
    fn paths(&self, head: &Head, stored: Option<&BTreeMap<String, String>>) -> Result<BTreeSet<String>> {
        if let Some((tree, paths)) = &*self.manifest.lock().unwrap()
            && *tree == head.tree
        {
            return Ok(paths.clone());
        }
        let fetched;
        let stored = match stored {
            Some(stored) => stored,
            None => {
                fetched = self.inner.tree(head)?;
                &fetched
            }
        };
        let paths = match (stored.get(MANIFEST), stored.iter().next()) {
            (Some(sha), _) => self.open_manifest(sha)?,
            (None, None) => BTreeSet::new(),
            // Files under readable names: either plain ones, or sealed by a version that didn't hide names.
            (None, Some((name, sha))) => {
                let blob = self.inner.blob(name, sha)?;
                return Err(if crypto::is_encrypted(&blob) { Error::sync(sync::READABLE_NAMES) } else { failed(name, CryptoError::NotEncrypted) });
            }
        };
        self.remember(&head.tree, &paths);
        Ok(paths)
    }

    fn open_manifest(&self, sha: &str) -> Result<BTreeSet<String>> {
        let text = self.open(MANIFEST_SEALED_AS, &self.inner.blob(MANIFEST, sha)?).map_err(|cause| failed(MANIFEST, cause))?;
        let paths: BTreeSet<String> = serde_json::from_slice(&text).map_err(|_| failed(MANIFEST, CryptoError::Corrupt))?;
        // Only someone with the key can write the manifest, but even they don't get to write outside the vault.
        let inside_vault = |path: &String| path.split('/').all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains('\\'));
        if !paths.iter().all(inside_vault) {
            return Err(failed(MANIFEST, CryptoError::Corrupt));
        }
        Ok(paths)
    }

    fn remember(&self, tree: &str, paths: &BTreeSet<String>) {
        *self.manifest.lock().unwrap() = Some((tree.to_string(), paths.clone()));
    }
}

fn failed(path: &str, cause: CryptoError) -> Error {
    Error::Crypto { path: path.to_string(), cause }
}

impl<R: Remote> Remote for EncryptedRemote<R> {
    fn head(&self) -> Result<Option<Head>> {
        self.inner.head()
    }

    fn tree(&self, head: &Head) -> Result<BTreeMap<String, String>> {
        let stored = self.inner.tree(head)?;
        let paths = self.paths(head, Some(&stored))?;
        // A name that isn't in the manifest isn't one of ours (a README added on GitHub, say), and is left alone.
        Ok(paths
            .into_iter()
            .filter_map(|path| {
                let sha = stored.get(&self.name(&path))?.clone();
                Some((path, sha))
            })
            .collect())
    }

    fn blob(&self, path: &str, sha: &str) -> Result<Vec<u8>> {
        let stored = self.inner.blob(&self.name(path), sha)?;
        self.open(path, &stored).map_err(|cause| failed(path, cause))
    }

    fn push(&self, parent: Option<&Head>, changes: &[Change], message: &str) -> Result<Head> {
        let before = match parent {
            Some(head) => self.paths(head, None)?,
            None => BTreeSet::new(),
        };
        let mut paths = before.clone();
        let mut sealed = Vec::with_capacity(changes.len() + 1);
        for change in changes {
            match change {
                Change::Put { path, bytes } => {
                    sealed.push(Change::Put { path: self.name(path), bytes: self.seal(path, bytes).map_err(|cause| failed(path, cause))? });
                    paths.insert(path.clone());
                }
                Change::Delete { path } => {
                    sealed.push(Change::Delete { path: self.name(path) });
                    paths.remove(path);
                }
            }
        }
        // Only adding or removing a file changes the manifest; an edit leaves it as it was.
        if paths != before {
            let list = serde_json::to_vec(&paths).expect("a set of strings serialises");
            sealed.push(Change::Put { path: MANIFEST.to_string(), bytes: self.seal(MANIFEST_SEALED_AS, &list).map_err(|cause| failed(MANIFEST, cause))? });
        }
        let head = self.inner.push(parent, &sealed, message)?;
        self.remember(&head.tree, &paths);
        Ok(head)
    }

    fn key_id(&self) -> Option<u8> {
        Some(self.key.id())
    }
}
