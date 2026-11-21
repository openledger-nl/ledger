//! Where the vault is, and the settings that live in it.
//!
//! The vault's location is the one thing that can't live in the vault, so it is kept in a small `config.json` in
//! the app's config folder, with the vaults opened before (for the vault picker) and the other choices that belong to
//! this machine's copy of Ledger rather than to a vault: whether it looks for updates by itself. Everything else belongs to the vault and sits in
//! `<vault>/.ledger/`:
//!
//! ```text
//! .ledger/settings.json     the screenshot hotkey and tool, the folded sidebar
//! .ledger/.env              the secrets (the Trello key and token, the Portflow session)
//! .ledger/conflicts/        versions set aside when a sync with Trello or Portflow resolved a conflict
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::onboarding::OnboardingConfig;

pub const LEDGER_DIR: &str = ".ledger";
// ---- the app's own config ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    /// The vault that is open, and that opens at the next start. Unset on a machine that has never chosen one.
    pub vault_path: Option<String>,
    /// The vaults opened on this machine, the most recent first, for the vault picker (as Obsidian lists them).
    #[serde(deserialize_with = "lenient", skip_serializing_if = "Vec::is_empty")]
    pub recent_vaults: Vec<String>,
    /// Read leniently: a value edited into something else by hand falls back to the default instead of making the
    /// whole file unreadable, which would also lose the vault's location.
    #[serde(deserialize_with = "lenient")]
    pub updates: UpdateConfig,
    /// The most the app's log may take on disk; unset means `applog::DEFAULT_MAX_BYTES`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_max_bytes: Option<u64>,
    /// The guide on the home page for a new client (see `onboarding`). Per machine, like the choice to look for updates.
    #[serde(deserialize_with = "lenient")]
    pub onboarding: OnboardingConfig,
}

fn lenient<'de, D: serde::Deserializer<'de>, T: serde::de::DeserializeOwned + Default>(deserializer: D) -> std::result::Result<T, D::Error> {
    Ok(serde_json::from_value(serde_json::Value::deserialize(deserializer)?).unwrap_or_default())
}

/// Looking for new releases (see `updates.rs`). Per machine, so not in the vault: it is about the program installed
/// here, it must not change when another vault is opened, and an update never touches this file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateConfig {
    /// Look shortly after starting and every few hours. A check from Settings works either way.
    pub automatic: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        UpdateConfig { automatic: true }
    }
}

fn config_file(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

pub fn load_config(dir: &Path) -> AppConfig {
    fs::read_to_string(config_file(dir)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

pub fn save_config(dir: &Path, config: &AppConfig) -> Result<()> {
    fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let file = config_file(dir);
    let text = serde_json::to_string_pretty(config).unwrap_or_default();
    fs::write(&file, text).map_err(|e| Error::io(&file, e))
}

/// Changes some settings and leaves the others as they were.
pub fn update_config(dir: &Path, change: impl FnOnce(&mut AppConfig)) -> Result<()> {
    let mut config = load_config(dir);
    change(&mut config);
    save_config(dir, &config)
}

/// Where the vault is: `LEDGER_VAULT` if set (and then not changeable from the UI), else the vault opened last.
/// Before the vault picker, a machine that never chose a folder used `Ledger` inside the user's documents (or `vault`
/// in the config folder without one); such a vault is still found there, so it keeps opening. `None` means no vault
/// has been chosen yet: the app starts on the picker and asks for one.
pub fn resolve_vault(config_dir: &Path, documents_dir: Option<PathBuf>) -> (Option<PathBuf>, bool) {
    if let Some(path) = std::env::var_os("LEDGER_VAULT").filter(|p| !p.is_empty()) {
        return (Some(PathBuf::from(path)), true);
    }
    if let Some(path) = load_config(config_dir).vault_path.filter(|p| !p.trim().is_empty()) {
        return (Some(PathBuf::from(path)), false);
    }
    let legacy = documents_dir.map_or_else(|| config_dir.join("vault"), |d| d.join("Ledger"));
    (legacy.is_dir().then_some(legacy), false)
}

/// How many vaults the picker remembers.
pub const RECENT_VAULTS: usize = 12;

/// Notes that `path` is the vault in use: it opens at the next start, and comes first among the recent ones.
pub fn remember_vault(config_dir: &Path, path: &Path) -> Result<()> {
    let path = path.display().to_string();
    update_config(config_dir, |config| {
        config.recent_vaults.retain(|p| p != &path);
        config.recent_vaults.insert(0, path.clone());
        config.recent_vaults.truncate(RECENT_VAULTS);
        config.vault_path = Some(path);
    })
}

/// Takes a vault off the picker's list. Its folder is left alone. Forgetting the open vault isn't possible: the list
/// shows it as open and the next start opens it.
pub fn forget_vault(config_dir: &Path, path: &str) -> Result<()> {
    update_config(config_dir, |config| config.recent_vaults.retain(|p| p != path))
}

// ---- the vault's own settings -----------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VaultSettings {
    pub screenshot_hotkey: Option<String>,
    pub screenshot_tool: ScreenshotTool,
    pub layout: LayoutSettings,
}

/// How the app's shell is laid out on this machine for this vault. Display only: nothing in the vault changes.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutSettings {
    #[serde(deserialize_with = "lenient")]
    pub sidebar_fold: SidebarFold,
}

/// What a folded sidebar becomes (hidden with Ctrl \, or on a window narrower than 1200 px): an icon rail with the
/// main pages and the projects' initials, or nothing at all.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarFold {
    #[default]
    Rail,
    Hide,
}

