//! The contract with margin-code, tested against margin-code's own output: the fixture vault its writer made
//! (`tests/fixtures/margin/vault`, copied verbatim from openledger-nl/margin; its README says how to refresh it, and
//! `CONTENTS.md` next to it what each file holds). Ledger must read every file as margin-code means it, and never
//! write any of them.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{TempDir, tempdir};

use crate::models::{ReviewCommentView, ReviewView};
use crate::posts::AddOptions;
use crate::vault::{self, Vault};
use crate::{activity, posts, projects, search, threads};

const PROJECT: &str = "fixture";
const THREAD: &str = "margin-fixture-feat-review-me";
const ROUND_1: &str = "1794214922674-9dxzhga6";
const ROUND_2: &str = "1794215536044-sk7lhuov";
/// The commits of the fixture's repository: main, and three on feat/review-me.
const BASE: &str = "3e0720893e748ccf801b960ac2b7a3b6f89871ce";
const H1: &str = "05ee15032ecfc5842073d96f0d0dea42593c5761";
const H2: &str = "83a8485b6c68cbd327062d8b7e7278654e14f960";
const H3: &str = "704ebb481cfb21b951b1180f0cc559e989cb4a44";

/// Every file under `dir`, by its path relative to it (with `/`), as bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, rel: &str, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir.join(rel)).unwrap().map(Result::unwrap) {
            let name = entry.file_name().into_string().unwrap();
            let child = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            if entry.file_type().unwrap().is_dir() {
                walk(dir, &child, out);
            } else {
                out.insert(child, fs::read(entry.path()).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, "", &mut out);
    out
}

/// A copy of the fixture, and its files as committed. The committed fixture itself is never opened as a vault:
/// loading one writes what it repairs (a project's missing README).
fn copy_of_fixture() -> (TempDir, BTreeMap<String, Vec<u8>>) {
    let fixture: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/margin/vault");
    let original = files(&fixture);
    assert_eq!(original.len(), 13, "the fixture as CONTENTS.md lists it");
    assert!(original.values().all(|bytes| !bytes.contains(&b'\r')), "the fixture is checked out as margin-code wrote it, LF");
    let dir = tempdir().unwrap();
    for (rel, bytes) in &original {
        let file = dir.path().join(rel);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
    (dir, original)
}

fn open(root: &Path) -> (Vault, rusqlite::Connection) {
    let mut vault = Vault::open(root.to_path_buf()).unwrap();
    let conn = vault.load().unwrap();
    (vault, conn)
}

fn comment<'a>(review: &'a ReviewView, path: &str, id: &str) -> &'a ReviewCommentView {
    let file = review.files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("{path} in {}", review.id));
    file.comments.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("{id} first in a conversation on {path}"))
}

