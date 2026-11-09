//! Code reviews written with margin-code: that their files load into the thread, that Ledger never writes them
//! (not on export, not on any change, not on a reload), and that they go only with their thread or project.

use std::fs;
use std::path::Path;

use rusqlite::Connection;
use tempfile::tempdir;

use crate::models::ReviewView;
use crate::posts::AddOptions;
use crate::vault::{self, Vault};
use crate::{posts, projects, threads};

const THREAD: &str = "projects/acme/threads/margin-code";
const REVIEW: &str = "1793873700123-k3j9x2ab";
const BASE: &str = "3f2a9c0d1e4b5a6978c0d1e2f3a4b5c6d7e8f901";
const HEAD: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";

fn write(root: &Path, rel: &str, text: &str) {
    let file = root.join(rel);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn open(root: &Path) -> (Vault, Connection) {
    let mut vault = Vault::open(root.to_path_buf()).unwrap();
    let conn = vault.load().unwrap();
    (vault, conn)
}

fn reviews(conn: &Connection, root: &Path, thread: &str) -> Vec<ReviewView> {
    threads::get(conn, root, "acme", thread).unwrap().reviews
}

/// A thread as margin-code makes one, holding a review in the agreed format with comments of every kind. Returns the
/// review's files, by path, as written.
fn margin_review(root: &Path) -> Vec<(String, String)> {
    let files = vec![
        (
            "projects/acme/project.md".to_string(),
            "---\nname: Acme\n---\n".to_string(),
        ),
        (
            format!("{THREAD}/thread.md"),
            "---\ntitle: Review margin code\ncreatedAt: 2026-11-09T10:15:00.000Z\n---\nThe code side of margin.\n".to_string(),
        ),
        (
            format!("{THREAD}/reviews/{REVIEW}.md"),
            format!(
                "---\nformat: 1\nrepo: github.com/openledger-nl/margin\nrepoName: margin\nbase: {BASE}\nbaseRef: main\nhead: {HEAD}\nheadRef: feat/code\n\
                 state: submitted\nverdict: changes\nauthor: Milan Molenbrugge\ncreatedAt: 2026-11-09T10:15:00.123Z\nupdatedAt: 2026-11-09T11:00:00.000Z\n\
                 submittedAt: 2026-11-09T11:00:00.000Z\nreviewedWith: margin-code 0.1.0   # a key Ledger doesn't know\n---\n\
                 Mostly fine. **Two** things to fix before it merges.\n"
            ),
        ),
        (
            // On lines 42-44 of the new side, with the code around it.
            format!("{THREAD}/reviews/{REVIEW}/1793873760000-aaaaaaaa.md"),
            format!(
                "---\nformat: 1\npath: browser/server/src/mcp.ts\nside: new\nline: 42\nendLine: 44\ncommit: {HEAD}\ncode: |\n  if (!token) {{\n    return null;\n  }}\n\
                 codeBefore: |\n  const token = read();\ncodeAfter: |\n  return token;\nresolved: false\nauthor: Milan Molenbrugge\n\
                 createdAt: 2026-11-09T10:16:00.000Z\nupdatedAt: 2026-11-09T10:16:00.000Z\n---\nThrow here instead of returning `null`.\n"
            ),
        ),
        (
            // A reply to it, which leaves out the place.
            format!("{THREAD}/reviews/{REVIEW}/1793873820000-bbbbbbbb.md"),
            "---\nformat: 1\nreplyTo: 1793873760000-aaaaaaaa\nauthor: Ana\ncreatedAt: 2026-11-09T10:17:00.000Z\nupdatedAt: 2026-11-09T10:17:00.000Z\n---\nAgreed, will do.\n"
                .to_string(),
        ),
        (
            // On the whole file, resolved.
            format!("{THREAD}/reviews/{REVIEW}/1793873880000-cccccccc.md"),
            "---\nformat: 1\npath: browser/server/src/mcp.ts\nside: new\nresolved: true\nauthor: Milan Molenbrugge\ncreatedAt: 2026-11-09T10:18:00.000Z\nupdatedAt: 2026-11-09T10:18:00.000Z\n---\nSplit this file up.\n"
                .to_string(),
        ),
        (
            // On a line that was removed: the old side, in the base commit.
            format!("{THREAD}/reviews/{REVIEW}/1793873940000-dddddddd.md"),
            format!(
                "---\nformat: 1\npath: README.md\nside: old\nline: 7\nendLine: 7\ncommit: {BASE}\ncode: |\n  Run it with npm.\nresolved: false\nauthor: Milan Molenbrugge\n\
                 createdAt: 2026-11-09T10:19:00.000Z\nupdatedAt: 2026-11-09T10:19:00.000Z\n---\nWhy did this go?\n"
            ),
        ),
    ];
    for (rel, text) in &files {
        write(root, rel, text);
    }
    files.into_iter().filter(|(rel, _)| rel.contains("/reviews/")).collect()
}

fn assert_untouched(root: &Path, files: &[(String, String)]) {
    for (rel, text) in files {
        assert_eq!(&read(root, rel), text, "{rel} is as margin-code wrote it");
    }
}

#[test]
fn a_review_from_margin_code_loads_into_its_thread() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    margin_review(root);
    let (_vault, conn) = open(root);

    let thread = threads::get(&conn, root, "acme", "margin-code").unwrap();
    assert_eq!(thread.title, "Review margin code", "the thread margin-code made is a thread like any other");
    assert_eq!(thread.description, "The code side of margin.");

    let [review] = thread.reviews.as_slice() else { panic!("one review: {:?}", thread.reviews) };
    assert_eq!(review.id, REVIEW);
    assert!(!review.newer_format);
    assert_eq!((review.repo.as_str(), review.repo_name.as_str()), ("github.com/openledger-nl/margin", "margin"));
    assert_eq!((review.base.as_str(), review.base_ref.as_deref()), (BASE, Some("main")));
    assert_eq!((review.head.as_str(), review.head_ref.as_deref()), (HEAD, Some("feat/code")));
    assert_eq!((review.state.as_str(), review.verdict.as_deref()), ("submitted", Some("changes")));
    assert_eq!(review.author, "Milan Molenbrugge");
    assert_eq!(review.created_at, "2026-11-09T10:15:00.123Z");
    assert_eq!(review.submitted_at.as_deref(), Some("2026-11-09T11:00:00.000Z"));
    assert!(review.summary_html.contains("<strong>Two</strong>"), "{}", review.summary_html);
    assert_eq!((review.comment_count, review.open_count), (4, 2), "four comments, two open conversations");

    // By file, in path order; on a file, the whole-file comment first, then by line.
    let files: Vec<&str> = review.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["README.md", "browser/server/src/mcp.ts"]);
    let removed = &review.files[0].comments[0];
    assert_eq!((removed.side.as_str(), removed.line, removed.end_line), ("old", Some(7), Some(7)));
    assert_eq!(removed.commit.as_deref(), Some(BASE));

    let [whole, lines] = review.files[1].comments.as_slice() else { panic!("two conversations on mcp.ts") };
    assert_eq!((whole.line, whole.resolved), (None, true));
    assert_eq!((lines.line, lines.end_line, lines.side.as_str(), lines.resolved), (Some(42), Some(44), "new", false));
    assert_eq!(lines.code.as_deref(), Some("if (!token) {\n  return null;\n}\n"));
    assert_eq!(lines.code_before.as_deref(), Some("const token = read();\n"));
    assert_eq!(lines.code_after.as_deref(), Some("return token;\n"));
    assert!(lines.html.contains("<code>null</code>"));
    let [reply] = lines.replies.as_slice() else { panic!("the reply sits under its comment") };
    assert_eq!((reply.author.as_str(), reply.body.as_str()), ("Ana", "Agreed, will do."));
    assert_eq!(reply.path, None, "a reply has no place of its own");
}

