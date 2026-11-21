//! The running app: the in-memory database and the vault it is mirrored to.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::screenshot::Screenshots;
use crate::settings;
use crate::vault::Vault;

pub struct Core {
    /// The working copy every read is answered from.
    pub conn: Connection,
    pub vault: Vault,
    pub config_dir: PathBuf,
    /// A vault is open. Not before the first one is chosen: then the app shows the vault picker, `vault` points at a
    /// folder that doesn't exist and every change is refused.
    pub open: bool,
    /// The vault location came from `LEDGER_VAULT`, so Settings can't change it.
    pub vault_from_env: bool,
}

impl Core {
    /// Opens the vault wherever it is configured to be, and loads it; with none chosen yet, starts without one. So
    /// does a vault that can't be opened (one from Ledger 0.x, say): the picker then says why.
    pub fn open(config_dir: PathBuf, documents_dir: Option<PathBuf>) -> Result<Core> {
        match settings::resolve_vault(&config_dir, documents_dir) {
            (Some(root), from_env) => {
                if !from_env {
                    // A vault from before the picker, found in its old default place, is put on the list.
                    let _ = settings::remember_vault(&config_dir, &root);
                }
                if let Some(refused) = crate::marker::refusal(&root, &crate::marker::inspect(&root)) {
                    crate::applog::info("vault", format!("Not opening {}: {refused}", root.display()));
                    return Core::without_vault(config_dir);
                }
                Core::open_at(root, config_dir, from_env)
            }
            (None, _) => Core::without_vault(config_dir),
        }
    }

    pub fn open_at(root: PathBuf, config_dir: PathBuf, vault_from_env: bool) -> Result<Core> {
        let mut vault = Vault::open(root)?;
        settings::register_local_dir(vault.root(), settings::local_dir_in(&config_dir, vault.id()));
        let conn = vault.load()?;
        Ok(Core { conn, vault, config_dir, open: true, vault_from_env })
    }

    /// The app before its first vault: an empty database, and nothing on disk until one is created or opened.
    pub fn without_vault(config_dir: PathBuf) -> Result<Core> {
        let vault = Vault::none(config_dir.join("no-vault"));
        let conn = crate::db::open_in_memory()?;
        Ok(Core { conn, vault, config_dir, open: false, vault_from_env: false })
    }

    /// Reads everything in again from the files.
    pub fn reload(&mut self) -> Result<()> {
        if !self.open {
            return Ok(());
        }
        self.conn = self.vault.load()?;
        Ok(())
    }

    /// Switches to the vault in `path`. With `copy`, and when that folder is empty, the current vault's files
    /// are copied there first, so "move my vault" is one step.
    pub fn set_vault(&mut self, path: &Path, copy: bool) -> Result<()> {
        if self.vault_from_env {
            return Err(Error::invalid("The vault location is set by the LEDGER_VAULT environment variable"));
        }
        if !path.is_absolute() {
            return Err(Error::invalid("Enter the full path of the folder"));
        }
        if self.open && path == self.vault.root() {
            return Ok(());
        }
        if self.open && (path.starts_with(self.vault.root()) || self.vault.root().starts_with(path)) {
            return Err(Error::invalid("The new vault can't be inside the current one, or the other way around"));
        }

        fs::create_dir_all(path).map_err(|e| Error::io(path, e))?;
        let empty = fs::read_dir(path).map_err(|e| Error::io(path, e))?.next().is_none();
        if copy && empty && self.open {
            copy_dir(self.vault.root(), path)?;
        }

        let mut vault = Vault::open(path.to_path_buf())?;
        settings::register_local_dir(vault.root(), settings::local_dir_in(&self.config_dir, vault.id()));
        let conn = vault.load()?;
        settings::remember_vault(&self.config_dir, path)?;

        self.vault = vault;
        self.conn = conn;
        self.open = true;
        Ok(())
    }

    /// Makes a new, empty vault: a folder called `name` in `parent`, and switches to it. A folder of that name may be
    /// there already, but only when it is empty, so nothing that is there is ever taken over by mistake.
    pub fn create_vault(&mut self, parent: &Path, name: &str) -> Result<PathBuf> {
        let name = vault_name(name)?;
        if !parent.is_absolute() {
            return Err(Error::invalid("Choose the folder to put the vault in"));
        }
        let path = parent.join(name);
        if path.exists() {
            let empty = path.is_dir() && fs::read_dir(&path).map_err(|e| Error::io(&path, e))?.next().is_none();
            if !empty {
                return Err(Error::invalid(format!("{} already exists and isn't empty. Pick another name, or open it as a vault", path.display())));
            }
        }
        self.set_vault(&path, false)?;
        Ok(path)
    }

    /// Opens a folder that is already there as the vault (as Obsidian's "Open folder as vault"). Files Ledger doesn't
    /// know are left alone, so any folder will do.
    pub fn open_vault(&mut self, path: &Path) -> Result<()> {
        if !path.is_dir() {
            return Err(Error::invalid(format!("{} isn't a folder that exists", path.display())));
        }
        self.set_vault(path, false)
    }
}

/// A vault's name is its folder's name, so it must be one that every system (and every cloud drive) accepts.
pub fn vault_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::invalid("Give the vault a name"));
    }
    if name == "." || name == ".." || name.ends_with('.') {
        return Err(Error::invalid("A vault's name can't end with a dot"));
    }
    if let Some(bad) = name.chars().find(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()) {
        return Err(Error::invalid(format!("A vault's name can't hold {bad:?}: it becomes a folder's name")));
    }
    Ok(name)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    for entry in fs::read_dir(from).map_err(|e| Error::io(from, e))?.flatten() {
        let target = to.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| Error::io(&entry.path(), e))?;
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|e| Error::io(&target, e))?;
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).map_err(|e| Error::io(&target, e))?;
        }
    }
    Ok(())
}

/// What the Tauri commands hold on to.
pub struct App {
    core: Arc<Mutex<Core>>,
    pub screenshots: Screenshots,
}

impl App {
    pub fn new(core: Arc<Mutex<Core>>, screenshots: Screenshots) -> App {
        App { core, screenshots }
    }

    pub fn lock(&self) -> MutexGuard<'_, Core> {
        self.core.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Answers from memory. The closure also gets the vault's root, for the files the database only points at.
    pub fn read<T>(&self, f: impl FnOnce(&Connection, &Path) -> Result<T>) -> Result<T> {
        let core = self.lock();
        f(&core.conn, core.vault.root())
    }

    /// Changes memory, writes the difference through to the vault's files, and lets the Trello sync know.
    pub fn mutate<T>(&self, f: impl FnOnce(&Connection, &Path) -> Result<T>) -> Result<T> {
        let out = {
            let mut guard = self.lock();
            let core = &mut *guard;
            if !core.open {
                return Err(Error::invalid("Create or open a vault first"));
            }
            let out = f(&core.conn, core.vault.root())?;
            core.vault.persist(&core.conn)?;
            out
        };
        // Always, not only when `persist` wrote something: some changes are made to files directly (a published
        // image, an upload), which the write-through doesn't count. The Trello sync (if it is set up) looks at what
        // changed after a pause.
        crate::trello::notify_vault_changed();
        Ok(out)
    }
}
