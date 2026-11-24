//! Upgrades a vault of an older format to today's (see `marker`), and only ever when the user chooses to. Opening a
//! vault never changes its format: a vault of an older format isn't opened at all (`marker::Found::Older`), and the
//! vault picker says what upgrading it changes and leaves the choice to the user. Only its "Upgrade and open" runs
//! [`vault`].
//!
//! **Format 1 to 2: a project's history in one folder.** What happened in a project, frozen, used to be in two places:
//! the published versions in `versions/`, and the record of each Claude Code run in its thread's `runs/`, so a run went
//! when its thread was deleted while the versions it made stayed. Format 2 keeps both in the project's `history/`:
//!
//! ```text
//! projects/<p>/versions/<deliverable>/...       ->  projects/<p>/history/deliverables/<deliverable>/...
//! projects/<p>/threads/<thread>/runs/<id>.md    ->  projects/<p>/history/runs/<id>.md, which now names its thread
//! ```
//!
//! What pointed at the old places is pointed at the new ones: a "version published" note's link to its manifest, and the
//! version a prompt's feedback was given on ("### On v1.0 (versions/plan/plan@v1.0.md)"). Nothing is deleted.
//!
//! **Safe to stop, and to do again.** The files are moved first and the marker is written last, so an upgrade that
//! stops halfway (the app closed, a file a sync client wouldn't let go of) leaves a vault of the older format still:
//! the picker asks again, and upgrading it then carries on where it stopped. A file that is at its new place already,
//! with the same contents, is not moved twice; one that is there with other contents is kept beside it as a conflict
//! copy, so nothing is lost.
//!
//! **Other computers.** A Ledger of format 1 refuses a vault of format 2 when it opens it (`marker::Found::Newer`), but
//! one that had the vault open already writes to the old places until it is closed. What it writes there is left
//! where it is: [`leftovers`] counts it for Settings > Vault, and [`tidy`] moves it into `history/` in the same way
//! when the user asks.

use std::path::Path;

use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};

use crate::error::{Error, Result};
use crate::marker::{self, Found, Marker};
use crate::store::{self, Store};
use crate::vault::Vault;
use crate::{conflicts, deliverables, frontmatter, provenance, vault};

/// Where format 1 kept a project's published versions, in the project's folder.
const OLD_VERSIONS: &str = "versions";

/// Where format 1 kept the records of a thread's runs, in the thread's folder.
const OLD_RUNS: &str = "runs";

/// What moving files into `history/` did.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Moved {
    /// Files moved from `versions/` to `history/deliverables/`: versions, their manifests and whatever else was there.
    pub versions: usize,
    /// Run records moved from their thread to `history/runs/`.
    pub runs: usize,
    /// Files whose new place held other contents already: kept beside it as conflict copies.
    pub kept_apart: usize,
    /// "Version published" notes and prompts whose links now point at the new places.
    pub relinked: usize,
}

impl std::fmt::Display for Moved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} files of versions and {} run records moved into history/, {} notes and prompts relinked, {} kept as conflict copies",
            self.versions, self.runs, self.relinked, self.kept_apart
        )
    }
}

/// Upgrades the vault in `root`, of an older format, to today's, and returns its marker then. The user chose this in
/// the vault picker; nothing else calls it. An encrypted vault takes its key, which this machine must have
/// (`Error::Locked` when it doesn't, and then nothing is changed). A vault that is of today's format already (another
/// computer upgraded it, and the cloud brought that here) is left as it is.
pub fn vault(root: &Path) -> Result<Marker> {
    let marker = match marker::inspect(root) {
        Found::Older(marker) => marker,
        Found::Vault(marker) => return Ok(marker),
        found => return Err(marker::refusal(root, &found).unwrap_or_else(|| Error::invalid(format!("{} isn't a vault yet, so there is nothing to upgrade", root.display())))),
    };
    let store = Vault::store_for(root, &marker)?;
    store::register(&store);
    let from = marker.ledger;
    let moved = into_history(&store, true).map_err(|e| {
        Error::Io(format!(
            "{} couldn't be upgraded all the way, so it isn't opened yet ({e}). Nothing is lost: close any program that has its files open and upgrade it again, which carries on where it stopped.",
            root.display()
        ))
    })?;
    let upgraded = Marker { ledger: marker::FORMAT, ..marker };
    marker::write(root, &upgraded)?;
    crate::applog::info("vault", format!("Upgraded {} from vault format {from} to {}, as asked: {moved}", root.display(), marker::FORMAT));
    Ok(upgraded)
}

