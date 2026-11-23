//! Upgrading a vault of an older format to today's (see `upgrade`): format 1 kept a project's versions in `versions/`
//! and each run's record in its thread; format 2 keeps both in the project's `history/`. Only ever when the user
//! chooses to: opening a vault, at the start or from the picker, never changes it.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use rusqlite::Connection;
use tempfile::tempdir;

use crate::claude_code::records::{self, RunRecord};
use crate::marker::{self, Found};
use crate::state::Core;
use crate::vault::Vault;
use crate::{posts, prompts, provenance, settings, store, threads, upgrade};

const ID: &str = "0123456789abcdef0123456789abcdef";

fn write(root: &Path, rel: &str, text: &str) {
    let file = root.join(rel);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn names(root: &Path, rel: &str) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(root.join(rel)).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    out.sort();
    out
}

fn format_of(root: &Path) -> u32 {
    match marker::inspect(root).marker() {
        Some(marker) => marker.ledger,
        None => panic!("{root:?} isn't a vault"),
    }
}

/// Every file in the folder, with its contents and the time it was last changed.
fn snapshot(root: &Path) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, (Vec<u8>, SystemTime)>) {
        for entry in fs::read_dir(dir).unwrap().map(Result::unwrap) {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, (fs::read(&path).unwrap(), fs::metadata(&path).unwrap().modified().unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Why opening the vault in `root` is refused.
fn refused(root: &Path) -> String {
    Vault::open(root.to_path_buf()).err().expect("a vault of an older format isn't opened").to_string()
}

fn open(root: &Path) -> (Vault, Connection) {
    let mut vault = Vault::open(root.to_path_buf()).unwrap();
    let conn = vault.load().unwrap();
    (vault, conn)
}

const NOTE_V1: &str = "---\ncreatedAt: 2026-11-20T10:00:00.000Z\n---\n\n[plan@v0.1.md](../../../versions/plan/plan@v0.1.manifest.md)\n";

const RUN_V1: &str = "---\nprompt: p1\nstatus: finished\nmodel: opus\nstartedAt: 2026-11-20T09:00:00.000Z\nfinishedAt: 2026-11-20T09:01:00.000Z\nseconds: 60\ndeliverables:\n- plan@v0.1.md\n---\n\nWrote the plan.\n";

/// A vault as Ledger 1.0 left it: a version of a markdown plan (with its manifest), a version of a picture, a run that
/// made the plan's version, the note that says it was published, and a prompt whose feedback was on that version.
fn vault_of_format_1(root: &Path) {
    write(root, "ledger.json", &format!("{{\n  \"ledger\": 1,\n  \"id\": \"{ID}\"\n}}\n"));
    write(root, "projects/acme/project.md", "---\nname: Acme\n---\n");
    write(root, "projects/acme/threads/call/thread.md", "---\ntitle: Call\n---\n");
    write(
        root,
        "projects/acme/threads/call/prompts/p1.md",
        "---\ntitle: Tighten the plan\ndeliverables:\n- plan.md\nbase: |-\n  ### On v0.1 (versions/plan/plan@v0.1.md), from the thread \"Call\"\n\n  Shorter.\n\n  Keep the (versions/ of the app) as they are.\ncreatedAt: 2026-11-20T08:00:00.000Z\n---\n\nMake it shorter.\n",
    );
    write(root, "projects/acme/threads/call/posts/n1.md", NOTE_V1);
    write(root, "projects/acme/threads/call/runs/r1.md", RUN_V1);
    write(root, "projects/acme/deliverables/plan.md", "# Plan\n\ntwo\n");
    write(root, "projects/acme/versions/plan/plan@v0.1.md", "# Plan\n\none\n");
    write(
        root,
        "projects/acme/versions/plan/plan@v0.1.manifest.md",
        "---\ndeliverable: plan.md\nversion: '0.1'\nthread: call\nprompt: p1\nrun: r1\nby: ai\ncreatedAt: 2026-11-20T09:01:00.000Z\n---\n\nThe first go.\n",
    );
    fs::create_dir_all(root.join("projects/acme/versions/chart")).unwrap();
    fs::write(root.join("projects/acme/versions/chart/chart@v0.1.png"), [1, 2, 3]).unwrap();
}

#[test]
fn a_vault_of_format_1_is_left_as_it_is_until_the_user_chooses_to_upgrade_it() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    vault_of_format_1(root);
    let before = snapshot(root);
    assert!(matches!(marker::inspect(root), Found::Older(ref m) if m.ledger == 1 && m.id == ID));

    // Opened as a vault, from the picker or as a folder: refused, saying why and what to do.
    let why = refused(root);
    assert!(why.contains("older Ledger") && why.contains("upgrade it"), "{why}");
    let config = tempdir().unwrap();
    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    assert!(core.open_vault(root).is_err());
    assert!(!core.open);

    // The vault opened last, at the start: the app starts on the picker, which asks.
    settings::remember_vault(config.path(), root).unwrap();
    let core = Core::open(config.path().to_path_buf()).unwrap();
    assert!(!core.open, "a vault that needs an upgrade isn't opened at the start");
    drop(core);

    assert_eq!(snapshot(root), before, "nothing in it is changed, not even ledger.json");
    assert_eq!(format_of(root), 1);

    // The user chooses to upgrade it: the picker's "Upgrade and open".
    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    core.upgrade_vault(root).unwrap();
    assert!(core.open);
    assert_eq!(format_of(root), marker::FORMAT);
    let projects: Vec<String> = crate::projects::list(&core.conn).unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(projects, ["Acme"]);
}

#[test]
fn upgrading_a_vault_of_format_1_keeps_its_history_in_one_folder() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    vault_of_format_1(root);
    let made = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    fs::File::options().write(true).open(root.join("projects/acme/versions/chart/chart@v0.1.png")).unwrap().set_modified(made).unwrap();

    assert_eq!(upgrade::vault(root).unwrap().ledger, marker::FORMAT);
    let (_vault, conn) = open(root);
    assert_eq!(format_of(root), marker::FORMAT);
    assert!(read(root, "ledger.json").contains(ID), "it is the same vault, with the same id");

    // The versions are where format 2 keeps them, as they were; the old folders are gone.
    let project = "projects/acme";
    assert_eq!(names(root, project), ["README.md", "deliverables", "history", "project.md", "threads"], "no versions/ any more");
    assert_eq!(names(root, &format!("{project}/history")), ["deliverables", "runs"]);
    assert_eq!(names(root, &format!("{project}/history/deliverables")), ["chart", "plan"]);
    assert_eq!(read(root, &format!("{project}/history/deliverables/plan/plan@v0.1.md")), "# Plan\n\none\n");
    assert!(read(root, &format!("{project}/history/deliverables/plan/plan@v0.1.manifest.md")).contains("run: r1"));
    let chart = root.join(format!("{project}/history/deliverables/chart/chart@v0.1.png"));
    assert_eq!(fs::read(&chart).unwrap(), [1, 2, 3]);
    assert_eq!(fs::metadata(&chart).unwrap().modified().unwrap(), made, "a version is moved, not written anew");

    // The run's record names its thread now that its folder doesn't, first, and says the rest as it did.
    assert_eq!(names(root, &format!("{project}/threads/call")), ["posts", "prompts", "thread.md"]);
    let run = read(root, &format!("{project}/history/runs/r1.md"));
    assert!(run.starts_with("---\nthread: call\nprompt: p1\nstatus: finished\n") && run.ends_with("Wrote the plan.\n"), "{run}");

    // What pointed at the old places points at the new ones; other text is left as it was.
    let note = read(root, &format!("{project}/threads/call/posts/n1.md"));
    assert_eq!(note, NOTE_V1.replace("../../../versions/", "../../../history/deliverables/"));
    let prompt = read(root, &format!("{project}/threads/call/prompts/p1.md"));
    assert!(prompt.contains("### On v0.1 (history/deliverables/plan/plan@v0.1.md), from the thread"), "{prompt}");
    assert!(prompt.contains("Keep the (versions/ of the app) as they are."), "{prompt}");

    // And it all reads in: the version, who made it, the run behind it and the note that it was published.
    let versions = provenance::resolve(&conn, "acme", "plan.md").unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!((versions[0].run.as_deref(), versions[0].author), (Some("r1"), provenance::AI));
    assert!(versions[0].post.is_some(), "the note is the version's \"version published\" note");
    let runs = records::for_prompt(&conn, "acme", "call", "p1").unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].files, ["plan@v0.1.md"]);
    assert_eq!(prompts::list(&conn, "acme", "call").unwrap().len(), 1);

    // Upgrading it again, or opening it again, changes nothing.
    let before: Vec<(String, String)> = ["history/runs/r1.md", "threads/call/posts/n1.md", "threads/call/prompts/p1.md"]
        .iter()
        .map(|rel| (rel.to_string(), read(root, &format!("{project}/{rel}"))))
        .collect();
    upgrade::vault(root).unwrap();
    drop(open(root));
    for (rel, text) in before {
        assert_eq!(read(root, &format!("{project}/{rel}")), text, "{rel}");
    }
}

