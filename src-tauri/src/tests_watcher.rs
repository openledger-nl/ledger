//! The vault watcher: which changes it looks at, how it waits for a burst of them to end, and, on a real folder, that a
//! file written outside Ledger is read in within seconds while the app's own writes and its bookkeeping are not.

use std::fs;
use std::path::Path;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tempfile::tempdir;

use crate::posts::AddOptions;
use crate::state::Core;
use crate::watcher::{VaultWatcher, in_vault, settle};
use crate::{posts, projects, threads};

fn write(root: &Path, rel: &str, text: &str) {
    let file = root.join(rel);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

#[test]
fn only_paths_the_vault_reads_are_looked_at() {
    for rel in [
        "projects/acme/threads/t/posts/1.md",
        "projects/acme/threads/t/reviews/1790000000000-abcd1234.md",
        "projects/acme/threads/t/reviews/1790000000000-abcd1234/1790000000001-efgh5678.md",
        "projects/acme/deliverables/report.pdf",
        "projects/acme",
        "semesters/s.md",
    ] {
        assert!(in_vault(Path::new(rel)), "{rel} is looked at");
    }
    for rel in [
        ".ledger/settings.json",
        ".ledger/conflicts/x.md",
        ".git/index",
        ".gitignore",
        "projects/acme/threads/t/posts/.1.md.tmp",
        // margin-code's atomic writes go through a dotfile next to the target
        "projects/acme/threads/t/reviews/.1790000000000-abcd1234.md.4f2a",
        "projects/acme/.obsidian/workspace.json",
        "projects/acme/attachments/site plan.png",
        "projects/acme/attachments/notes/readme.md",
    ] {
        assert!(!in_vault(Path::new(rel)), "{rel} is not looked at");
    }
}

#[test]
fn a_burst_of_events_is_looked_at_once_it_is_over() {
    let (tx, rx) = channel();
    let (done_tx, done) = channel();
    let worker = std::thread::spawn(move || settle(rx, Duration::from_millis(150), Duration::from_secs(5), || done_tx.send(Instant::now()).unwrap()));

    let last = {
        for _ in 0..5 {
            tx.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        Instant::now()
    };
    let at = done.recv_timeout(Duration::from_secs(5)).expect("looked at once the burst ended");
    assert!(at >= last + Duration::from_millis(100), "it waited for the quiet after the last event");
    assert_eq!(done.recv_timeout(Duration::from_millis(400)), Err(RecvTimeoutError::Timeout), "once for the whole burst");

    // The next change is looked at again; and with the sender gone, the worker ends.
    tx.send(()).unwrap();
    done.recv_timeout(Duration::from_secs(5)).expect("a later change is looked at too");
    drop(tx);
    worker.join().unwrap();
}

#[test]
fn a_steady_stream_of_events_does_not_put_it_off_for_ever() {
    let (tx, rx) = channel();
    let (done_tx, done) = channel();
    let worker = std::thread::spawn(move || settle(rx, Duration::from_millis(150), Duration::from_millis(300), || done_tx.send(()).unwrap()));

    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1000) {
        tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(30));
    }
    drop(tx);
    worker.join().unwrap();
    assert!(done.try_iter().count() >= 2, "looked at every MAX_WAIT while the changes kept coming");
}

/// A vault with a project and a thread, held the way the app holds it, and a watcher on it that reports each reload.
fn watched() -> (tempfile::TempDir, tempfile::TempDir, Arc<Mutex<Core>>, VaultWatcher, std::sync::mpsc::Receiver<()>) {
    let dir = tempdir().unwrap();
    let config = tempdir().unwrap();
    let core = Core::open_at(dir.path().to_path_buf(), config.path().to_path_buf(), false).unwrap();
    projects::create(&core.conn, "Acme").unwrap();
    threads::create(&core.conn, "acme", "Talk").unwrap();
    let mut core = core;
    core.vault.persist(&core.conn).unwrap();
    let core = Arc::new(Mutex::new(core));

    let (tx, reloads) = channel();
    let watcher = VaultWatcher::start_with(core.clone(), Duration::from_millis(200), Duration::from_secs(2), move || {
        let _ = tx.send(());
    });
    assert!(watcher.watching());
    // The system's watch is live by now, but give it a moment before the first change.
    std::thread::sleep(Duration::from_millis(300));
    (dir, config, core, watcher, reloads)
}

#[test]
fn a_file_written_outside_ledger_is_read_in_within_seconds() {
    let (dir, _config, core, _watcher, reloads) = watched();
    let root = dir.path();

    // Written the way margin-code writes: a dotfile next to the target, renamed over it.
    write(root, "projects/acme/threads/talk/posts/.outside.md.tmp", "Written in another editor.");
    fs::rename(root.join("projects/acme/threads/talk/posts/.outside.md.tmp"), root.join("projects/acme/threads/talk/posts/outside.md")).unwrap();

    reloads.recv_timeout(Duration::from_secs(10)).expect("the change was read in");
    let core = core.lock().unwrap();
    let bodies: Vec<String> = posts::list(&core.conn, "acme", "talk").unwrap().into_iter().map(|p| p.body).collect();
    assert_eq!(bodies, ["Written in another editor."]);
    assert!(!core.vault.changed_on_disk());
}

#[test]
fn the_apps_own_writes_and_its_bookkeeping_are_not_reloaded() {
    let (dir, _config, core, _watcher, reloads) = watched();
    let root = dir.path();

    {
        let mut core = core.lock().unwrap();
        let core = &mut *core;
        posts::add(&core.conn, "acme", "talk", "Written in Ledger.", AddOptions::default()).unwrap();
        core.vault.persist(&core.conn).unwrap();
    }
    write(root, ".ledger/settings.json", "{}");
    write(root, "projects/acme/attachments/site plan.png", "png");
    write(root, "projects/acme/threads/talk/posts/.half-written.md.tmp", "not yet");

    assert_eq!(reloads.recv_timeout(Duration::from_millis(1500)), Err(RecvTimeoutError::Timeout), "nothing was reloaded");

    // A change outside Ledger after all that is still noticed.
    write(root, "projects/acme/threads/talk/posts/later.md", "Later, elsewhere.");
    reloads.recv_timeout(Duration::from_secs(10)).expect("the later change was read in");
    assert_eq!(posts::list(&core.lock().unwrap().conn, "acme", "talk").unwrap().len(), 2);
}

#[test]
fn a_moved_vault_is_watched_in_its_new_place() {
    let (dir, config, core, watcher, reloads) = watched();
    let moved = tempdir().unwrap();
    {
        let mut core = core.lock().unwrap();
        core.set_vault(moved.path(), true).unwrap();
    }
    watcher.watch(moved.path());
    std::thread::sleep(Duration::from_millis(300));
    drop((dir, config));

    write(moved.path(), "projects/acme/threads/talk/posts/there.md", "In the new place.");
    reloads.recv_timeout(Duration::from_secs(10)).expect("the new folder is watched");
    assert_eq!(posts::list(&core.lock().unwrap().conn, "acme", "talk").unwrap()[0].body, "In the new place.");
}
