//! Signing in to Fontys by itself: the Fontys username and password, typed into the Fontys login page in Portflow's
//! sign-in window, so that a sign-in, and the hidden refresh after the session's eight hours, need no typing even when
//! Canvas has forgotten you.
//!
//! Off unless the person turns it on in Settings, because a saved password is the most sensitive thing Ledger can keep.
//! So:
//!
//! - It can't be a hash: the login page needs the password as typed, so Ledger must be able to read it back. It is
//!   kept in the system's own keychain (the Secret Service on Linux, so GNOME Keyring or KWallet; the Keychain on macOS;
//!   the Credential Manager on Windows), which encrypts it with a key that logging in to the computer unlocks. It is
//!   never written to the vault, its `.ledger/`, the settings or the log. With no keychain to keep it in, it isn't
//!   saved at all: there is no plain-file fallback.
//! - The settings (`.ledger/portflow.json`, this machine only) hold only the username, so the keychain is asked only
//!   when there is something in it.
//! - It is typed only into the Fontys login page: Ledger checks that the page the window loaded is
//!   `https://login.fontysict.nl`, and the script checks the page's address again before it touches anything.
//! - Once per sign-in. When the login page comes back (the password was refused, or Fontys asks for a second factor),
//!   nothing more is typed: a visible window waits for the person, a hidden refresh stops and asks for a sign-in.
//! - Ledger's own copies are wiped from memory once used ([`Zeroizing`]).

use std::path::Path;

use tauri::Url;
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::portflow_config as config;

/// The Fontys login page, the only page the password is typed into.
pub const LOGIN_HOST: &str = "login.fontysict.nl";
/// The keychain entry's service; the account is the username.
const SERVICE: &str = "nl.openledger.ledger.portflow";

/// A saved login, read back from the keychain for one sign-in.
pub struct Login {
    pub username: String,
    pub password: Zeroizing<String>,
}

/// Somewhere to keep the password. A trait so that tests can keep it in memory.
pub trait LoginStore: Send + Sync {
    fn load(&self, username: &str) -> Result<Option<Zeroizing<String>>>;
    fn save(&self, username: &str, password: &str) -> Result<()>;
    fn forget(&self, username: &str) -> Result<()>;
}

// ---- the system keychain ----------------------------------------------------------------------------

pub struct Keychain;

fn keychain_error(e: keyring::Error) -> Error {
    match e {
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => Error::invalid(
            "There is no system keychain to keep the password in (on Linux: GNOME Keyring or KWallet), or it is locked. Ledger doesn't save it anywhere else.",
        ),
        other => Error::invalid(format!("The system keychain refused: {other}")),
    }
}

impl LoginStore for Keychain {
    fn load(&self, username: &str) -> Result<Option<Zeroizing<String>>> {
        let entry = keyring::Entry::new(SERVICE, username).map_err(keychain_error)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(Zeroizing::new(password))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keychain_error(e)),
        }
    }

    fn save(&self, username: &str, password: &str) -> Result<()> {
        keyring::Entry::new(SERVICE, username).and_then(|entry| entry.set_password(password)).map_err(keychain_error)
    }

    fn forget(&self, username: &str) -> Result<()> {
        match keyring::Entry::new(SERVICE, username).and_then(|entry| entry.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keychain_error(e)),
        }
    }
}

// ---- what the app does with it ----------------------------------------------------------------------

/// Saves the login: the password in the keychain, the username in the settings. A login saved before under another
/// username is forgotten.
pub fn save(root: &Path, store: &dyn LoginStore, username: &str, password: &str) -> Result<()> {
    let username = username.trim();
    if username.is_empty() || password.is_empty() {
        return Err(Error::invalid("Fill in both your Fontys username and your password"));
    }
    store.save(username, password)?;
    let before = config::read_settings(root).saved_login;
    config::update_settings(root, |s| s.saved_login = Some(username.to_string()))?;
    if let Some(old) = before.filter(|old| old != username) {
        store.forget(&old)?;
    }
    Ok(())
}

/// Forgets the login: the password from the keychain, the username from the settings.
pub fn forget(root: &Path, store: &dyn LoginStore) -> Result<()> {
    if let Some(username) = config::read_settings(root).saved_login {
        store.forget(&username)?;
        config::update_settings(root, |s| s.saved_login = None)?;
    }
    Ok(())
}

/// The saved login, for one sign-in; `None` when none is saved or the keychain can't give it. Asks the keychain only
/// when the settings say a login is saved.
pub fn saved(root: &Path, store: &dyn LoginStore) -> Option<Login> {
    let username = config::read_settings(root).saved_login?;
    match store.load(&username) {
        Ok(Some(password)) => Some(Login { username, password }),
        Ok(None) => None,
        Err(e) => {
            crate::applog::error("portflow", format!("Could not read the saved Fontys login: {e}"));
            None
        }
    }
}

/// Whether the window has the Fontys login page open.
pub fn is_login_page(url: &Url) -> bool {
    url.scheme() == "https" && url.host_str() == Some(LOGIN_HOST) && url.port().is_none()
}

