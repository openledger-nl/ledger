//! Portflow's settings on this machine: whether the feature is on, what Ledger may do there, and which portfolio it
//! reads. Kept in the vault's `.ledger/portflow.json`, which the vault's `.gitignore` lists and the GitHub sync never
//! looks at, so each machine decides for itself and nothing here is uploaded:
//!
//! ```text
//! .ledger/portflow.json          on or off, read only or read and write (and which writes), the launch address, the portfolio
//! .ledger/portflow-guides.json   what Ledger last saw of each reading guide and evidence description, both sides (hashes)
//! ```
//!
//! The session token is not here, nor anywhere on disk: it lives eight hours and is kept in memory only
//! (`portflow_auth.rs`). What the vault holds of Portflow (the mirrored outcomes, people, collections and the
//! evidence items) is plain files under `portflow/` and in the projects, written by the sync (`portflow_store.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::settings::{self, LEDGER_DIR};

/// Where Fontys ICT students open Portflow: the Canvas page that launches it.
pub const DEFAULT_LAUNCH_URL: &str = "https://fhict.instructure.com/accounts/1/external_tools/23360?launch_type=global_navigation";

/// The write that puts a project's reading guide (its README.md) in the description of the collection it is linked to
/// for the current semester. Replaces "edit-descriptions": the guide is how a description is edited in Ledger, so a
/// machine that had that switch on starts with this one off.
pub const GUIDE_WRITE: &str = "guide-description";

/// The write that puts an evidence item's description, as kept in Ledger ("What? / Goal? / Why it contributes?"), in
/// the evidence item in Portflow.
pub const EVIDENCE_WRITE: &str = "evidence-description";

/// The writes this version can make: only these can be switched on; the rest are listed, greyed out, until built.
pub const BUILT_WRITES: [&str; 2] = [GUIDE_WRITE, EVIDENCE_WRITE];

/// The kinds of write Ledger could make in Portflow, each with its own switch. None is on until the person turns it on.
pub const WRITE_KINDS: [(&str, &str); 8] = [
    ("upload-versions", "Upload versions"),
    ("create-evidence", "Create evidence"),
    (GUIDE_WRITE, "Reading guide to collection description"),
    (EVIDENCE_WRITE, "Evidence description to evidence item"),
    ("link-outcomes", "Link outcomes"),
    ("reply-feedback", "Reply to feedback"),
    ("ask-feedback", "Ask for feedback"),
    ("confirm-feedback", "Ask to confirm feedback"),
];

/// What Ledger may do in Portflow.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Access {
    /// Off (the default): read only. On: the kinds in `writes` are allowed.
    pub write: bool,
    pub writes: BTreeSet<String>,
}

impl Access {
    /// Whether this kind of write may be made: read and write is on, and so is the kind's own switch.
    pub fn allows(&self, kind: &str) -> bool {
        self.write && self.writes.contains(kind)
    }
}

/// The portfolio Ledger reads: found at sign-in, kept so the next session doesn't need to look for it again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Portfolio {
    pub id: i64,
    /// The signed-in student, as Portflow names them.
    pub user_id: i64,
    pub user_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortflowSettings {
    /// Off by default: a new, optional feature. Off means fully off (see `portflow.rs`).
    pub enabled: bool,
    /// The Canvas page that launches Portflow.
    pub launch_url: String,
    pub access: Access,
    pub portfolio: Option<Portfolio>,
}

impl Default for PortflowSettings {
    fn default() -> Self {
        PortflowSettings { enabled: false, launch_url: DEFAULT_LAUNCH_URL.to_string(), access: Access::default(), portfolio: None }
    }
}

fn settings_file(root: &Path) -> PathBuf {
    root.join(LEDGER_DIR).join("portflow.json")
}

/// The settings, or the defaults (off) when there are none or they can't be read.
pub fn read_settings(root: &Path) -> PortflowSettings {
    let mut settings: PortflowSettings =
        fs::read_to_string(settings_file(root)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
    if settings.launch_url.trim().is_empty() {
        settings.launch_url = DEFAULT_LAUNCH_URL.to_string();
    }
    // A write kind this version doesn't know is dropped rather than allowed.
    settings.access.writes.retain(|kind| WRITE_KINDS.iter().any(|(k, _)| k == kind));
    settings
}

/// Whether the feature is on in the vault at `root`. Everything Portflow does asks this first.
pub fn is_enabled(root: &Path) -> bool {
    read_settings(root).enabled
}

/// Changes some settings and leaves the others as they were.
pub fn update_settings(root: &Path, change: impl FnOnce(&mut PortflowSettings)) -> Result<PortflowSettings> {
    let mut settings = read_settings(root);
    change(&mut settings);
    let file = settings_file(root);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::write(&file, serde_json::to_string_pretty(&settings).unwrap_or_default()).map_err(|e| Error::io(&file, e))?;
    settings::ensure_gitignore(root)?;
    Ok(settings)
}

/// Checks a launch address before it is saved: an https page, so the sign-in never goes over plain http.
pub fn check_launch_url(url: &str) -> Result<String> {
    let url = url.trim();
    let parsed = tauri::Url::parse(url).map_err(|_| Error::invalid("Enter the full address of the Canvas page that opens Portflow"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(Error::invalid("The address must start with https://"));
    }
    Ok(url.to_string())
}

// ---- what Ledger last saw of each reading guide ----------------------------------------------------------

/// A project's reading guide and its collection's description, as they were the last time they agreed: hashes of the
/// description as Portflow had it (HTML, white space folded) and of the README as it was then. A round compares both
/// with now to tell which side changed. Bookkeeping for this machine only, so it lives in `.ledger/`, next to the
/// settings, and survives a restart; lost or unreadable, the next round starts as on a first one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GuideMemory {
    /// By `<project>/<collection id>`.
    #[serde(default)]
    pub guides: BTreeMap<String, GuideSeen>,
    /// Evidence descriptions, by `<project>/<evidence id in Portflow>`; there `readme` is Ledger's text of the description.
    #[serde(default)]
    pub evidence: BTreeMap<String, GuideSeen>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GuideSeen {
    pub description: String,
    pub readme: String,
}

fn guides_file(root: &Path) -> PathBuf {
    root.join(LEDGER_DIR).join("portflow-guides.json")
}

pub fn read_guides(root: &Path) -> GuideMemory {
    fs::read_to_string(guides_file(root)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

pub fn write_guides(root: &Path, memory: &GuideMemory) -> Result<()> {
    let file = guides_file(root);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::write(&file, serde_json::to_string_pretty(memory).unwrap_or_default()).map_err(|e| Error::io(&file, e))?;
    settings::ensure_gitignore(root)
}
