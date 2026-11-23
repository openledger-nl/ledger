//! Conflict copies: when two machines change the same file before their cloud storage has carried one change to the
//! other, the provider keeps both, the second under another name. Ledger does the same when a file it is about to
//! write was changed on disk since it last read it (see `Vault::persist`). Such a copy is not read in as a thing of
//! its own (it would be a second post with the same id, or an extra deliverable); it is listed, so the person can
//! choose which one to keep.
//!
//! The copies are known by their names, whatever the provider:
//!
//! ```text
//! notes (conflicted copy 2026-11-20 101500).md     Nextcloud, ownCloud and OpenCloud
//! notes (Laptop's conflicted copy 2026-11-20).md   Dropbox
//! notes.sync-conflict-20261120-101500-ABCDEFG.md   Syncthing
//! notes (Ledger conflict 2026-11-20 101500).md     Ledger itself
//! notes-LAPTOP.md                                  OneDrive and SharePoint: the computer's name after a dash
//! ```
//!
//! OneDrive's form could also be a name someone chose ("report-V2.md"), so it only counts for files Ledger names
//! itself (things kept by their id, and `project.md`, `thread.md`, `README.md`), and only when the original is there.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// The copy, by its path in the vault.
    pub copy: String,
    /// The file it is a copy of.
    pub original: String,
}

/// The name Ledger gives the copy of `rel` it keeps when the file changed on disk under it.
pub fn ledger_copy(rel: &str, stamp: &str) -> String {
    let (dir, name) = split_dir(rel);
    let (stem, ext) = split_ext(name);
    format!("{dir}{stem} (Ledger conflict {stamp}){ext}")
}

/// The file `rel` is a conflict copy of, if it is one; `exists` says whether a file is in the vault.
pub fn original_of(rel: &str, exists: impl Fn(&str) -> bool) -> Option<String> {
    let (dir, name) = split_dir(rel);
    let (stem, ext) = split_ext(name);
    let original = |base: &str| format!("{dir}{base}{ext}");

    // A note in brackets at the end: Nextcloud's, Dropbox's and Ledger's own.
    if let Some(inner) = stem.strip_suffix(')')
        && let Some(open) = inner.rfind(" (")
    {
        let said = &inner[open + 2..];
        if said.contains("conflicted copy") || said.starts_with("Ledger conflict") {
            return Some(original(&inner[..open]));
        }
    }
    // Syncthing's.
    if let Some(at) = stem.find(".sync-conflict-") {
        return Some(original(&stem[..at]));
    }
    // OneDrive's: `-COMPUTERNAME`, for the names Ledger gives, when the original is there.
    if named_by_ledger(dir, stem) || named_by_ledger(dir, base_before_dash(stem)?) {
        for (at, _) in stem.match_indices('-') {
            let (base, machine) = (&stem[..at], &stem[at + 1..]);
            if is_machine_name(machine) && exists(&original(base)) {
                return Some(original(base));
            }
        }
    }
    None
}

/// `dir/` and the file's name.
fn split_dir(rel: &str) -> (&str, &str) {
    match rel.rfind('/') {
        Some(at) => (&rel[..=at], &rel[at + 1..]),
        None => ("", rel),
    }
}

/// The name without its extension, and the extension with its dot (empty when there is none).
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 => (&name[..at], &name[at..]),
        _ => (name, ""),
    }
}

fn base_before_dash(stem: &str) -> Option<&str> {
    stem.find('-').map(|at| &stem[..at])
}

/// A computer's name as OneDrive puts it after the dash: capitals, digits and dashes, with a capital in it.
fn is_machine_name(text: &str) -> bool {
    text.len() >= 2
        && text.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
        && text.chars().any(|c| c.is_ascii_uppercase())
        && !text.starts_with('-')
        && !text.ends_with('-')
}

/// Whether Ledger names the files in `dir` itself (by an id, or as one of its own files), so that a name it would
/// never give is a copy.
fn named_by_ledger(dir: &str, stem: &str) -> bool {
    const BY_ID: [&str; 9] = ["/posts/", "/prompts/", "/feedback/", "/uploads/", "/runs/", "/userstories/", "/tasks/", "/evidence/", "/templates/prompts/"];
    const SHARED: [&str; 5] = ["shared/people/", "shared/semesters/", "shared/clients/", "shared/templates/prompts/", "shared/templates/semesters/"];
    let own_file = matches!(stem, "project" | "thread" | "README");
    let versions_record = dir.contains(&format!("/{}/", crate::deliverables::VERSIONS_DIR)) && stem.contains(".manifest");
    own_file || versions_record || BY_ID.iter().any(|d| dir.ends_with(d)) || SHARED.contains(&dir) || dir.starts_with("meetings/")
}