#[test]
fn margin_codes_fixture_reads_as_it_means() {
    let (dir, _) = copy_of_fixture();
    let root = dir.path();
    let (_vault, conn) = open(root);

    // The project (Ledger's, by hand) and the thread margin-code made, its title double-quoted for its ": ".
    assert_eq!(projects::get(&conn, PROJECT).unwrap().name, "Fixture project");
    let thread = threads::get(&conn, root, PROJECT, THREAD).unwrap();
    assert_eq!((thread.title.as_str(), thread.created_at.as_str(), thread.description.as_str()), ("margin-fixture: feat/review-me", "2026-11-09T09:01:01.337Z", ""));
    let ids: Vec<&str> = thread.reviews.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, [ROUND_1, ROUND_2], "both reviews, oldest first");
    let [one, two] = thread.reviews.as_slice() else { unreachable!() };

    // ---- round 1: submitted, changes requested, base..h1 ----
    assert_eq!((one.format, one.newer_format), (1, false));
    assert_eq!((one.repo.as_str(), one.repo_name.as_str()), ("github.com/openledger-nl/margin-fixture", "margin-fixture"));
    assert_eq!((one.base.as_str(), one.base_ref.as_deref(), one.head.as_str(), one.head_ref.as_deref()), (BASE, Some("main"), H1, Some("feat/review-me")));
    assert_eq!((one.state.as_str(), one.verdict.as_deref()), ("submitted", Some("changes")));
    assert_eq!(one.author, "Fixture Reviewer");
    assert_eq!(
        (one.created_at.as_str(), one.updated_at.as_str(), one.submitted_at.as_deref()),
        ("2026-11-09T09:02:02.674Z", "2026-11-09T09:11:14.707Z", Some("2026-11-09T09:11:14.707Z"))
    );
    assert_eq!(
        one.summary,
        "Two things before this goes in:\n\n- keep `legacyTotal`, or say that removing it breaks callers\n- decide what `average([])` returns"
    );
    assert!(one.summary_html.contains("<li>keep <code>legacyTotal</code>"), "{}", one.summary_html);
    assert_eq!((one.comment_count, one.open_count), (6, 3), "six comments; open: the range, the whole file, the removed line");
    let paths: Vec<&str> = one.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["src/average.js", "src/legacy.js", "src/total.js"]);

    // One line of the new side, with the lines around it (`code: |2` and `codeAfter: |2`: the indentation indicator
    // keeps their leading spaces); resolved; a reply, and a reply to that reply, both under it.
    let line = comment(one, "src/total.js", "1794214984011-54fzye5x");
    assert_eq!((line.side.as_str(), line.line, line.end_line, line.commit.as_deref()), ("new", Some(3), Some(3), Some(H1)));
    assert_eq!(line.code.as_deref(), Some("  if (!Array.isArray(xs)) throw new TypeError('xs must be an array');\n"));
    assert_eq!(line.code_before.as_deref(), Some("// Sums a list of numbers.\nexport function total(xs) {\n"));
    assert_eq!(line.code_after.as_deref(), Some("  let sum = 0;\n  for (const x of xs) sum += x;\n  return sum;\n"));
    assert!(line.resolved);
    assert_eq!((line.created_at.as_str(), line.updated_at.as_str()), ("2026-11-09T09:03:04.011Z", "2026-11-09T09:06:08.022Z"));
    assert_eq!(line.body, "Throwing here breaks callers that pass any other iterable. Accept iterables?");
    let replies: Vec<(&str, &str, Option<&str>)> = line.replies.iter().map(|r| (r.id.as_str(), r.body.as_str(), r.path.as_deref())).collect();
    assert_eq!(
        replies,
        [
            ("1794215045348-40l1bt7b", "Nothing passes anything but arrays today.", None),
            ("1794215106685-id1g7ub4", "Then say so in a comment, and this is fine.", None),
        ],
        "the reply, and the reply to the reply, in the first comment's conversation"
    );
    assert!(line.replies.iter().all(|r| r.replies.is_empty() && r.author == "Fixture Reviewer"));

    // A removed line: the old side, numbered in the base commit.
    let removed = comment(one, "src/legacy.js", "1794215229359-ts8cm6e7");
    assert_eq!((removed.side.as_str(), removed.line, removed.end_line, removed.commit.as_deref()), ("old", Some(2), Some(2), Some(BASE)));
    assert_eq!(removed.code.as_deref(), Some("export function legacyTotal(xs) {\n"));
    assert_eq!(removed.code_before.as_deref(), Some("// Kept for old callers.\n"));
    assert_eq!(removed.code_after.as_deref(), Some("  return xs.reduce((a, b) => a + b, 0);\n}\n"));
    assert!(!removed.resolved);
    assert_eq!(removed.body, "Who still calls `legacyTotal`? Removing it breaks them.");

    // On the whole file first (no line, endLine or code, but a commit and a side), then a range of lines, whose file
    // also holds a key another tool added (`reviewTool`, with a YAML comment): passed over.
    let [whole, range] = one.files[0].comments.as_slice() else { panic!("two conversations on average.js") };
    assert_eq!(whole.id, "1794215413370-xpfjcd04");
    assert_eq!((whole.line, whole.end_line, whole.code.as_deref(), whole.code_before.as_deref(), whole.code_after.as_deref()), (None, None, None, None, None));
    assert_eq!((whole.side.as_str(), whole.commit.as_deref(), whole.resolved), ("new", Some(H1), false));
    assert_eq!(whole.body, "This file needs a test.");
    assert_eq!(range.id, "1794215290696-hqvrfpxw");
    assert_eq!((range.side.as_str(), range.line, range.end_line, range.commit.as_deref()), ("new", Some(4), Some(7), Some(H1)));
    assert_eq!(
        range.code.as_deref(),
        Some("export function average(xs) {\n  if (xs.length === 0) return 0;\n  return total(xs) / xs.length;\n}\n")
    );
    assert_eq!(range.code_before.as_deref(), Some("import { total } from './total.js';\n\n// The mean of a list of numbers.\n"));
    assert_eq!(range.code_after, None);
    assert_eq!((range.format, range.newer_format, range.resolved), (1, false, false));
    assert_eq!(range.updated_at, "2026-11-09T09:09:12.033Z");
    assert_eq!(range.body, "`average([])` returning 0 hides bad input. `NaN`, or throw?");

    // ---- round 2: a draft with no summary yet, whose head moved from h2 to h3 ----
    assert_eq!((two.state.as_str(), two.verdict.as_deref(), two.submitted_at.as_deref(), two.summary.as_str()), ("draft", None, None, ""));
    assert_eq!((two.base.as_str(), two.head.as_str(), two.head_ref.as_deref()), (BASE, H3, Some("feat/review-me")));
    assert_eq!((two.created_at.as_str(), two.updated_at.as_str()), ("2026-11-09T09:12:16.044Z", "2026-11-09T09:14:18.718Z"));
    assert_eq!((two.comment_count, two.open_count), (3, 3));
    let paths: Vec<&str> = two.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["src/average.test.js", "src/total.js"]);

    // Made after the move, on the head; `codeBefore: |+` keeps the blank line that ends it.
    let after = comment(two, "src/average.test.js", "1794215720055-4sg9yx7g");
    assert_eq!((after.line, after.end_line, after.commit.as_deref()), (Some(4), Some(4), Some(H3)));
    assert_eq!(after.code.as_deref(), Some("assert.equal(average([2, 4]), 3);\n"));
    assert_eq!(after.code_before.as_deref(), Some("import assert from 'node:assert/strict';\nimport { average } from './average.js';\n\n"));
    assert_eq!(after.code_after.as_deref(), Some("assert.ok(Number.isNaN(average([])));\n"));
    assert_eq!(after.body, "Add a case with one element.");

    // On total.js: the format 2 comment (lines 1-3) first, then the one made on h2 (line 6).
    let ids: Vec<&str> = two.files[1].comments.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["1794215781392-hw6o8g33", "1794215597381-cc4ep86x"]);

    // Made before the move: its lines are h2's, not the head's, and are kept as they are.
    let before = comment(two, "src/total.js", "1794215597381-cc4ep86x");
    assert_eq!((before.line, before.end_line, before.commit.as_deref()), (Some(6), Some(6), Some(H2)));
    assert_ne!(before.commit.as_deref(), Some(two.head.as_str()));
    assert_eq!(before.code.as_deref(), Some("  for (const x of xs) sum += x;\n"));
    assert_eq!(
        before.code_before.as_deref(),
        Some("  // Arrays only: nothing passes anything else.\n  if (!Array.isArray(xs)) throw new TypeError('xs must be an array');\n  let sum = 0;\n")
    );
    assert_eq!(before.code_after.as_deref(), Some("  return sum;\n}\n"));
    assert_eq!(before.body, "This loop could be `xs.reduce` now.");

    // Format 2, with a key format 1 lacks (`suggestion`): read as far as it goes, and flagged.
    let newer = comment(two, "src/total.js", "1794215781392-hw6o8g33");
    assert_eq!((newer.format, newer.newer_format), (2, true));
    assert!(!two.newer_format, "the review itself is format 1");
    assert_eq!((newer.line, newer.end_line, newer.commit.as_deref(), newer.resolved), (Some(1), Some(3), Some(H3), false));
    assert_eq!(newer.code.as_deref(), Some("/**\n * The sum of a list of numbers.\n */\n"));
    assert_eq!(newer.updated_at, "2026-11-09T09:16:21.392Z");
    assert_eq!(newer.body, "One line is enough for this comment.");
}