/// How a screenshot is taken: Ledger's built-in capture, or a program of the user's own (Flameshot, say). The
/// command is remembered while the built-in one is in use, so switching back and forth loses nothing.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScreenshotTool {
    pub use_command: bool,
    pub command: String,
}

pub fn settings_path(root: &Path) -> PathBuf {
    root.join(LEDGER_DIR).join("settings.json")
}

pub fn secrets_path(root: &Path) -> PathBuf {
    root.join(LEDGER_DIR).join(".env")
}

pub fn read_settings(root: &Path) -> VaultSettings {
    fs::read_to_string(settings_path(root)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// Changes some settings and leaves the others as they were.
pub fn update_settings(root: &Path, change: impl FnOnce(&mut VaultSettings)) -> Result<()> {
    let mut settings = read_settings(root);
    change(&mut settings);
    let file = settings_path(root);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::write(&file, serde_json::to_string_pretty(&settings).unwrap_or_default()).map_err(|e| Error::io(&file, e))
}

// ---- the secrets ---------------------------------------------------------------------------------------

/// One value from `.ledger/.env`; the last line that sets it wins. `None` when the file or the line isn't there.
pub fn read_secret(root: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(secrets_path(root)).ok()?;
    text.lines().filter_map(parse_line).filter(|(key, _)| *key == name).map(|(_, value)| value).last()
}

/// Sets one line in `.ledger/.env` and leaves every other line as it was. Where the system allows it only its owner
/// can read the file.
pub fn write_secret(root: &Path, name: &str, value: &str) -> Result<()> {
    let file = secrets_path(root);
    let existing = fs::read_to_string(&file).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    if lines.is_empty() {
        lines.push("# Ledger's secrets for this vault, on this machine.".into());
    }
    let line = format!("{name}={}", quote(value));
    match lines.iter().position(|l| parse_line(l).is_some_and(|(k, _)| k == name)) {
        Some(i) => lines[i] = line,
        None => lines.push(line),
    }

    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::write(&file, lines.join("\n") + "\n").map_err(|e| Error::io(&file, e))?;
    restrict_to_owner(&file);
    Ok(())
}

fn parse_line(line: &str) -> Option<(&str, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (key, value) = line.strip_prefix("export ").unwrap_or(line).split_once('=')?;
    let value = value.trim();
    let value = ['"', '\''].iter().find_map(|q| value.strip_prefix(*q).and_then(|v| v.strip_suffix(*q))).unwrap_or(value);
    Some((key.trim(), value.to_string()))
}

fn quote(value: &str) -> String {
    if value.chars().any(|c| c.is_whitespace() || c == '#' || c == '"' || c == '\'') {
        format!("\"{}\"", value.replace('"', ""))
    } else {
        value.to_string()
    }
}

#[cfg(unix)]
fn restrict_to_owner(file: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(file, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_to_owner(_file: &Path) {}
