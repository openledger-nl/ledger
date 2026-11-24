//! Every file of the vault is read and written through here, by its path in the vault (`projects/acme/project.md`,
//! always with `/`). That keeps one place that knows how the files are kept: as they are, in a vault of plain files,
//! or sealed, in an encrypted one (see `vault_crypto`); and one place that takes care of what a folder synced by
//! cloud storage needs:
//!
//! - **Writes are atomic**: a file is written beside its place under a dot-name and renamed over it, so a crash or a
//!   sync client never sees half a note.
//! - **A write the system refuses for a moment is tried again.** On Windows a sync client that holds a file open
//!   makes replacing it fail (a sharing violation, EBUSY or EPERM); it lets go within moments.
//! - **Dotfiles are not part of the vault**, the temporary files writes go through included.
//!
//! The store of a vault is found by the vault's folder (see [`at`]), set when the vault is opened, so that everything
//! that only has the vault's folder reaches the same store.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::{Duration, UNIX_EPOCH};

/// A file's size and modification time: enough to notice that something else touched it.
pub type Fingerprint = (u64, u128);

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
    /// The vault's key and what it knows of the sealed files; `None` for a vault of plain files.
    sealed: Option<Arc<crate::vault_crypto::Sealed>>,
}

static STORES: LazyLock<Mutex<HashMap<PathBuf, Store>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The store of the vault in `root`: the one set when it was opened, or one for plain files.
pub fn at(root: &Path) -> Store {
    STORES.lock().unwrap_or_else(PoisonError::into_inner).get(root).cloned().unwrap_or_else(|| Store::plain(root.to_path_buf()))
}

/// Makes `store` the one of its vault's folder.
pub fn register(store: &Store) {
    STORES.lock().unwrap_or_else(PoisonError::into_inner).insert(store.root.clone(), store.clone());
}

/// How many times a write the system refuses is tried, and how long it waits before each next try.
const TRIES: u32 = 5;
const PAUSE: Duration = Duration::from_millis(120);

/// Whether an error is one that goes away when another program lets go of the file.
fn is_transient(e: &io::Error) -> bool {
    // 32 and 33 are Windows' sharing and lock violations; a sync client holding the file shows as either, or as
    // permission denied.
    matches!(e.kind(), ErrorKind::PermissionDenied | ErrorKind::ResourceBusy) || matches!(e.raw_os_error(), Some(32 | 33))
}

/// Runs `op` again, a moment later, while it fails in a way that goes away by itself.
pub fn retrying<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut tries = 1;
    loop {
        match op() {
            Err(e) if tries < TRIES && is_transient(&e) => {
                std::thread::sleep(PAUSE * tries);
                tries += 1;
            }
            other => return other,
        }
    }
}

pub fn fingerprint_of(file: &Path) -> Option<Fingerprint> {
    let meta = fs::metadata(file).ok()?;
    let nanos = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    Some((meta.len(), nanos))
}