/// The script that fills in the login page and signs in. It checks the page's address before anything else, in case
/// the window moved on in between, and ticks "Keep me signed in", so that Fontys remembers the window for longer.
pub fn fill_script(login: &Login) -> Zeroizing<String> {
    let text = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    let (user, pass) = (Zeroizing::new(text(&login.username)), Zeroizing::new(text(&login.password)));
    Zeroizing::new(format!(
        r#"(function (u, p) {{
  if (location.protocol !== 'https:' || location.hostname !== '{LOGIN_HOST}' || location.port !== '') return;
  var user = document.getElementById('userNameInput');
  var pass = document.getElementById('passwordInput');
  var button = document.getElementById('submitButton');
  if (!user || !pass || !button) return;
  user.value = u;
  pass.value = p;
  var keep = document.getElementById('kmsiInput');
  if (keep) keep.checked = true;
  button.click();
}})({}, {});"#,
        *user, *pass
    ))
}

// ---- one for tests ----------------------------------------------------------------------------------

/// Passwords in memory, by username.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryLoginStore {
    pub passwords: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
}

#[cfg(test)]
impl LoginStore for MemoryLoginStore {
    fn load(&self, username: &str) -> Result<Option<Zeroizing<String>>> {
        Ok(self.passwords.lock().unwrap().get(username).cloned().map(Zeroizing::new))
    }

    fn save(&self, username: &str, password: &str) -> Result<()> {
        self.passwords.lock().unwrap().insert(username.to_string(), password.to_string());
        Ok(())
    }

    fn forget(&self, username: &str) -> Result<()> {
        self.passwords.lock().unwrap().remove(username);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_fontys_login_page_gets_the_password() {
        assert!(is_login_page(&"https://login.fontysict.nl/adfs/ls/?SAMLRequest=abc".parse().unwrap()));
        for no in [
            "http://login.fontysict.nl/adfs/ls/",
            "https://login.fontysict.nl:8443/adfs/ls/",
            "https://login.fontysict.nl.example.com/",
            "https://evil.example/login.fontysict.nl",
            "https://fontysict.instructure.com/login",
        ] {
            assert!(!is_login_page(&no.parse().unwrap()), "{no}");
        }
    }

    #[test]
    fn the_script_checks_the_page_first_and_keeps_the_password_a_string() {
        let login = Login { username: "i123456@fontysict.nl".into(), password: Zeroizing::new(r#"it's "x" \ </script> two\nlines"#.into()) };
        let script = fill_script(&login);
        let check = script.find("location.hostname !== 'login.fontysict.nl'").expect("checks the page");
        assert!(check < script.find("getElementById").unwrap(), "checks the address before it touches the page");
        assert!(script.ends_with(r#"})("i123456@fontysict.nl", "it's \"x\" \\ </script> two\\nlines");"#), "{}", *script);
    }

    #[test]
    fn the_password_goes_in_the_keychain_and_only_the_username_in_the_settings() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let store = MemoryLoginStore::default();
        assert!(saved(root, &store).is_none());

        save(root, &store, " i123456@fontysict.nl ", "secret").unwrap();
        assert_eq!(config::read_settings(root).saved_login.as_deref(), Some("i123456@fontysict.nl"));
        let on_disk = std::fs::read_to_string(root.join(".ledger/portflow.json")).unwrap();
        assert!(!on_disk.contains("secret"), "the password is not in the settings: {on_disk}");
        let login = saved(root, &store).expect("a login");
        assert_eq!((login.username.as_str(), login.password.as_str()), ("i123456@fontysict.nl", "secret"));

        // Another account replaces the first, which is forgotten.
        save(root, &store, "i654321@fontysict.nl", "other").unwrap();
        assert_eq!(store.passwords.lock().unwrap().keys().collect::<Vec<_>>(), ["i654321@fontysict.nl"]);

        forget(root, &store).unwrap();
        assert!(store.passwords.lock().unwrap().is_empty());
        assert_eq!(config::read_settings(root).saved_login, None);
        assert!(saved(root, &store).is_none());

        assert!(save(root, &store, "  ", "secret").is_err());
        assert!(save(root, &store, "i123456@fontysict.nl", "").is_err());
    }

    /// Against this machine's real keychain, with a throwaway entry: `cargo test -- --ignored keychain`.
    #[test]
    #[ignore = "needs a system keychain"]
    fn the_system_keychain_keeps_and_forgets_a_password() {
        let user = "ledger-test-throwaway";
        Keychain.save(user, "not-a-real-password").unwrap();
        assert_eq!(Keychain.load(user).unwrap().as_deref().map(String::as_str), Some("not-a-real-password"));
        Keychain.forget(user).unwrap();
        assert!(Keychain.load(user).unwrap().is_none());
        Keychain.forget(user).unwrap();
    }

    #[test]
    fn a_username_without_a_password_in_the_keychain_signs_in_as_usual() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryLoginStore::default();
        config::update_settings(dir.path(), |s| s.saved_login = Some("i123456@fontysict.nl".into())).unwrap();
        assert!(saved(dir.path(), &store).is_none());
    }
}