#[test]
fn ledger_never_rewrites_margin_codes_files() {
    let (dir, original) = copy_of_fixture();
    let root = dir.path();
    let (mut vault, conn) = open(root);

    assert!(vault::export(&conn).unwrap().keys().all(|p| !p.contains("/reviews/")), "no review file is the export's");
    // Work in the thread, and elsewhere, then read it all in again.
    posts::add(&conn, PROJECT, THREAD, "Read the second round.", AddOptions::default()).unwrap();
    let other = threads::create(&conn, PROJECT, "Other").unwrap().slug;
    vault.persist(&conn).unwrap();
    threads::delete(&conn, root, PROJECT, &other).unwrap();
    vault.persist(&conn).unwrap();
    let conn = vault.load().unwrap();
    vault.persist(&conn).unwrap();
    assert!(!vault.changed_on_disk());

    let now = files(root);
    for (rel, bytes) in &original {
        assert!(now.get(rel) == Some(bytes), "{rel} is byte for byte as margin-code wrote it");
    }
    // (besides the vault's own .gitignore and .ledger/)
    let added: Vec<&str> = now.keys().filter(|rel| !original.contains_key(*rel) && !rel.starts_with('.')).map(String::as_str).collect();
    assert_eq!(added.len(), 2, "only Ledger's own files were added: {added:?}");
    assert!(added.contains(&"projects/fixture/README.md") && added.iter().any(|rel| rel.contains(&format!("/{THREAD}/posts/"))));
    assert_eq!(threads::get(&conn, root, PROJECT, THREAD).unwrap().reviews.len(), 2);
}

#[test]
fn the_fixture_is_found_by_search_and_counted_once_submitted() {
    let (dir, _) = copy_of_fixture();
    let (_vault, conn) = open(dir.path());

    let mut hits: Vec<(String, String)> = search::search(&conn, "legacyTotal").unwrap().into_iter().map(|h| (h.kind, h.id)).collect();
    hits.sort();
    assert_eq!(
        hits,
        [("review".to_string(), ROUND_1.to_string()), ("review-comment".to_string(), "1794215229359-ts8cm6e7".to_string())],
        "the summary and the comment on the removed line"
    );

    let reviews: Vec<String> = activity::list(&conn).unwrap().into_iter().filter(|e| e.kind == "review").map(|e| e.at).collect();
    assert_eq!(reviews, ["2026-11-09T09:11:14.707Z"], "round 1, when it was submitted; not the draft");
}
