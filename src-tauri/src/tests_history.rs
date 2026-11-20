//! What changed between the versions of a deliverable, who wrote what is in it, and the history notes in the vault.

use std::fs;

use rusqlite::Connection;
use tempfile::tempdir;

use crate::models::{Bump, PublishChoices};
use crate::vault::{self, Vault};
use crate::{db, deliverables, history, projects, prompts, threads};

/// A project with "plan.md": v0.1 typed by you, v0.2 written with AI from a prompt, and a live file you edited since.
fn setup() -> (Connection, String) {
    let c = db::open_in_memory().unwrap();
    let p = projects::create(&c, "Acme").unwrap().slug;
    let t = threads::create(&c, &p, "Kick-off").unwrap().slug;
    let prompt = prompts::create_titled(&c, &p, &t, "Write the risks", "Add the risks.", None).unwrap();
    deliverables::create(&c, &p, "Plan", "").unwrap();
    let root = tempdir().unwrap();
    let publish = |text: &str, choices: PublishChoices| {
        deliverables::save(&c, &p, "plan.md", text).unwrap();
        deliverables::publish_with(&c, root.path(), &p, "plan.md", Bump::Minor, None, &choices).unwrap();
    };
    publish("# Plan\n\nWe build a small app for the shop.\n", PublishChoices::default());
    publish(
        "# Plan\n\nWe build a small app for the shop.\n\n## Risks\n\nThe deadline is tight and the budget is small.\n",
        PublishChoices { ai: true, prompt: Some(prompt.id.clone()), ..Default::default() },
    );
    deliverables::save(&c, &p, "plan.md", "# Plan\n\nWe build a large app for the shop.\n\n## Risks\n\nThe deadline is tight.\n").unwrap();
    (c, p)
}

#[test]
fn two_versions_compare_word_by_word_with_who_added_what() {
    let (c, p) = setup();
    let cmp = history::compare(&c, &p, "plan.md", "0.1", "0.2").unwrap();
    assert_eq!((cmp.from.as_deref(), cmp.to.as_deref()), (Some("0.1"), Some("0.2")));
    let states: Vec<&str> = cmp.blocks.iter().map(|b| b.state).collect();
    assert_eq!(states, ["same", "same", "added", "added"]);
    assert_eq!((cmp.words_added, cmp.words_removed), (10, 0));
    // All of it by the AI of v0.2.
    assert_eq!(cmp.added_by.len(), 1);
    let source = &cmp.sources[cmp.added_by[0].source as usize];
    assert_eq!((source.version.as_deref(), source.by, source.prompt.as_deref()), (Some("0.2"), "ai", Some("Write the risks")));
    assert!(cmp.blocks[3].html.contains("<ins class=\"dv-ins\" data-by=\"ai\""), "{}", cmp.blocks[3].html);
    // And line by line, as git shows it.
    assert!(cmp.hunks.iter().flat_map(|h| &h.lines).any(|l| l.tag == "add" && l.parts.iter().any(|x| x.text.contains("deadline"))));
}

#[test]
fn the_live_file_compares_with_any_version_and_keeps_whose_words_they_are() {
    let (c, p) = setup();
    let cmp = history::compare(&c, &p, "plan.md", "0.1", "live").unwrap();
    assert_eq!(cmp.to, None);
    let by: Vec<&str> = cmp.added_by.iter().map(|s| cmp.sources[s.source as usize].by).collect();
    assert_eq!(by, ["ai", "human"], "the risks by the AI, \"large\" by you");
    let changed = cmp.blocks.iter().find(|b| b.state == "changed").unwrap();
    assert!(changed.html.contains("<del class=\"dv-del\">small</del>") && changed.html.contains(">large</ins>"), "{}", changed.html);
    assert!(history::compare(&c, &p, "plan.md", "0.9", "live").is_err());
}

#[test]
fn who_wrote_what_and_where_the_ai_s_words_went() {
    let (c, p) = setup();
    let all = history::authors(&c, &p, "plan.md", "live", None).unwrap();
    let words = |by: &str| all.words.iter().filter(|s| all.sources[s.source as usize].by == by).map(|s| s.words).sum::<usize>();
    assert_eq!((words("human"), words("ai")), (9, 5));

    // v0.2's AI wrote ten words; you took five of them out since.
    let traced = history::authors(&c, &p, "plan.md", "live", Some("0.2")).unwrap();
    let trace = traced.trace.unwrap();
    assert_eq!((trace.wrote, trace.kept), (10, 5));
    assert_eq!(trace.removed_by.iter().map(|s| (traced.sources[s.source as usize].version.clone(), s.words)).collect::<Vec<_>>(), [(None, 5)]);
    // Only its words are marked.
    let marked: Vec<_> = traced.blocks.iter().filter(|b| b.html.contains("dv-by")).collect();
    assert_eq!(marked.len(), 2);
    assert!(history::authors(&c, &p, "plan.md", "0.1", Some("0.2")).is_err(), "v0.2 came after v0.1");
}

