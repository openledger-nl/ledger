//! The vault's marker: `ledger.json` at its root. It says that the folder is a vault, which format its files are in,
//! and the vault's id:
//!
//! ```json
//! { "ledger": 1, "id": "3f9a…" }
//! ```
//!
//! - **The cutoff.** Ledger opens only folders with a marker of a format it reads. A folder that holds a vault from
//!   Ledger 0.x (which had no marker) is refused, and nothing in it is changed: 0.x vaults are not read or converted.
//!   Everything else in the app may take a vault in today's format for granted.
//! - **The id** names the vault on this machine (the vault's folder has another path on every machine that syncs
//!   it), so what belongs to this machine's copy (its settings, its secrets) is kept outside the vault by id: see
//!   [`crate::settings::local_dir`].
//! - **The format** is what a future change to the vault's files bumps, with a step that brings a vault from one
//!   format to the next, run once.

use std::fs;
use std::path::Path;

use aes_gcm::aead::Generate;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const FILE: &str = "ledger.json";

/// The format of the vault's files that this Ledger reads and writes.
pub const FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    /// The format of the vault's files.
    pub ledger: u32,
    pub id: String,
}

/// What a folder is, as far as opening it as a vault goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// A vault of today's format.
    Vault(Marker),
    /// Nothing there yet, or a folder of other files without anything of Ledger in it: it becomes a vault.
    New,
    /// A vault from Ledger 0.x: refused, and left as it is.
    Legacy,
    /// A vault in a format a newer Ledger wrote.
    Newer(u32),
    /// A marker that can't be read.
    Damaged(String),
}

/// Folders that only a vault from Ledger 0.x has at its root without a marker.
const LEGACY_SIGNS: [&str; 3] = [".ledger", "projects", "semesters"];

pub fn inspect(root: &Path) -> Found {
    let file = root.join(FILE);
    match fs::read_to_string(&file) {
        Ok(text) => match serde_json::from_str::<Marker>(&text) {
            Ok(marker) if marker.ledger > FORMAT => Found::Newer(marker.ledger),
            Ok(marker) if marker.ledger == FORMAT && !marker.id.trim().is_empty() => Found::Vault(marker),
            Ok(marker) => Found::Damaged(format!("{} names format {} and id {:?}, which this Ledger doesn't read", file.display(), marker.ledger, marker.id)),
            Err(e) => Found::Damaged(format!("{}: {e}", file.display())),
        },
        Err(_) if LEGACY_SIGNS.iter().any(|name| root.join(name).exists()) => Found::Legacy,
        Err(_) => Found::New,
    }
}

/// What to tell someone about a folder that can't be opened as a vault; `None` for one that can.
pub fn refusal(root: &Path, found: &Found) -> Option<Error> {
    match found {
        Found::Vault(_) | Found::New => None,
        Found::Legacy => Some(Error::invalid(format!(
            "{} holds a vault from Ledger 0.x. Ledger 1.0 can't open it and doesn't change it: create a new vault, in a folder your cloud storage keeps in sync.",
            root.display()
        ))),
        Found::Newer(format) => Some(Error::invalid(format!(
            "{} was made by a newer Ledger (vault format {format}). Update Ledger to open it.",
            root.display()
        ))),
        Found::Damaged(why) => Some(Error::invalid(format!("The vault's {FILE} can't be read, so the vault isn't opened: {why}"))),
    }
}

/// A random id for a new vault: 32 hex digits.
pub fn new_id() -> String {
    <[u8; 16]>::generate().iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes the marker of a new vault and returns it.
pub fn create(root: &Path) -> Result<Marker> {
    let marker = Marker { ledger: FORMAT, id: new_id() };
    write(root, &marker)?;
    Ok(marker)
}

/// Writes the marker. It is written when the vault is made, and not changed after that.
pub fn write(root: &Path, marker: &Marker) -> Result<()> {
    fs::create_dir_all(root).map_err(|e| Error::io(root, e))?;
    let file = root.join(FILE);
    let text = serde_json::to_string_pretty(marker).map_err(|e| Error::invalid(format!("The vault's marker can't be written: {e}")))?;
    let temp = root.join(format!(".{FILE}.tmp"));
    fs::write(&temp, text + "\n").map_err(|e| Error::io(&temp, e))?;
    fs::rename(&temp, &file).map_err(|e| Error::io(&file, e))
}

/// For tests that write a vault's files by hand before opening it: makes the folder a vault of today's format.
#[cfg(test)]
pub fn ensure(root: &Path) {
    if !root.join(FILE).exists() {
        let _ = create(root);
    }
}