#[test]
fn an_upgrade_that_stopped_halfway_carries_on_where_it_stopped() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    vault_of_format_1(root);
    // Stopped after the plan's version was copied, and after the run's record was written, before either was removed.
    write(root, "projects/acme/history/deliverables/plan/plan@v0.1.md", "# Plan\n\none\n");
    let run_v2 = format!("---\nthread: call\n{}", RUN_V1.strip_prefix("---\n").unwrap());
    write(root, "projects/acme/history/runs/r1.md", &run_v2);

    // Its marker still says format 1, so it is still not opened: the picker asks again.
    assert!(refused(root).contains("older Ledger"));

    upgrade::vault(root).unwrap();
    let (vault, _conn) = open(root);
    assert_eq!(format_of(root), marker::FORMAT);
    assert!(!root.join("projects/acme/versions").exists() && !root.join("projects/acme/threads/call/runs").exists());
    assert_eq!(names(root, "projects/acme/history/deliverables/plan"), ["plan@v0.1.manifest.md", "plan@v0.1.md"], "nothing is moved twice");
    assert_eq!(names(root, "projects/acme/history/runs"), ["r1.md"]);
    assert!(vault.conflicts().is_empty(), "{:?}", vault.conflicts());
}

#[test]
fn what_an_older_ledger_still_writes_to_the_old_places_is_moved_in_only_when_asked() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    vault_of_format_1(root);
    upgrade::vault(root).unwrap();
    let (vault, _) = open(root);
    assert_eq!(upgrade::leftovers(vault.store()), 0);
    drop(vault);

    // A Ledger 1.0 that had the vault open on another computer all along: it saw no versions any more, published the
    // plan as v0.1 again, and ran a prompt.
    write(root, "projects/acme/versions/plan/plan@v0.1.md", "# Plan\n\ntwo\n");
    write(root, "projects/acme/threads/call/runs/r2.md", &RUN_V1.replace("Wrote the plan.", "Wrote it again."));

    // Opening the vault leaves them where they are, and only counts them, for Settings > Vault.
    let before = snapshot(root);
    let (mut vault, conn) = open(root);
    assert_eq!(snapshot(root), before, "opening moves nothing");
    assert_eq!(upgrade::leftovers(vault.store()), 2);
    assert_eq!(records::for_prompt(&conn, "acme", "call", "p1").unwrap().len(), 1, "not read until they are moved");

    // The user moves them in.
    let moved = upgrade::tidy(vault.store()).unwrap();
    assert_eq!((moved.versions, moved.runs, moved.kept_apart), (0, 1, 1));
    let conn = vault.load().unwrap();
    assert_eq!(upgrade::leftovers(vault.store()), 0);
    assert!(!root.join("projects/acme/versions").exists() && !root.join("projects/acme/threads/call/runs").exists());
    assert!(read(root, "projects/acme/history/runs/r2.md").starts_with("---\nthread: call\n"));
    assert_eq!(records::for_prompt(&conn, "acme", "call", "p1").unwrap().len(), 2);
    // The plan's v0.1 is the one there was; the other is kept beside it, for the Conflicts page.
    assert_eq!(read(root, "projects/acme/history/deliverables/plan/plan@v0.1.md"), "# Plan\n\none\n");
    let conflicts = vault.conflicts();
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].original, "projects/acme/history/deliverables/plan/plan@v0.1.md");
    assert_eq!(read(root, &conflicts[0].copy), "# Plan\n\ntwo\n");
}