/// How many files in the vault are where format 1 kept its history (`versions/`, a thread's `runs/`): written there by
/// an older Ledger that still had the vault open after it was upgraded. They are only counted, not changed; the app
/// doesn't read them until they are moved into `history/` ([`tidy`]).
pub fn leftovers(store: &Store) -> usize {
    store.list(&["projects"], |dir| !leads_to_old_places(dir, false)).into_keys().filter(|rel| old_place(rel).is_some()).count()
}

/// Moves what an older Ledger wrote to the places format 1 kept its history in into `history/`, as the user asked in
/// Settings > Vault, and points the links at them anew.
pub fn tidy(store: &Store) -> Result<Moved> {
    let moved = into_history(store, false).map_err(|e| Error::Io(format!("Not everything could be moved into history/ ({e}). Close any program that has those files open and try again.")))?;
    crate::applog::info("vault", format!("Moved the files an older Ledger wrote into history/, as asked: {moved}"));
    Ok(moved)
}

// ---- format 1 to 2 ----------------------------------------------------------------------------------

/// A file in one of the places format 1 kept its history in, and where format 2 keeps it.
enum Old<'a> {
    /// A file in a project's `versions/`: it goes to the same place in `history/deliverables/`.
    Version { to: String },
    /// A run's record in its thread's `runs/`: it goes to the project's `history/runs/`, and names its thread.
    Run { to: String, thread: &'a str },
}

/// Where the file at `rel` goes, when it is in one of the places format 1 kept its history in.
fn old_place(rel: &str) -> Option<Old<'_>> {
    match rel.split('/').collect::<Vec<_>>().as_slice() {
        ["projects", project, folder, rest @ ..] if *folder == OLD_VERSIONS && !rest.is_empty() => {
            Some(Old::Version { to: format!("projects/{project}/{}/{}", deliverables::VERSIONS_DIR, rest.join("/")) })
        }
        ["projects", project, "threads", thread, folder, name] if *folder == OLD_RUNS => {
            let id = name.strip_suffix(".md")?;
            Some(Old::Run { to: vault::run_file(project, id), thread })
        }
        _ => None,
    }
}

/// Moves what format 1 kept in `versions/` and in the threads' `runs/` into `history/`, and points the links at them
/// anew: in every note and prompt when `everything` (the vault is of format 1), otherwise only when a version moved.
fn into_history(store: &Store, everything: bool) -> std::result::Result<Moved, String> {
    let mut moved = Moved::default();
    let stamp = chrono::Local::now().format("%Y-%m-%d %H%M%S").to_string();
    for rel in store.list(&["projects"], |dir| !leads_to_old_places(dir, false)).into_keys() {
        let (placed, version) = match old_place(&rel) {
            Some(Old::Version { to }) => (place(store, &rel, &to, None, &stamp)?, true),
            Some(Old::Run { to, thread }) => {
                let text = store.read(&rel).map_err(|e| format!("{rel}: {e}"))?;
                let contents = naming_thread(&String::from_utf8_lossy(&text), thread).map_or(text, String::into_bytes);
                (place(store, &rel, &to, Some(contents), &stamp)?, false)
            }
            None => continue,
        };
        match (placed, version) {
            (Placed::KeptApart, _) => moved.kept_apart += 1,
            (Placed::Moved, true) => moved.versions += 1,
            (Placed::Moved, false) => moved.runs += 1,
        }
    }
    if everything || moved.versions > 0 {
        for rel in store.list(&["projects"], |dir| !leads_to_old_places(dir, true)).into_keys() {
            let relink: fn(&str) -> Option<String> = match rel.split('/').collect::<Vec<_>>().as_slice() {
                ["projects", _, "threads", _, "posts", name] if name.ends_with(".md") => relinked_note,
                ["projects", _, "threads", _, "prompts", name] if name.ends_with(".md") => relinked_prompt,
                _ => continue,
            };
            let text = store.read(&rel).map_err(|e| format!("{rel}: {e}"))?;
            if let Some(text) = relink(&String::from_utf8_lossy(&text)) {
                store.write(&rel, text.as_bytes()).map_err(|e| format!("{rel}: {e}"))?;
                moved.relinked += 1;
            }
        }
    }
    Ok(moved)
}