#[test]
fn ledger_never_writes_a_review_file() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let files = margin_review(root);
    let (mut vault, conn) = open(root);

    // Not part of the export, so nothing renders them, and the write-through has nothing to rewrite or delete.
    assert!(vault::export(&conn).unwrap().keys().all(|p| !p.contains("/reviews/")));
    assert!(!vault.changed_on_disk(), "loading wrote nothing over them");

    // Changes all around them: in their thread, to the thread itself, in another thread that is then deleted.
    posts::add(&conn, "acme", "margin-code", "A note next to the review.", AddOptions::default()).unwrap();
    threads::rename(&conn, "acme", "margin-code", "Review: margin code").unwrap();
    let other = threads::create(&conn, "acme", "Other").unwrap().slug;
    vault.persist(&conn).unwrap();
    threads::delete(&conn, root, "acme", &other).unwrap();
    vault.persist(&conn).unwrap();

    assert_untouched(root, &files);
    assert!(read(root, &format!("{THREAD}/thread.md")).contains("Review: margin code"), "the thread's own file is Ledger's");
    assert!(!vault.changed_on_disk());
}

#[test]
fn reviews_survive_a_reload_and_follow_margin_codes_edits() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let files = margin_review(root);
    let (mut vault, conn) = open(root);
    let before = format!("{:?}", reviews(&conn, root, "margin-code"));
    posts::add(&conn, "acme", "margin-code", "A note.", AddOptions::default()).unwrap();
    vault.persist(&conn).unwrap();

    let conn = vault.load().unwrap();
    assert_eq!(format!("{:?}", reviews(&conn, root, "margin-code")), before);
    assert_untouched(root, &files);

    // margin-code adds a reply, resolves the conversation and deletes a comment; the reload shows exactly that.
    let first = format!("{THREAD}/reviews/{REVIEW}/1793873760000-aaaaaaaa.md");
    write(root, &first, &read(root, &first).replace("resolved: false", "resolved: true"));
    write(
        root,
        &format!("{THREAD}/reviews/{REVIEW}/1793874000000-eeeeeeee.md"),
        "---\nformat: 1\nreplyTo: 1793873820000-bbbbbbbb\nauthor: Milan Molenbrugge\ncreatedAt: 2026-11-09T10:20:00.000Z\nupdatedAt: 2026-11-09T10:20:00.000Z\n---\nThanks.\n",
    );
    fs::remove_file(root.join(format!("{THREAD}/reviews/{REVIEW}/1793873940000-dddddddd.md"))).unwrap();
    assert!(vault.changed_on_disk());
    let conn = vault.load().unwrap();

    let [review] = reviews(&conn, root, "margin-code").try_into().unwrap();
    assert_eq!(review.files.len(), 1, "the comment on README.md is gone");
    let lines = &review.files[0].comments[1];
    assert!(lines.resolved);
    let replies: Vec<&str> = lines.replies.iter().map(|r| r.body.as_str()).collect();
    assert_eq!(replies, ["Agreed, will do.", "Thanks."], "a reply to a reply sits in the same conversation");
    assert_eq!(review.open_count, 0);
    assert!(!root.join(format!("{THREAD}/reviews/{REVIEW}/1793873940000-dddddddd.md")).exists(), "and nothing brought it back");
}

