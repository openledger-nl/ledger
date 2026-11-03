//! The Jira sync's own files. Like the Trello sync's, they sit in the vault's `.ledger/` folder, which is never
//! uploaded:
//!
//! ```text
//! .ledger/jira.json        the Jira site, and which Jira project each Ledger project is linked to
//! .ledger/jira-sync.json   the sync's bookkeeping: what both sides last agreed on, for every linked issue
//! .ledger/.env             the email address and API token (JIRA_EMAIL, JIRA_API_TOKEN), with the other secrets
//! .ledger/conflicts/       the losing version when both sides changed the same thing
//! ```
//!
//! The issue key of a linked item is *not* here: it is in the item's own frontmatter (`jiraIssue`), so it travels
//! with the item to other machines.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::settings::{self, LEDGER_DIR};
use crate::trello_card::Kind;

pub const KEY_EMAIL: &str = "JIRA_EMAIL";
pub const KEY_TOKEN: &str = "JIRA_API_TOKEN";

// ---- settings -----------------------------------------------------------------------------------------

/// A Ledger project's link to a Jira project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLink {
    /// The Jira project's key.
    pub project: String,
    pub project_name: String,
    /// Column id → Jira status id, for the columns whose status isn't simply the one with the same name. An empty
    /// status means "don't sync this column".
    pub columns: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JiraSettings {
    /// `https://<name>.atlassian.net`.
    pub site: String,
    /// Project slug → Jira project.
    pub projects: BTreeMap<String, ProjectLink>,
}

fn settings_file(root: &Path) -> PathBuf {
    root.join(LEDGER_DIR).join("jira.json")
}

fn state_file(root: &Path) -> PathBuf {
    root.join(LEDGER_DIR).join("jira-sync.json")
}

pub fn settings_path(root: &Path) -> PathBuf {
    settings_file(root)
}

pub fn read_settings(root: &Path) -> JiraSettings {
    fs::read_to_string(settings_file(root)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

fn write_file(file: &Path, text: String) -> Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::write(file, text).map_err(|e| Error::io(file, e))
}

/// Changes some settings and leaves the others as they were.
pub fn update_settings(root: &Path, change: impl FnOnce(&mut JiraSettings)) -> Result<JiraSettings> {
    let mut settings = read_settings(root);
    change(&mut settings);
    write_file(&settings_file(root), serde_json::to_string_pretty(&settings).unwrap_or_default())?;
    settings::ensure_gitignore(root)?;
    Ok(settings)
}

// ---- the secrets --------------------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct Credentials {
    pub email: String,
    pub token: String,
}

// Deliberately not `Debug`/`Serialize`: nothing should be able to print these by accident.
impl Credentials {
    pub fn complete(&self) -> bool {
        !self.email.is_empty() && !self.token.is_empty()
    }
}

pub fn read_credentials(root: &Path) -> Credentials {
    Credentials {
        email: settings::read_secret(root, KEY_EMAIL).unwrap_or_default(),
        token: settings::read_secret(root, KEY_TOKEN).unwrap_or_default(),
    }
}

pub fn write_credentials(root: &Path, credentials: &Credentials) -> Result<()> {
    settings::write_secret(root, KEY_EMAIL, &credentials.email)?;
    settings::write_secret(root, KEY_TOKEN, &credentials.token)
}

/// Whether there is something to sync: a site, both credentials, and at least one linked project.
pub fn is_configured(root: &Path) -> bool {
    let settings = read_settings(root);
    !settings.site.is_empty() && read_credentials(root).complete() && settings.projects.values().any(|p| !p.project.is_empty())
}

// ---- the sync's bookkeeping ---------------------------------------------------------------------------

/// What is compared between the two sides: the title, the description in canonical form, and for a task the Jira
/// status (`None` when its column has no status, which is then left alone).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Content {
    pub title: String,
    pub desc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// A linked issue, with what both sides agreed on when they were last in step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tracked {
    pub project: String,
    pub kind: Kind,
    pub id: String,
    pub base: Content,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncState {
    /// Issue key → the item it is linked to here.
    pub items: BTreeMap<String, Tracked>,
}

pub fn read_state(root: &Path) -> SyncState {
    fs::read_to_string(state_file(root)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

pub fn write_state(root: &Path, state: &SyncState) -> Result<()> {
    write_file(&state_file(root), serde_json::to_string(state).unwrap_or_default())?;
    settings::ensure_gitignore(root)
}

pub fn update_state(root: &Path, change: impl FnOnce(&mut SyncState)) -> Result<()> {
    let mut state = read_state(root);
    change(&mut state);
    write_state(root, &state)
}