/// Writes `bytes` to `file` in one step: beside it under a dot-name, then renamed over it.
fn write_atomic(file: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let temp = file.with_file_name(format!(".{name}.tmp"));
    retrying(|| fs::write(&temp, bytes))?;
    retrying(|| fs::rename(&temp, file)).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

/// After a delete, removes the folders it left empty, stopping at `root` (the vault's, or a folder in it).
pub fn prune_empty_dirs(root: &Path, file: &Path) {
    let mut dir = file.parent();
    while let Some(current) = dir {
        if current == root || !current.starts_with(root) || fs::remove_dir(current).is_err() {
            break;
        }
        dir = current.parent();
    }
}

impl Store {
    pub fn plain(root: PathBuf) -> Store {
        Store { root, sealed: None }
    }

    pub fn sealed(root: PathBuf, sealed: Arc<crate::vault_crypto::Sealed>) -> Store {
        Store { root, sealed: Some(sealed) }
    }

    /// Whether the vault's files are encrypted.
    pub fn is_sealed(&self) -> bool {
        self.sealed.is_some()
    }

    /// Where the file at the vault path `rel` is on disk, in a vault of plain files.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Every file of the vault under the folders `tops`, by its path in the vault, with its fingerprint; `skip` leaves
    /// out a folder (by its path in the vault) and everything in it.
    pub fn list(&self, tops: &[&str], skip: impl Fn(&str) -> bool) -> BTreeMap<String, Fingerprint> {
        if let Some(sealed) = &self.sealed {
            return sealed
                .list(&self.root)
                .into_iter()
                .filter(|(rel, _)| tops.iter().any(|top| rel.starts_with(&format!("{top}/"))) && !ancestors(rel).any(&skip))
                .collect();
        }
        fn walk(root: &Path, rel: &str, skip: &dyn Fn(&str) -> bool, out: &mut BTreeMap<String, Fingerprint>) {
            let Ok(entries) = fs::read_dir(root.join(rel)) else { return };
            for entry in entries.flatten() {
                let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
                if name.starts_with('.') {
                    continue;
                }
                let child = format!("{rel}/{name}");
                let Ok(kind) = entry.file_type() else { continue };
                if kind.is_dir() {
                    if !skip(&child) {
                        walk(root, &child, skip, out);
                    }
                } else if kind.is_file()
                    && let Some(fp) = fingerprint_of(&entry.path())
                {
                    out.insert(child, fp);
                }
            }
        }
        let mut out = BTreeMap::new();
        for top in tops {
            walk(&self.root, top, &skip, &mut out);
        }
        out
    }

    /// The files directly and further inside the folder `dir` (a vault path), by their path in the vault.
    pub fn list_under(&self, dir: &str) -> BTreeMap<String, Fingerprint> {
        let dir = dir.trim_end_matches('/');
        let top = dir.split('/').next().unwrap_or(dir);
        let prefix = format!("{dir}/");
        self.list(&[top], |_| false).into_iter().filter(|(rel, _)| rel.starts_with(&prefix)).collect()
    }

    pub fn fingerprint(&self, rel: &str) -> Option<Fingerprint> {
        match &self.sealed {
            Some(sealed) => sealed.fingerprint(&self.root, rel),
            None => fingerprint_of(&self.root.join(rel)),
        }
    }

    pub fn exists(&self, rel: &str) -> bool {
        match &self.sealed {
            Some(_) => self.fingerprint(rel).is_some(),
            None => self.root.join(rel).is_file(),
        }
    }

    pub fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        match &self.sealed {
            Some(sealed) => sealed.read(&self.root, rel),
            None => fs::read(self.root.join(rel)),
        }
    }

    pub fn read_string(&self, rel: &str) -> io::Result<String> {
        String::from_utf8(self.read(rel)?).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
    }

    /// Writes the file at `rel` in one step, making its folders.
    pub fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()> {
        match &self.sealed {
            Some(sealed) => sealed.write(&self.root, rel, bytes),
            None => write_atomic(&self.root.join(rel), bytes),
        }
    }

    /// Removes the file at `rel` (one that isn't there is fine), and the folders that leaves empty.
    pub fn remove(&self, rel: &str) -> io::Result<()> {
        if let Some(sealed) = &self.sealed {
            return sealed.remove(&self.root, rel);
        }
        let file = self.root.join(rel);
        match retrying(|| fs::remove_file(&file)) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        prune_empty_dirs(&self.root, &file);
        Ok(())
    }

    /// Moves the file at `from` to `to`, making the folders `to` needs and removing those `from` leaves empty. The time
    /// it was last changed goes with it.
    pub fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if let Some(sealed) = &self.sealed {
            return sealed.rename(&self.root, from, to);
        }
        let (a, b) = (self.root.join(from), self.root.join(to));
        if let Some(dir) = b.parent() {
            fs::create_dir_all(dir)?;
        }
        retrying(|| fs::rename(&a, &b))?;
        prune_empty_dirs(&self.root, &a);
        Ok(())
    }

    /// Takes a file from outside the vault in, at `rel`. Returns its size.
    pub fn import_file(&self, source: &Path, rel: &str) -> io::Result<u64> {
        let bytes = fs::read(source)?;
        self.write(rel, &bytes)?;
        Ok(bytes.len() as u64)
    }

    /// Removes the folder `rel` and everything in it.
    pub fn remove_dir(&self, rel: &str) -> io::Result<()> {
        if self.sealed.is_some() {
            for file in self.list_under(rel).keys() {
                self.remove(file)?;
            }
            return Ok(());
        }
        let dir = self.root.join(rel);
        if dir.is_dir() {
            retrying(|| fs::remove_dir_all(&dir))?;
        }
        Ok(())
    }

    /// A file on disk with what the vault holds at `rel`, for a program outside Ledger to open: the file itself in a
    /// vault of plain files; in an encrypted one a plain copy in the work folder, outside the synced folder, whose
    /// changes [`Store::take_back`] takes in.
    pub fn materialize(&self, rel: &str) -> io::Result<PathBuf> {
        match &self.sealed {
            Some(sealed) => sealed.materialize(&self.root, rel),
            None => Ok(self.root.join(rel)),
        }
    }

    /// The folder `dir` of the vault on disk, for a program that works in a folder (a Claude Code run): the folder
    /// itself in a vault of plain files; in an encrypted one a plain copy of everything in it in the work folder,
    /// whose changes and new files [`Store::take_back`] takes in.
    pub fn materialize_dir(&self, dir: &str) -> io::Result<PathBuf> {
        match &self.sealed {
            Some(sealed) => {
                let files: Vec<String> = self.list_under(dir).into_keys().collect();
                sealed.materialize_dir(&self.root, dir, &files)
            }
            None => Ok(self.root.join(dir)),
        }
    }

    /// Takes in what programs outside Ledger changed in the plain copies; nothing to do in a vault of plain files.
    /// Returns how many files were taken in.
    pub fn take_back(&self) -> io::Result<usize> {
        match &self.sealed {
            Some(sealed) => sealed.take_back(&self.root),
            None => Ok(0),
        }
    }
}

/// `projects/acme/attachments/a.png` → `projects/acme/attachments`, `projects/acme`, `projects`.
fn ancestors(rel: &str) -> impl Iterator<Item = &str> {
    let mut end = rel.len();
    std::iter::from_fn(move || {
        let cut = rel[..end].rfind('/')?;
        end = cut;
        Some(&rel[..cut])
    })
}