#[test]
fn reviews_go_with_their_thread_and_only_then() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    margin_review(root);
    // A second thread with a review of its own; and leftovers margin-code may have in a review folder: comments whose
    // review isn't there (yet), and a file that isn't markdown.
    let kept = margin_review_in(root, "projects/acme/threads/kept");
    write(root, &format!("{THREAD}/reviews/1793875000000-orphanzz/1793875000001-ffffffff.md"), "---\nformat: 1\n---\nA comment waiting for its review.\n");
    write(root, &format!("{THREAD}/reviews/notes.txt"), "margin-code's own");
    let (mut vault, conn) = open(root);
    assert_eq!(reviews(&conn, root, "margin-code").len(), 1, "comments without their review wait on disk");

    threads::delete(&conn, root, "acme", "margin-code").unwrap();
    vault.persist(&conn).unwrap();
    assert!(!root.join(THREAD).exists(), "the thread's folder went, reviews and all");
    assert_untouched(root, &kept);
    assert!(!vault.changed_on_disk());
    let conn = vault.load().unwrap();
    assert!(!threads::exists(&conn, "acme", "margin-code").unwrap(), "nothing left behind brings the thread back");
    assert_eq!(reviews(&conn, root, "kept").len(), 1);

    // A project that is deleted takes all of it.
    projects::delete(&conn, "acme").unwrap();
    vault.persist(&conn).unwrap();
    vault.remove_project_dir("acme").unwrap();
    assert!(!root.join("projects/acme").exists());
}

