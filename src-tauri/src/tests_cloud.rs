//! A vault in a folder that cloud storage keeps in sync: another machine's changes arrive behind the app's back, and
//! the provider's conflict copies show up beside the files.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use rusqlite::Connection;
use tempfile::tempdir;

use crate::conflicts::{self, Conflict};
use crate::vault::Vault;
use crate::{deliverables, posts, projects, store, threads, watcher};

fn open(root: &Path) -> (Vault, Connection) {
    crate::marker::ensure(root);
    let mut vault = Vault::open(root.to_path_buf()).unwrap();
    let conn = vault.load().unwrap();
    (vault, conn)
}

fn write(root: &Path, rel: &str, text: &str) {
    let file = root.join(rel);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

/// Writes `text` as another machine would, so the file's size and time both change.
fn edit_elsewhere(root: &Path, rel: &str, text: &str) {
    std::thread::sleep(Duration::from_millis(20));
    write(root, rel, text);
}

#[test]
fn a_file_another_machine_changed_is_not_overwritten_and_ledgers_edit_is_kept_beside_it() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let p = projects::create(&conn, "Acme").unwrap().slug;
    let t = threads::create(&conn, &p, "Call").unwrap().slug;
    let post = posts::add(&conn, &p, &t, "Mine, first.", Default::default()).unwrap();
    vault.persist(&conn).unwrap();
    let rel = format!("projects/{p}/threads/{t}/posts/{}.md", post.id);
    let theirs = format!("---\nid: {}\ncreatedAt: {}\n---\n\nTheirs, from the laptop, a little longer.\n", post.id, post.created_at);
    edit_elsewhere(root, &rel, &theirs);

    // An edit here before the watcher noticed: their file stays, mine is kept as a copy, and the vault is read again.
    posts::update(&conn, &p, &t, &post.id, "Mine, second.").unwrap();
    vault.persist(&conn).unwrap();
    assert_eq!(fs::read_to_string(root.join(&rel)).unwrap(), theirs, "the other machine's edit is not overwritten");
    assert!(vault.take_stale(), "the vault knows it is behind");
    let conn = vault.load().unwrap();
    let copies: Vec<&Conflict> = vault.conflicts().iter().collect();
    assert_eq!(copies.len(), 1, "{copies:?}");
    assert_eq!(copies[0].original, rel);
    assert!(copies[0].copy.contains(" (Ledger conflict "), "{}", copies[0].copy);
    assert!(fs::read_to_string(root.join(&copies[0].copy)).unwrap().ends_with("Mine, second.\n"));
    // In memory: their version, once.
    let listed = posts::list(&conn, &p, &t).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].body.contains("Theirs"));
}

#[test]
fn a_file_another_machine_changed_is_not_deleted_either() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let p = projects::create(&conn, "Acme").unwrap().slug;
    let t = threads::create(&conn, &p, "Call").unwrap().slug;
    let post = posts::add(&conn, &p, &t, "Soon gone.", Default::default()).unwrap();
    vault.persist(&conn).unwrap();
    let rel = format!("projects/{p}/threads/{t}/posts/{}.md", post.id);
    edit_elsewhere(root, &rel, &format!("---\nid: {}\n---\n\nStill wanted elsewhere, and longer.\n", post.id));

    posts::delete(&conn, &p, &t, &post.id).unwrap();
    vault.persist(&conn).unwrap();
    assert!(root.join(&rel).exists(), "an edit made elsewhere beats a delete here");
    assert!(vault.take_stale());
}

#[test]
fn only_what_changed_is_written() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let p = projects::create(&conn, "Acme").unwrap().slug;
    let t = threads::create(&conn, &p, "Call").unwrap().slug;
    for i in 0..5 {
        posts::add(&conn, &p, &t, &format!("Note {i}."), Default::default()).unwrap();
    }
    vault.persist(&conn).unwrap();
    let before = store::at(root).list(&["projects"], |_| false);

    let post = &posts::list(&conn, &p, &t).unwrap()[2];
    posts::update(&conn, &p, &t, &post.id, "Changed.").unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(vault.persist(&conn).unwrap(), 1, "one note changed, one file written");
    let after = store::at(root).list(&["projects"], |_| false);
    let changed: Vec<&String> = after.iter().filter(|(rel, fp)| before.get(*rel) != Some(fp)).map(|(rel, _)| rel).collect();
    assert_eq!(changed.len(), 1, "{changed:?}");
    assert!(!vault.take_stale());
}

