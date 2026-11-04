//! Portflow's settings on this machine: whether the feature is on, what Ledger may do there, and which portfolio it
//! reads. Kept in the vault's `.ledger/portflow.json`, which the vault's `.gitignore` lists and the GitHub sync never
//! looks at, so each machine decides for itself and nothing here is uploaded:
//!
//! ```text
//! .ledger/portflow.json   on or off, read only or read and write (and which writes), the launch address, the portfolio
//! ```
//!
//! The session token is not here, nor anywhere on disk: it lives eight hours and is kept in memory only
//! (`portflow_auth.rs`). What the vault holds of Portflow (the mirrored outcomes, people, collections and the
//! evidence items) is plain files under `portflow/` and in the projects, written by the sync (`portflow_store.rs`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::settings::{self, LEDGER_DIR};

/// Where Fontys ICT students open Portflow: the Canvas page that launches it.
pub const DEFAULT_LAUNCH_URL: &str = "https://fhict.instructure.com/accounts/1/external_tools/23360?launch_type=global_navigation";

/// The kinds of write Ledger could make in Portflow, each with its own switch. None is on until the person turns it on.
pub const WRITE_KINDS: [(&str, &str); 7] = [
    ("upload-versions", "Upload versions"),
    ("create-evidence", "Create evidence"),
    ("edit-descriptions", "Edit collection descriptions"),
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