/// Whether the folder `dir` (a path in the vault) is, or holds, one of the places format 1 kept its history in: a
/// project's `versions/`, a thread's `runs/`; with `notes`, also a thread's `posts/` and `prompts/`.
fn leads_to_old_places(dir: &str, notes: bool) -> bool {
    let parts: Vec<&str> = dir.split('/').collect();
    match parts.as_slice() {
        ["projects"] | ["projects", _] | ["projects", _, "threads"] | ["projects", _, "threads", _] => true,
        ["projects", _, folder, ..] if *folder == OLD_VERSIONS => true,
        ["projects", _, "threads", _, folder] => *folder == OLD_RUNS || (notes && matches!(*folder, "posts" | "prompts")),
        _ => false,
    }
}

enum Placed {
    /// At its new place, and gone from the old one.
    Moved,
    /// Its new place held other contents: it is beside it, as a conflict copy.
    KeptApart,
}

/// Puts the file at `from` at `to`, with `contents` (its own when `None`), and removes it from `from`. When `to` holds
/// the same already (an upgrade that stopped halfway before), only `from` goes; when it holds something else, the file
/// is kept beside it as a conflict copy, for the Conflicts page.
fn place(store: &Store, from: &str, to: &str, contents: Option<Vec<u8>>, stamp: &str) -> std::result::Result<Placed, String> {
    fn fail(rel: &str) -> impl Fn(std::io::Error) -> String + '_ {
        move |e| format!("{rel}: {e}")
    }
    let (to, placed) = if !store.exists(to) {
        (to.to_string(), Placed::Moved)
    } else {
        let ours = match &contents {
            Some(bytes) => bytes.clone(),
            None => store.read(from).map_err(fail(from))?,
        };
        if store.read(to).map_err(fail(to))? == ours {
            store.remove(from).map_err(fail(from))?;
            return Ok(Placed::Moved);
        }
        (conflicts::ledger_copy(to, stamp), Placed::KeptApart)
    };
    match contents {
        // A version keeps its bytes, and the time it was made: it is moved, not written anew.
        None => store.rename(from, &to).map_err(fail(from))?,
        Some(bytes) => {
            store.write(&to, &bytes).map_err(fail(&to))?;
            store.remove(from).map_err(fail(from))?;
        }
    }
    Ok(placed)
}

/// A run's record as format 2 keeps it: its frontmatter names its thread, first, as the record's own folder no longer
/// does. `None` for a file without frontmatter, which is moved as it is.
fn naming_thread(text: &str, thread: &str) -> Option<String> {
    let parsed = frontmatter::parse(text)?;
    let mut meta = Mapping::new();
    meta.insert(Value::String("thread".into()), Value::String(thread.into()));
    for (key, value) in parsed.meta {
        if key.as_str() != Some("thread") {
            meta.insert(key, value);
        }
    }
    Some(frontmatter::render(&meta, &parsed.body))
}

/// A "version published" note that links to its manifest in `versions/`, linking to it in `history/deliverables/`
/// instead; `None` for any other post.
fn relinked_note(text: &str) -> Option<String> {
    let body = frontmatter::parse(text).map_or_else(|| text.to_string(), |parsed| parsed.body);
    let (_, manifest) = provenance::note_target(&body)?;
    manifest.strip_prefix(&format!("{OLD_VERSIONS}/"))?;
    let old = format!("](../../../{OLD_VERSIONS}/");
    let at = text.rfind(&old)?;
    Some(format!("{}](../../../{}/{}", &text[..at], deliverables::VERSIONS_DIR, &text[at + old.len()..]))
}

/// A prompt whose base says which version a piece of feedback was given on ("### On v1.0 (versions/plan/plan@v1.0.md)",
/// see `feedback::block`) and names its file in `versions/`, naming it in `history/deliverables/` instead; `None` for a
/// prompt that names none.
fn relinked_prompt(text: &str) -> Option<String> {
    const HEADING: &str = "### On v";
    let old = format!(" ({OLD_VERSIONS}/");
    let mut out = String::with_capacity(text.len() + 64);
    let mut rest = text;
    let mut changed = false;
    while let Some(at) = rest.find(&old) {
        let before = &rest[..at];
        let on_a_version = before.rfind(HEADING).is_some_and(|h| deliverables::parse_version(&before[h + HEADING.len()..]).is_some());
        out.push_str(before);
        if on_a_version {
            out.push_str(&format!(" ({}/", deliverables::VERSIONS_DIR));
            changed = true;
        } else {
            out.push_str(&old);
        }
        rest = &rest[at + old.len()..];
    }
    out.push_str(rest);
    changed.then_some(out)
}