/// A review with one comment in the thread folder `dir`. Returns its files as written.
fn margin_review_in(root: &Path, dir: &str) -> Vec<(String, String)> {
    let files = vec![
        (format!("{dir}/thread.md"), "---\ntitle: Kept\ncreatedAt: 2026-11-08T09:00:00.000Z\n---\n".to_string()),
        (
            format!("{dir}/reviews/1793800000000-keptkept.md"),
            format!("---\nformat: 1\nrepo: git:{BASE}\nbase: {BASE}\nhead: {HEAD}\nstate: draft\nauthor: Ana\ncreatedAt: 2026-11-08T09:00:00.000Z\nupdatedAt: 2026-11-08T09:00:00.000Z\n---\n"),
        ),
        (
            format!("{dir}/reviews/1793800000000-keptkept/1793800000001-gggggggg.md"),
            "---\nformat: 1\npath: src/lib.rs\nline: 1\nendLine: 1\ncode: |\n  mod a;\nresolved: false\nauthor: Ana\ncreatedAt: 2026-11-08T09:01:00.000Z\nupdatedAt: 2026-11-08T09:01:00.000Z\n---\nWhy?\n"
                .to_string(),
        ),
    ];
    for (rel, text) in &files {
        write(root, rel, text);
    }
    files
}

#[test]
fn a_draft_without_a_remote_reads_as_in_progress() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "projects/acme/project.md", "---\nname: Acme\n---\n");
    margin_review_in(root, "projects/acme/threads/kept");
    let (_vault, conn) = open(root);
    let [review] = reviews(&conn, root, "kept").try_into().unwrap();
    assert_eq!((review.state.as_str(), review.verdict, review.submitted_at), ("draft", None, None));
    assert_eq!(review.repo, format!("git:{BASE}"));
    assert_eq!((review.base_ref, review.head_ref), (None, None));
    assert_eq!(review.summary, "", "a draft may have no summary yet");
}

#[test]
fn a_review_in_a_newer_format_is_shown_as_far_as_it_reads_and_left_alone() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "projects/acme/project.md", "---\nname: Acme\n---\n");
    let review = format!(
        "---\nformat: 2\nrepo: github.com/openledger-nl/margin\nrepoName: margin\nbase: {BASE}\nhead: {HEAD}\nstate: submitted\nverdict: approve\n\
         author: Ana\ncreatedAt: 2026-11-09T10:15:00.123Z\nupdatedAt: 2026-11-09T10:15:00.123Z\nsubmittedAt: 2026-11-09T10:15:00.123Z\n\
         reviewers:\n  - Ana\n  - Bo\n---\nShip it.\n"
    );
    let comment = "---\nformat: 3\npath: a.ts\nline: 3\nanchor: {kind: symbol, name: main}\nauthor: Ana\ncreatedAt: 2026-11-09T10:16:00.000Z\n---\nNice.\n";
    write(root, &format!("{THREAD}/reviews/{REVIEW}.md"), &review);
    write(root, &format!("{THREAD}/reviews/{REVIEW}/1793873760000-aaaaaaaa.md"), comment);
    let (mut vault, conn) = open(root);

    let [shown] = reviews(&conn, root, "margin-code").try_into().unwrap();
    assert!(shown.newer_format && shown.format == 2);
    assert_eq!((shown.verdict.as_deref(), shown.summary.as_str()), (Some("approve"), "Ship it."));
    let comment_view = &shown.files[0].comments[0];
    assert!(comment_view.newer_format);
    assert_eq!((comment_view.path.as_deref(), comment_view.line, comment_view.end_line), (Some("a.ts"), Some(3), Some(3)));
    assert_eq!(comment_view.updated_at, comment_view.created_at);

    posts::add(&conn, "acme", "margin-code", "A note.", AddOptions::default()).unwrap();
    vault.persist(&conn).unwrap();
    assert_eq!(read(root, &format!("{THREAD}/reviews/{REVIEW}.md")), review);
    assert_eq!(read(root, &format!("{THREAD}/reviews/{REVIEW}/1793873760000-aaaaaaaa.md")), comment);
}

