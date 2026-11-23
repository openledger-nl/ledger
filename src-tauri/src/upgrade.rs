//! Brings a vault of an older format up to today's (see `marker`), when it is opened and before it is read.
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
//! version a prompt's feedback was given on ("### On v1.0 (versions/plan/plan@v1.0.md)").
//!
//! **Safe to stop, and to do again.** The files are moved first and the marker is written last, so an upgrade that
//! stops halfway (the app closed, a file a sync client wouldn't let go of) carries on where it stopped the next time
//! the vault is opened. A file that is at its new place already, with the same contents, is not moved twice; one that
//! is there with other contents is kept beside it as a conflict copy, so nothing is lost.
//!
//! **Other computers.** A Ledger of format 1 refuses a vault of format 2 when it opens it (`marker::Found::Newer`), but
//! one that had the vault open already writes to the old places until it is closed. So a vault of today's format is
//! looked over for those places too each time it is opened, and what turns up there is moved in the same way.

use std::path::Path;

use serde_yaml_ng::{Mapping, Value};

use crate::error::{Error, Result};
use crate::marker::{self, Marker};
use crate::store::Store;
use crate::{conflicts, deliverables, frontmatter, provenance, vault};

/// Where format 1 kept a project's published versions, in the project's folder.
const OLD_VERSIONS: &str = "versions";

/// Where format 1 kept the records of a thread's runs, in the thread's folder.
const OLD_RUNS: &str = "runs";

/// What bringing a vault up to date did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

impl Moved {
    fn is_empty(&self) -> bool {
        *self == Moved::default()
    }
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

/// Brings the vault in `root`, whose files `store` reaches, up to today's format, and returns its marker as it is then.
/// A vault of today's format only has what an older Ledger still wrote to the old places moved in.
pub fn run(root: &Path, store: &Store, marker: &Marker) -> Result<Marker> {
    let from = marker.ledger;
    let moved = into_history(store, from < 2).map_err(|e| {
        Error::Io(format!(
            "{} couldn't be brought up to date for this Ledger, so it isn't opened yet ({e}). Close any program that has its files open and open the vault again: it carries on where it stopped.",
            root.display()
        ))
    })?;
    if from >= marker::FORMAT {
        if !moved.is_empty() {
            crate::applog::info("vault", format!("Found files in the places a vault of format 1 used, written there by an older Ledger: {moved}"));
        }
        return Ok(marker.clone());
    }
    let upgraded = Marker { ledger: marker::FORMAT, ..marker.clone() };
    marker::write(root, &upgraded)?;
    crate::applog::info("vault", format!("Upgraded {} from vault format {from} to {}: {moved}", root.display(), marker::FORMAT));
    Ok(upgraded)
}

// ---- format 1 to 2 ----------------------------------------------------------------------------------

/// Moves what format 1 kept in `versions/` and in the threads' `runs/` into `history/`, and points the links at them
/// anew: in every note and prompt when `everything` (the vault is of format 1), otherwise only when something moved.
fn into_history(store: &Store, everything: bool) -> std::result::Result<Moved, String> {
    let mut moved = Moved::default();
    let stamp = chrono::Local::now().format("%Y-%m-%d %H%M%S").to_string();
    for rel in store.list(&["projects"], |dir| !leads_to_old_places(dir, false)).into_keys() {
        match rel.split('/').collect::<Vec<_>>().as_slice() {
            ["projects", project, folder, rest @ ..] if *folder == OLD_VERSIONS && !rest.is_empty() => {
                let to = format!("projects/{project}/{}/{}", deliverables::VERSIONS_DIR, rest.join("/"));
                match place(store, &rel, &to, None, &stamp)? {
                    Placed::Moved => moved.versions += 1,
                    Placed::KeptApart => moved.kept_apart += 1,
                }
            }
            ["projects", project, "threads", thread, folder, name] if *folder == OLD_RUNS => {
                let Some(id) = name.strip_suffix(".md") else { continue };
                let text = store.read(&rel).map_err(|e| format!("{rel}: {e}"))?;
                let contents = naming_thread(&String::from_utf8_lossy(&text), thread).map_or(text, String::into_bytes);
                match place(store, &rel, &vault::run_file(project, id), Some(contents), &stamp)? {
                    Placed::Moved => moved.runs += 1,
                    Placed::KeptApart => moved.kept_apart += 1,
                }
            }
            _ => {}
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
