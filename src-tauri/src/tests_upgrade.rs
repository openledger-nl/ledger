//! Bringing a vault of an older format up to today's (see `upgrade`): format 1 kept a project's versions in
//! `versions/` and each run's record in its thread; format 2 keeps both in the project's `history/`.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use rusqlite::Connection;
use tempfile::tempdir;

use crate::claude_code::records::{self, RunRecord};
use crate::marker::{self, Found};
use crate::state::Core;
use crate::vault::Vault;
use crate::{posts, prompts, provenance, store, threads};

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
    match marker::inspect(root) {
        Found::Vault(marker) => marker.ledger,
        other => panic!("{other:?}"),
    }
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
fn a_vault_of_format_1_keeps_its_history_in_one_folder_once_it_is_opened() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    vault_of_format_1(root);
    let made = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    fs::File::options().write(true).open(root.join("projects/acme/versions/chart/chart@v0.1.png")).unwrap().set_modified(made).unwrap();
    assert_eq!(format_of(root), 1, "a vault of format 1 is one this Ledger opens");

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

    // Opening it again changes nothing.
    let before: Vec<(String, String)> = ["history/runs/r1.md", "threads/call/posts/n1.md", "threads/call/prompts/p1.md"]
        .iter()
        .map(|rel| (rel.to_string(), read(root, &format!("{project}/{rel}"))))
        .collect();
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

    let (vault, _conn) = open(root);
    assert_eq!(format_of(root), marker::FORMAT);
    assert!(!root.join("projects/acme/versions").exists() && !root.join("projects/acme/threads/call/runs").exists());
    assert_eq!(names(root, "projects/acme/history/deliverables/plan"), ["plan@v0.1.manifest.md", "plan@v0.1.md"], "nothing is moved twice");
    assert_eq!(names(root, "projects/acme/history/runs"), ["r1.md"]);
    assert!(vault.conflicts().is_empty(), "{:?}", vault.conflicts());
}

#[test]
fn what_an_older_ledger_still_writes_to_the_old_places_is_moved_in() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    vault_of_format_1(root);
    drop(open(root));
    assert_eq!(format_of(root), marker::FORMAT);

    // A Ledger 1.0 that had the vault open on another computer all along: it saw no versions any more, published the
    // plan as v0.1 again, and ran a prompt.
    write(root, "projects/acme/versions/plan/plan@v0.1.md", "# Plan\n\ntwo\n");
    write(root, "projects/acme/threads/call/runs/r2.md", &RUN_V1.replace("Wrote the plan.", "Wrote it again."));

    let (vault, conn) = open(root);
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
fn an_encrypted_vault_of_format_1_is_brought_up_to_date_sealed() {
    let place = tempdir().unwrap();
    let config = tempdir().unwrap();
    let mut core = Core::without_vault(config.path().to_path_buf()).unwrap();
    let (root, _) = core.create_vault(place.path(), "Secret", Some("correct horse battery staple")).unwrap();
    let project = crate::projects::create(&core.conn, "Acme").unwrap().slug;
    threads::create(&core.conn, &project, "Call").unwrap();
    core.vault.persist(&core.conn).unwrap();

    // Files where format 1 kept them, sealed, and a marker that says format 1.
    let sealed = store::at(&root);
    sealed.write(&format!("projects/{project}/versions/plan/plan@v0.1.md"), b"# Plan\n\none\n").unwrap();
    sealed.write(&format!("projects/{project}/threads/call/runs/r1.md"), RUN_V1.as_bytes()).unwrap();
    sealed.write(&format!("projects/{project}/threads/call/posts/n1.md"), NOTE_V1.as_bytes()).unwrap();
    let Found::Vault(mut old) = marker::inspect(&root) else { panic!() };
    old.ledger = 1;
    marker::write(&root, &old).unwrap();
    drop(core);

    let core = Core::open_at(root.clone(), config.path().to_path_buf(), false).unwrap();
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
fn only_formats_this_ledger_knows_are_opened() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    for (format, opens) in [(0, false), (1, true), (2, true), (3, false)] {
        write(root, "ledger.json", &format!("{{ \"ledger\": {format}, \"id\": \"{ID}\" }}"));
        let found = marker::inspect(root);
        assert_eq!(matches!(found, Found::Vault(_)), opens, "format {format}: {found:?}");
    }
    assert_eq!(marker::inspect(root), Found::Newer(3));
    assert!(marker::refusal(root, &Found::Newer(3)).unwrap().to_string().contains("Update Ledger"));
}
