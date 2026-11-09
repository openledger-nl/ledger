//! Notices, while the app runs, files changed in the vault by something other than Ledger: an edit in Obsidian, a
//! review written in VS Code with margin-code, a `git pull`. Before, those were only picked up when the window
//! regained focus, which still happens as a fallback (`commands::check_vault_changes`).
//!
//! The folder is watched with the system's own notifications (the `notify` crate). An event under a dotfile or
//! dot-folder (`.ledger/`, `.git/`, the temporary files every write goes through) or in a project's `attachments/` is
//! dropped as it arrives. Any other starts a wait for the burst to end ([`QUIET`] without events, [`MAX_WAIT`] at most);
//! then the vault is asked whether its files changed behind the app's back ([`Vault::changed_on_disk`], which knows
//! the app's own writes), and only then reloaded. What follows a reload (refreshing the screens, syncing) is the
//! caller's, passed to [`VaultWatcher::start`] as `on_reload`.
//!
//! [`Vault::changed_on_disk`]: crate::vault::Vault::changed_on_disk

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::applog;
use crate::state::Core;

/// How long the vault must stay quiet after a change before it is looked at, and the longest a steady stream of
/// changes can put that off.
pub const QUIET: Duration = Duration::from_secs(1);
pub const MAX_WAIT: Duration = Duration::from_secs(5);

pub struct VaultWatcher {
    /// The system's watch on the vault folder; replaced when the vault moves, `None` when it couldn't be set up.
    watcher: Mutex<Option<RecommendedWatcher>>,
    tx: Sender<()>,
}

impl VaultWatcher {
    /// Watches the vault `core` holds, and reloads it when its files change behind the app's back. `on_reload` is
    /// called after each such reload, without the core locked.
    pub fn start(core: Arc<Mutex<Core>>, on_reload: impl Fn() + Send + 'static) -> VaultWatcher {
        VaultWatcher::start_with(core, QUIET, MAX_WAIT, on_reload)
    }

    /// [`VaultWatcher::start`] with other waits, for the tests.
    pub fn start_with(core: Arc<Mutex<Core>>, quiet: Duration, max_wait: Duration, on_reload: impl Fn() + Send + 'static) -> VaultWatcher {
        let (tx, rx) = channel();
        let root = lock(&core).vault.root().to_path_buf();
        std::thread::spawn(move || {
            settle(rx, quiet, max_wait, || {
                let reloaded = lock(&core).reload_if_changed();
                match reloaded {
                    Ok(true) => on_reload(),
                    Ok(false) => {}
                    Err(e) => applog::error("vault", format!("Changes made outside Ledger couldn't be read in: {e}")),
                }
            })
        });
        let watcher = VaultWatcher { watcher: Mutex::new(None), tx };
        watcher.watch(&root);
        watcher
    }

    /// Watches this folder from now on, instead of the one before (the vault moved). If the system won't watch it
    /// (too many folders for inotify, say), changes are still picked up when the window regains focus.
    pub fn watch(&self, root: &Path) {
        let tx = self.tx.clone();
        let roots = prefixes(root);
        let made = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            if let Ok(event) = result
                && matters(&event, &roots)
            {
                let _ = tx.send(());
            }
        })
        .and_then(|mut watcher| watcher.watch(root, RecursiveMode::Recursive).map(|()| watcher));
        let mut slot = lock(&self.watcher);
        *slot = match made {
            Ok(watcher) => Some(watcher),
            Err(e) => {
                applog::error("vault", format!("{} can't be watched for changes made outside Ledger ({e}); they are read in when the window regains focus", root.display()));
                None
            }
        };
    }

    /// Whether the folder is being watched.
    #[cfg(test)]
    pub fn watching(&self) -> bool {
        lock(&self.watcher).is_some()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The vault folder as given, and as the system spells it (macOS reports `/private/var/…` for `/var/…`).
fn prefixes(root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![root.to_path_buf()];
    if let Ok(canonical) = std::fs::canonicalize(root)
        && canonical != root
    {
        roots.push(canonical);
    }
    roots
}

/// Whether an event from the system can be a change to the vault. Reading a file is not; and when the system lost
/// track (too many events at once), anything may have changed.
fn matters(event: &notify::Event, roots: &[PathBuf]) -> bool {
    if event.need_rescan() {
        return true;
    }
    !matches!(event.kind, EventKind::Access(_))
        && event.paths.iter().any(|path| roots.iter().any(|root| path.strip_prefix(root).is_ok_and(in_vault)))
}

/// Whether a path, relative to the vault, can hold something the vault reads: not a dotfile or anything in a
/// dot-folder (`.ledger/`, `.git/`, the `.name.tmp` a write goes through), and not in a project's `attachments/`.
pub fn in_vault(rel: &Path) -> bool {
    let parts: Vec<&str> = rel.components().map(|c| c.as_os_str().to_str().unwrap_or_default()).collect();
    let attachments = parts.len() >= 3 && parts[0] == "projects" && parts[2] == "attachments";
    !attachments && !parts.iter().any(|part| part.starts_with('.'))
}

/// Waits for an event, then for the events to stop (`quiet` without one, `max_wait` at most), and calls `then`; again
/// and again, until every sender is gone. Events that arrive while `then` runs start the next round.
pub fn settle(rx: Receiver<()>, quiet: Duration, max_wait: Duration, then: impl Fn()) {
    while rx.recv().is_ok() {
        let first = Instant::now();
        loop {
            let left = max_wait.saturating_sub(first.elapsed());
            if left.is_zero() {
                break;
            }
            match rx.recv_timeout(quiet.min(left)) {
                Ok(()) => {}
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        then();
    }
}