#[test]
fn each_version_gets_a_note_in_the_history_folder_that_obsidian_can_read() {
    let (c, p) = setup();
    let files = vault::export(&c).unwrap();
    let first = &files[&format!("projects/{p}/history/plan/v0.1.md")];
    assert!(first.contains("madeBy: You") && first.contains("The first version, 9 words: [plan@v0.1.md](../../versions/plan@v0.1.md)"), "{first}");
    let second = &files[&format!("projects/{p}/history/plan/v0.2.md")];
    assert!(second.contains("by: ai\nmadeBy: AI, as declared\n") && second.contains("prompt: Write the risks"), "{second}");
    assert!(second.contains("> +10 words, -0 words since [v0.1](v0.1.md)."), "{second}");
    assert!(second.contains("## What changed\n\n## ==Risks==\n\n==The deadline is tight and the budget is small.==\n"), "{second}");
    // The live file is no version: it has no note.
    assert_eq!(files.keys().filter(|k| k.contains("/history/")).count(), 2);
}

#[test]
fn history_notes_are_written_but_never_read_back() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let mut vault = Vault::open(root.to_path_buf()).unwrap();
    let c = vault.load().unwrap();
    let p = projects::create(&c, "Acme").unwrap().slug;
    deliverables::create(&c, &p, "Plan", "# Plan\n\nOne.\n").unwrap();
    deliverables::publish_with(&c, root, &p, "plan.md", Bump::Minor, None, &PublishChoices::default()).unwrap();
    vault.persist(&c).unwrap();
    let note = root.join(format!("projects/{p}/history/plan/v0.1.md"));
    assert!(note.exists());

    // Reloaded, the vault is what it was; a note that was deleted by hand is written again.
    fs::remove_file(&note).unwrap();
    let c = vault.load().unwrap();
    assert!(note.exists());
    assert_eq!(deliverables::list(&c, &p).unwrap().len(), 1);
    assert_eq!(threads::list(&c, &p).unwrap().len(), 0);
}

/// `text` with `edits` made, positions in UTF-16 as the editor counts.
fn apply(text: &str, edits: &[history::Edit]) -> String {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    let mut edits = edits.to_vec();
    edits.sort_by_key(|e| std::cmp::Reverse(e.from));
    for e in edits {
        units.splice(e.from..e.to, e.insert.encode_utf16());
    }
    String::from_utf16(&units).unwrap()
}

#[test]
fn the_text_being_typed_shows_each_change_since_the_last_version_and_how_to_undo_it() {
    let (c, p) = setup();
    let live = "# Plan \u{1F331}\n\nWe build a large app for the shop.\n\n## Risks\n\nThe deadline is tight.\n\nA new closing line.\n";
    let got = history::live_changes(&c, &p, "plan.md", live, None).unwrap();
    assert_eq!(got.against.as_deref(), Some("0.2"));
    let kinds: Vec<&str> = got.changes.iter().map(|ch| ch.kind).collect();
    assert_eq!(kinds, ["changed", "changed", "changed", "added"]);
    // What was added, typed now, by you; the risk that was only cut short says nothing of who cut it.
    let by: Vec<Option<bool>> = got.changes.iter().map(|ch| ch.by.map(|b| got.sources[b as usize].version.is_none())).collect();
    assert_eq!(by, [Some(true), Some(true), None, Some(true)]);

    // Positions count as the editor does: the seedling before "large" is two units.
    let large = &got.changes[1];
    let units: Vec<u16> = live.encode_utf16().collect();
    let span = &large.inserts[0];
    assert_eq!(String::from_utf16(&units[span.from..span.to]).unwrap(), "large");
    assert_eq!(large.deletes[0].text, "small");

    // Undoing a change puts back what v0.2 said there; undoing all of them is v0.2.
    let v2 = "# Plan\n\nWe build a small app for the shop.\n\n## Risks\n\nThe deadline is tight and the budget is small.\n";
    assert_eq!(apply(live, &large.revert), live.replace("large", "small"));
    let all: Vec<history::Edit> = got.changes.iter().flat_map(|ch| ch.revert.clone()).collect();
    assert_eq!(apply(live, &all), v2);
}

#[test]
fn against_an_older_version_the_ai_s_changes_are_told_from_yours() {
    let (c, p) = setup();
    let live = "# Plan\n\nWe build a large app for the shop.\n\n## Risks\n\nThe deadline is tight.\n";
    let got = history::live_changes(&c, &p, "plan.md", live, Some("0.1")).unwrap();
    let by: Vec<(&str, &str)> = got.changes.iter().map(|ch| (ch.kind, got.sources[ch.by.unwrap() as usize].by)).collect();
    assert_eq!(by, [("changed", "human"), ("added", "ai"), ("added", "ai")]);
    // A paragraph taken out comes back where it was.
    let without = "# Plan\n\n## Risks\n\nThe deadline is tight and the budget is small.\n";
    let got = history::live_changes(&c, &p, "plan.md", without, None).unwrap();
    assert_eq!(got.changes.len(), 1);
    assert_eq!(got.changes[0].kind, "removed");
    assert_eq!(apply(without, &got.changes[0].revert), "# Plan\n\nWe build a small app for the shop.\n\n## Risks\n\nThe deadline is tight and the budget is small.\n");
}
