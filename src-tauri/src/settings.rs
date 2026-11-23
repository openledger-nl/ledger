//! Where the vault is, and what belongs to this machine rather than to the vault.
//!
//! The vault's location is the one thing that can't live in the vault, so it is kept in a small `config.json` in
//! the app's config folder, with the vaults opened before (for the vault picker) and the other choices that belong to
//! this machine's copy of Ledger rather than to a vault: whether it looks for updates by itself.
//!
//! What belongs to this machine's copy of one vault is kept outside the vault too, so that the cloud storage the
//! vault is in never uploads it, in a folder of the app's config folder named after the vault's id (see `marker`):
//!
//! ```text
//! <config>/vaults/<vault id>/settings.json   the screenshot hotkey and tool, the folded sidebar
//! <config>/vaults/<vault id>/secrets.env     the secrets, only when the system has no keychain to keep them in
//! <config>/vaults/<vault id>/conflicts/      versions set aside when a sync with Trello or Portflow resolved a conflict
//! ```
//!
//! and the Trello, Portflow and Claude Code settings beside them. The secrets (the Trello key and token, the
//! Portflow session) go to the system's keychain.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::onboarding::OnboardingConfig;

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

/// Where the vault is: `LEDGER_VAULT` if set (and then not changeable from the UI), else the vault opened last. `None`
/// means no vault has been chosen yet: the app starts on the picker and asks for one.
pub fn resolve_vault(config_dir: &Path) -> (Option<PathBuf>, bool) {
    if let Some(path) = std::env::var_os("LEDGER_VAULT").filter(|p| !p.is_empty()) {
        return (Some(PathBuf::from(path)), true);
    }
    (load_config(config_dir).vault_path.filter(|p| !p.trim().is_empty()).map(PathBuf::from), false)
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

// ---- what belongs to this machine's copy of a vault ---------------------------------------------------

/// Where each open vault keeps what belongs to this machine, by the vault's folder: set when a vault is opened (see
/// `state::Core`), so that everything that only has the vault's folder finds it.
static LOCAL_DIRS: LazyLock<Mutex<HashMap<PathBuf, PathBuf>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The folder for what belongs to this machine's copy of the vault in `root`: `<config>/vaults/<vault id>/`.
pub fn local_dir_in(config_dir: &Path, vault_id: &str) -> PathBuf {
    config_dir.join("vaults").join(vault_id)
}

/// Says where the vault in `root` keeps what belongs to this machine.
pub fn register_local_dir(root: &Path, dir: PathBuf) {
    LOCAL_DIRS.lock().unwrap_or_else(PoisonError::into_inner).insert(root.to_path_buf(), dir);
}

/// The folder for what belongs to this machine's copy of the vault in `root`: its settings, its secrets when there
/// is no keychain to keep them in, the bookkeeping of the syncs with Trello and Portflow and what they set aside.
/// It is outside the vault, so that the cloud storage the vault is in never uploads it, and it is the same folder
/// wherever the vault's folder is on this machine. A vault that the app didn't open (in a test) gets one in the
/// system's temporary folder, by its id, or by its path when it has no marker.
pub fn local_dir(root: &Path) -> PathBuf {
    if let Some(dir) = LOCAL_DIRS.lock().unwrap_or_else(PoisonError::into_inner).get(root) {
        return dir.clone();
    }
    let id = match crate::marker::inspect(root).marker() {
        Some(marker) => marker.id.clone(),
        None => {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            root.hash(&mut hasher);
            format!("path-{:016x}", hasher.finish())
        }
    };
    local_dir_in(&std::env::temp_dir().join("ledger-local"), &id)
}

// ---- this machine's settings for a vault ----------------------------------------------------------------

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
    local_dir(root).join("settings.json")
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

/// Every secret Ledger keeps for a vault, by name: so they can all be found, to keep them out of what a Claude Code
/// run shows.
pub const SECRET_NAMES: [&str; 4] = [
    crate::trello_config::KEY_API_KEY,
    crate::trello_config::KEY_TOKEN,
    crate::portflow_auth::SESSION_SECRET,
    crate::portflow_auth::CSRF_SECRET,
];

/// Where the secrets go when the system has no keychain: a file in the vault's folder for this machine.
pub fn secrets_path(root: &Path) -> PathBuf {
    local_dir(root).join("secrets.env")
}

/// Where the secrets of the vault in `root` are kept, for the screen: the keychain, or the file when there is none.
pub fn secrets_place(root: &Path) -> String {
    if keychain::available() { "the system keychain".to_string() } else { secrets_path(root).display().to_string() }
}

/// A secret kept for the vault in `root` on this machine. `None` when there is none (or it was set to nothing).
pub fn read_secret(root: &Path, name: &str) -> Option<String> {
    if let Some(value) = keychain::get(&keychain::account(root, name)) {
        return Some(value).filter(|v| !v.is_empty());
    }
    let text = fs::read_to_string(secrets_path(root)).ok()?;
    text.lines().filter_map(parse_line).filter(|(key, _)| *key == name).map(|(_, value)| value).next_back().filter(|v| !v.is_empty())
}

/// Keeps a secret for the vault in `root` on this machine, never in the vault: in the system's keychain (the
/// Credential Manager on Windows, the Keychain on macOS, the Secret Service on Linux), or, where there is none or it
/// refuses the value, in a file outside the vault that only this user can read. An empty value removes it.
pub fn write_secret(root: &Path, name: &str, value: &str) -> Result<()> {
    let file = secrets_path(root);
    if keychain::set(&keychain::account(root, name), value) {
        // An earlier write may have had to put it in the file: it is in the keychain now.
        if fs::read_to_string(&file).is_ok_and(|text| text.lines().filter_map(parse_line).any(|(key, _)| key == name)) {
            return write_line(&file, name, "");
        }
        return Ok(());
    }
    write_line(&file, name, value)
}

/// Sets one line of an env file and leaves every other line as it was. Where the system allows it only its owner can
/// read the file.
fn write_line(file: &Path, name: &str, value: &str) -> Result<()> {
    let existing = fs::read_to_string(file).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    if lines.is_empty() {
        lines.push("# Ledger's secrets for one vault, on this machine: kept here because the system has no keychain.".into());
    }
    let line = format!("{name}={}", quote(value));
    match lines.iter().position(|l| parse_line(l).is_some_and(|(k, _)| k == name)) {
        Some(i) => lines[i] = line,
        None => lines.push(line),
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::write(file, lines.join("\n") + "\n").map_err(|e| Error::io(file, e))?;
    restrict_to_owner(file);
    Ok(())
}

/// Every secret kept for the vault in `root`, whatever their names: what a run's output must never show.
pub fn secret_values(root: &Path) -> Vec<String> {
    let mut values: Vec<String> = SECRET_NAMES.iter().filter_map(|name| keychain::get(&keychain::account(root, name))).collect();
    let text = fs::read_to_string(secrets_path(root)).unwrap_or_default();
    values.extend(text.lines().filter_map(parse_line).map(|(_, value)| value));
    values.retain(|v| !v.is_empty());
    values
}

/// The system's keychain, by way of the `keyring` crate. Tests never touch it: there it holds nothing and takes
/// nothing, so every secret goes to the file.
mod keychain {
    use std::path::Path;

    const SERVICE: &str = "nl.openledger.ledger";

    /// The keychain entry of one secret of one vault: by the vault's id, which is the name of its folder for this
    /// machine, so it is the same wherever the vault's folder is.
    pub fn account(root: &Path, name: &str) -> String {
        let dir = super::local_dir(root);
        let id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        format!("{id}/{name}")
    }

    #[cfg(not(test))]
    pub fn get(account: &str) -> Option<String> {
        keyring::Entry::new(SERVICE, account).ok()?.get_password().ok()
    }

    #[cfg(not(test))]
    pub fn set(account: &str, value: &str) -> bool {
        let Ok(entry) = keyring::Entry::new(SERVICE, account) else { return false };
        if value.is_empty() {
            return matches!(entry.delete_credential(), Ok(()) | Err(keyring::Error::NoEntry));
        }
        entry.set_password(value).is_ok()
    }

    /// Whether there is a keychain to keep secrets in: asking for an entry that isn't there says so.
    #[cfg(not(test))]
    pub fn available() -> bool {
        keyring::Entry::new(SERVICE, "ledger-probe").is_ok_and(|entry| matches!(entry.get_password(), Ok(_) | Err(keyring::Error::NoEntry)))
    }

    #[cfg(test)]
    pub fn get(_account: &str) -> Option<String> {
        None
    }

    #[cfg(test)]
    pub fn set(_account: &str, _value: &str) -> bool {
        false
    }

    #[cfg(test)]
    pub fn available() -> bool {
        false
    }

    #[cfg(test)]
    const _: &str = SERVICE;
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