#[test]
fn a_run_stays_when_its_thread_is_deleted_as_the_versions_it_made_do() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    marker::ensure(root);
    let (mut vault, conn) = open(root);
    let project = crate::projects::create(&conn, "Acme").unwrap().slug;
    let thread = threads::create(&conn, &project, "Call").unwrap().slug;
    let prompt = prompts::create_titled(&conn, &project, &thread, "Plan", "Write the plan.", None).unwrap();
    let run = RunRecord {
        id: "1763629200000-abcdefgh".into(),
        project: project.clone(),
        thread: thread.clone(),
        prompt_id: prompt.id.clone(),
        status: "finished".into(),
        model: "opus".into(),
        permission_mode: "acceptEdits".into(),
        started_at: "2026-11-20T09:00:00.000Z".into(),
        finished_at: "2026-11-20T09:01:00.000Z".into(),
        seconds: 60,
        turns: Some(3),
        files: vec!["plan@v0.1.md".into()],
        body: "Wrote the plan.\n".into(),
    };
    records::insert(&conn, &run).unwrap();
    vault.persist(&conn).unwrap();
    let file = format!("projects/{project}/history/runs/{}.md", run.id);
    assert!(read(root, &file).starts_with(&format!("---\nthread: {thread}\nprompt: {}\n", prompt.id)));

    threads::delete(&conn, root, &project, &thread).unwrap();
    vault.persist(&conn).unwrap();
    assert!(!root.join(format!("projects/{project}/threads/{thread}")).exists());
    assert!(root.join(&file).is_file(), "the record stays");
    let (_vault, conn) = open(root);
    assert_eq!(records::for_prompt(&conn, &project, &thread, &prompt.id).unwrap().len(), 1, "and reads back in, its thread gone");
    assert!(posts::list(&conn, &project, &thread).unwrap().is_empty());
}