#[test]
fn the_providers_conflict_copies_are_listed_not_read_in() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    let p = projects::create(&conn, "Acme").unwrap().slug;
    let t = threads::create(&conn, &p, "Call").unwrap().slug;
    let post = posts::add(&conn, &p, &t, "The one.", Default::default()).unwrap();
    deliverables::create(&conn, &p, "Plan", "# Plan\n").unwrap();
    vault.persist(&conn).unwrap();
    let posts_dir = format!("projects/{p}/threads/{t}/posts");
    let original = fs::read_to_string(root.join(format!("{posts_dir}/{}.md", post.id))).unwrap();

    // OneDrive's, Nextcloud's, Dropbox's and Syncthing's copies.
    write(root, &format!("{posts_dir}/{}-DESKTOP-4F2A.md", post.id), &original.replace("The one.", "The one, on the desktop."));
    write(root, &format!("projects/{p}/threads/{t}/thread (conflicted copy 2026-11-20 101500).md"), "---\ntitle: Call, elsewhere\n---\n");
    write(root, &format!("projects/{p}/deliverables/plan (Laptop's conflicted copy 2026-11-20).md"), "# Plan, the laptop's\n");
    write(root, &format!("projects/{p}/deliverables/plan.sync-conflict-20261120-101500-ABCDEFG.md"), "# Plan, syncthing's\n");
    // Not copies: a deliverable of one's own named with capitals after a dash, and a note named like a copy of nothing.
    write(root, &format!("projects/{p}/deliverables/plan-V2.md"), "# Plan, version two of my own\n");
    write(root, &format!("{posts_dir}/nothing-LAPTOP.md"), "---\nid: nothing-LAPTOP\n---\n\nA note of its own.\n");

    let (vault, conn) = open(root);
    let mut found: Vec<(String, String)> = vault.conflicts().iter().map(|c| (c.copy.clone(), c.original.clone())).collect();
    found.sort();
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(found.iter().any(|(copy, original)| copy.ends_with("-DESKTOP-4F2A.md") && original.ends_with(&format!("{}.md", post.id))));
    assert!(found.iter().any(|(copy, original)| copy.contains("thread (conflicted copy") && original.ends_with("/thread.md")));
    let names: Vec<String> = deliverables::list(&conn, &p).unwrap().into_iter().map(|d| d.path).collect();
    assert_eq!(names, ["plan-V2.md", "plan.md"], "copies aren't deliverables");
    assert_eq!(posts::list(&conn, &p, &t).unwrap().len(), 2, "the post once, and the note of its own");
    assert_eq!(threads::get(&conn, root, &p, &t).unwrap().title, "Call");
}

#[test]
fn conflict_copies_are_known_by_their_names() {
    let exists = |p: &str| ["projects/a/threads/t/posts/1795-ab12.md", "projects/a/project.md", "projects/a/deliverables/plan.md"].contains(&p);
    assert_eq!(conflicts::original_of("projects/a/threads/t/posts/1795-ab12-LAPTOP.md", exists).as_deref(), Some("projects/a/threads/t/posts/1795-ab12.md"));
    assert_eq!(conflicts::original_of("projects/a/project-MILANS-PC.md", exists).as_deref(), Some("projects/a/project.md"));
    assert_eq!(conflicts::original_of("projects/a/deliverables/plan-FINAL.md", exists), None, "a name someone chose");
    assert_eq!(conflicts::original_of("projects/a/deliverables/plan (conflicted copy 2026-11-20 101500).md", exists).as_deref(), Some("projects/a/deliverables/plan.md"));
    assert_eq!(conflicts::original_of("projects/a/deliverables/plan (2).md", exists), None);
    let copy = conflicts::ledger_copy("projects/a/deliverables/plan.md", "2026-11-20 101500");
    assert_eq!(copy, "projects/a/deliverables/plan (Ledger conflict 2026-11-20 101500).md");
    assert_eq!(conflicts::original_of(&copy, exists).as_deref(), Some("projects/a/deliverables/plan.md"));
}

#[test]
fn a_write_the_system_refuses_for_a_moment_is_tried_again() {
    let tries = AtomicUsize::new(0);
    let done = store::retrying(|| {
        if tries.fetch_add(1, Ordering::SeqCst) < 2 { Err(io::Error::from_raw_os_error(32)) } else { Ok("written") }
    });
    assert_eq!(done.unwrap(), "written");
    assert_eq!(tries.load(Ordering::SeqCst), 3);

    let gone = store::retrying(|| Err::<(), _>(io::Error::from(ErrorKind::NotFound)));
    assert_eq!(gone.unwrap_err().kind(), ErrorKind::NotFound, "an error that won't go away is not tried again");
}

#[test]
fn a_burst_of_changes_is_looked_at_once_it_is_over() {
    let (tx, rx) = mpsc::channel();
    let looked = Arc::new(AtomicUsize::new(0));
    let counter = looked.clone();
    let worker = std::thread::spawn(move || watcher::settle(&rx, Duration::from_millis(60), &|| {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    for _ in 0..10 {
        tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(looked.load(Ordering::SeqCst), 1, "one look for the burst");
    tx.send(()).unwrap();
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(looked.load(Ordering::SeqCst), 2);
    drop(tx);
    worker.join().unwrap();
}

#[test]
fn what_another_machine_wrote_is_noticed_and_read_in() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let (mut vault, conn) = open(root);
    projects::create(&conn, "Acme").unwrap();
    vault.persist(&conn).unwrap();
    assert!(!vault.changed_on_disk(), "Ledger's own writes are not news");

    write(root, "projects/bakery/project.md", "---\nname: Bakery\n---\n");
    assert!(vault.changed_on_disk());
    let conn = vault.load().unwrap();
    assert_eq!(projects::list(&conn).unwrap().len(), 2);
    assert!(!vault.changed_on_disk());
}
