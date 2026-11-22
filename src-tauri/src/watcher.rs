//! Watching the vault's folder, so that what the cloud storage brings in from another machine (or another program
//! writes) is read in at once, not only when the window comes back into focus: a change made in Ledger to something
//! that changed elsewhere meanwhile would otherwise end up as a conflict copy (see `Vault::persist`).
//!
//! Events come in bursts (a sync client writes a file in several steps, a pull brings many files), so the watcher
//! waits until the folder has been quiet for a moment, then asks the vault whether anything it reads changed. Ledger's
//! own writes are seen too, and change nothing: the vault knows what it wrote.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

/// How long the folder must be quiet before it is looked at.
pub const QUIET: Duration = Duration::from_millis(700);

/// What the watcher does once a burst of changes is over: look at the vault and read it in again when it changed.
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

/// The folder watched now. Watching another stops watching the first.
pub struct VaultWatcher {
    inner: Mutex<Option<RecommendedWatcher>>,
    tx: Sender<()>,
}

impl VaultWatcher {
    /// Starts the thread that waits for quiet and then calls `on_change`.
    pub fn start(on_change: OnChange) -> VaultWatcher {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || settle(&rx, QUIET, &*on_change));
        VaultWatcher { inner: Mutex::new(None), tx }
    }

    /// Watches the vault in `root`, and nothing else. A folder that can't be watched (it isn't there, or the system
    /// has no watches left) is left to the check when the window comes back into focus.
    pub fn watch(&self, root: &Path) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        *inner = None;
        let tx = self.tx.clone();
        let made = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|e| e.paths.iter().any(|p| counts(p))) {
                let _ = tx.send(());
            }
        });
        match made {
            Ok(mut watcher) => match watcher.watch(root, RecursiveMode::Recursive) {
                Ok(()) => *inner = Some(watcher),
                Err(e) => crate::applog::info("vault", format!("Not watching {}: {e}", root.display())),
            },
            Err(e) => crate::applog::info("vault", format!("Can't watch the vault's folder: {e}")),
        }
    }

    /// Stops watching (no vault is open).
    pub fn stop(&self) {
        *self.inner.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// Whether a change to `path` can matter: the temporary files writes go through, and other dotfiles, don't.
fn counts(path: &Path) -> bool {
    !path.components().any(|c| c.as_os_str().to_str().is_some_and(|s| s.starts_with('.')))
}

/// Waits for a change, then for `quiet` without one, then calls `on_change`; for as long as the sender lives.
pub fn settle(rx: &Receiver<()>, quiet: Duration, on_change: &dyn Fn()) {
    while rx.recv().is_ok() {
        loop {
            match rx.recv_timeout(quiet) {
                Ok(()) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        on_change();
    }
}