#[test]
fn an_encrypted_vault_of_format_1_is_upgraded_sealed_once_the_user_chooses_to() {
    const PASSPHRASE: &str = "correct horse battery staple";
    let place = tempdir().unwrap();
    let config = tempdir().unwrap();
    let root = place.path().join("Secret");
    let project = "acme";

    // An encrypted vault as Ledger 1.0 left it: its files where format 1 kept them, sealed, and a marker that says
    // format 1. This machine doesn't have its key yet.
    let (mut old, key) = marker::create_encrypted(&root, PASSPHRASE).unwrap();
    let sealed = store::Store::sealed(root.clone(), std::sync::Arc::new(crate::vault_crypto::Sealed::new(std::sync::Arc::new(key), &old.id)));
    sealed.write(&format!("projects/{project}/project.md"), b"---\nname: Acme\n---\n").unwrap();
    sealed.write(&format!("projects/{project}/threads/call/thread.md"), b"---\ntitle: Call\n---\n").unwrap();
    sealed.write(&format!("projects/{project}/versions/plan/plan@v0.1.md"), b"# Plan\n\none\n").unwrap();
    sealed.write(&format!("projects/{project}/threads/call/runs/r1.md"), RUN_V1.as_bytes()).unwrap();
    sealed.write(&format!("projects/{project}/threads/call/posts/n1.md"), NOTE_V1.as_bytes()).unwrap();
    drop(sealed);
    old.ledger = 1;
    marker::write(&root, &old).unwrap();

    // Not opened, and not changed, until the user chooses to upgrade it.
    let before = snapshot(&root);
    assert!(Core::open_at(root.clone(), config.path().to_path_buf(), false).is_err());
    assert_eq!(snapshot(&root), before);

    // Upgrading it takes its key: without it, nothing happens.
    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    assert!(matches!(core.upgrade_vault(&root), Err(crate::error::Error::Locked(_))));
    assert_eq!(snapshot(&root), before);
    let key = crate::vault_crypto::unlock(old.encryption.as_ref().unwrap(), &old.id, PASSPHRASE).unwrap();
    crate::vault_crypto::remember(&old.id, key, true);

    core.upgrade_vault(&root).unwrap();
    assert_eq!(format_of(&root), marker::FORMAT);
    let sealed = store::at(&root);
    assert!(!sealed.exists(&format!("projects/{project}/versions/plan/plan@v0.1.md")));
    assert_eq!(sealed.read(&format!("projects/{project}/history/deliverables/plan/plan@v0.1.md")).unwrap(), b"# Plan\n\none\n");
    assert!(sealed.read_string(&format!("projects/{project}/history/runs/r1.md")).unwrap().starts_with("---\nthread: call\n"));
    assert!(sealed.read_string(&format!("projects/{project}/threads/call/posts/n1.md")).unwrap().contains("../../../history/deliverables/plan/"));
    assert_eq!(records::for_prompt(&core.conn, &project, "call", "p1").unwrap().len(), 1);
    let on_disk: Vec<String> = fs::read_dir(&root).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(on_disk.iter().all(|name| name == "ledger.json" || name == "data"), "still nothing readable on disk: {on_disk:?}");
}

#[test]
fn only_a_vault_of_todays_format_opens_and_an_older_one_can_be_upgraded() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    for format in 0..=3 {
        write(root, "ledger.json", &format!("{{ \"ledger\": {format}, \"id\": \"{ID}\" }}"));
        let found = marker::inspect(root);
        let expected = match format {
            0 => matches!(found, Found::Damaged(_)),
            1 => matches!(found, Found::Older(_)),
            2 => matches!(found, Found::Vault(_)),
            _ => found == Found::Newer(3),
        };
        assert!(expected, "format {format}: {found:?}");
        assert_eq!(marker::refusal(root, &found).is_none(), format == 2, "format {format}");
    }
    assert!(marker::refusal(root, &Found::Newer(3)).unwrap().to_string().contains("Update Ledger"));
    assert!(upgrade::vault(root).is_err(), "a vault of a newer format isn't upgraded, or changed");
}