#[test]
fn review_files_are_read_leniently() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write(root, "projects/acme/project.md", "---\nname: Acme\n---\n");
    let folder = format!("{THREAD}/reviews");
    // No frontmatter at all: a draft that is all summary, made when the file was.
    write(root, &format!("{folder}/1793873000000-nofrontm.md"), "Just some thoughts.\n");
    // A verdict on a draft is not one; a state in other letters is still read; no format is the first.
    write(root, &format!("{folder}/1793873100000-verdicts.md"), "---\nstate: draft\nverdict: approve\nrepo: gitlab.com/group/sub/tool\n---\n");
    write(root, &format!("{folder}/1793873200000-shouting.md"), "---\nformat: one\nstate: SUBMITTED\nverdict: Approve\nupdatedAt: 2026-11-09T12:00:00.000Z\n---\n");
    let comments = format!("{folder}/1793873200000-shouting");
    // A last line before the first, a path with backslashes, a line that isn't a number, `resolved` as text.
    write(root, &format!("{comments}/1.md"), "---\npath: src\\win\\file.rs\nline: 9\nendLine: 3\nresolved: 'true'\ncreatedAt: 2026-11-09T10:01:00.000Z\n---\nOne.\n");
    write(root, &format!("{comments}/2.md"), "---\npath: b.rs\nline: nine\nendLine: 12\ncreatedAt: 2026-11-09T10:02:00.000Z\n---\nTwo.\n");
    // A reply whose comment is gone stands on its own; replies that answer each other in a loop are each shown.
    write(root, &format!("{comments}/3.md"), "---\nreplyTo: gone\npath: c.rs\nline: 4\ncreatedAt: 2026-11-09T10:03:00.000Z\n---\nThree.\n");
    write(root, &format!("{comments}/4.md"), "---\nreplyTo: '5'\ncreatedAt: 2026-11-09T10:04:00.000Z\n---\nFour.\n");
    write(root, &format!("{comments}/5.md"), "---\nreplyTo: '4'\ncreatedAt: 2026-11-09T10:05:00.000Z\n---\nFive.\n");
    write(root, &format!("{comments}/6.md"), "---\nreplyTo: '6'\npath: d.rs\ncreatedAt: 2026-11-09T10:06:00.000Z\n---\nSix answers itself.\n");
    let (_vault, conn) = open(root);

    let all = reviews(&conn, root, "margin-code");
    let by_id = |id: &str| all.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("{id} loaded"));

    let bare = by_id("1793873000000-nofrontm");
    assert_eq!((bare.state.as_str(), bare.format, bare.summary.as_str()), ("draft", 1, "Just some thoughts."));
    assert!(bare.created_at.starts_with("20"), "dated by the file: {}", bare.created_at);
    assert_eq!(bare.updated_at, bare.created_at);

    let draft = by_id("1793873100000-verdicts");
    assert_eq!((draft.state.as_str(), draft.verdict.as_deref()), ("draft", None));
    assert_eq!(draft.repo_name, "tool", "a missing name is the repo's last part");

    let shouting = by_id("1793873200000-shouting");
    assert_eq!((shouting.state.as_str(), shouting.verdict.as_deref(), shouting.format), ("submitted", Some("approve"), 1));
    assert_eq!(shouting.submitted_at.as_deref(), Some("2026-11-09T12:00:00.000Z"), "submitted at its last change");

    let comments: Vec<(&str, &str, Option<i64>, Option<i64>, usize)> = shouting
        .files
        .iter()
        .flat_map(|f| f.comments.iter().map(move |c| (f.path.as_str(), c.body.as_str(), c.line, c.end_line, c.replies.len())))
        .collect();
    assert_eq!(
        comments,
        [
            ("", "Three.", None, None, 0),
            ("", "Four.", None, None, 0),
            ("", "Five.", None, None, 0),
            ("b.rs", "Two.", None, None, 0),
            ("d.rs", "Six answers itself.", None, None, 0),
            ("src/win/file.rs", "One.", Some(9), Some(9), 0),
        ]
    );
    assert!(shouting.files.last().unwrap().comments[0].resolved);
}
