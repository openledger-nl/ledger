//! Signing in to Portflow with the Fontys account, in a window of Ledger's own.
//!
//! Portflow is an LTI tool inside Canvas: there is no login of its own and no API key. So Ledger opens the Canvas page
//! that launches Portflow (Settings → Portflow → the launch address) in a window, and the student signs in there the
//! way they always do (Fontys, Microsoft, two-factor). Ledger never sees what is typed into that window and never
//! stores a password: it waits until Portflow's page has started and then takes the one thing it needs, the session
//! token Portflow's own web app uses (`document.body.dataset.session`, a JWT that lives eight hours).
//!
//! The hand-over goes through a script that runs in every frame of that window:
//!
//! - on Canvas, it makes the form that launches Portflow open in the window itself rather than in a frame, so that
//!   Portflow's page is the top page;
//! - on Portflow, it notes which portfolio the page reads (the id is in its API calls), waits for the session, and then
//!   navigates to `https://ledger.invalid/portflow#<session>`. That address is never loaded: Ledger stops the
//!   navigation, reads the fragment, and closes the window.
//!
//! The token is kept where the Trello key is: the vault's `.ledger/.env` (`PORTFLOW_SESSION`), on this machine only,
//! gitignored, never synced, and readable by its owner only where the system allows it. So a restart within the eight
//! hours needs no sign-in. It never goes in the log. Signing out, switching Portflow off, or Portflow refusing the
//! token clears it. The window keeps its cookies in a folder of its own (in the app's data folder, not the vault), so
//! the next sign-in, and the hidden refresh after eight hours, go through without typing anything as long as the Canvas
//! session lasts. "Sign out and forget" removes that folder.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::portflow_api::{Credentials, PResult, PortflowError};
use crate::settings;

pub const LOGIN_LABEL: &str = "portflow-login";
const SENTINEL_HOST: &str = "ledger.invalid";
/// How long a visible sign-in may take, and a hidden refresh.
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const REFRESH_TIMEOUT: Duration = Duration::from_secs(60);
/// A session this close to its end is refreshed before it is used.
pub const REFRESH_MARGIN_MS: i64 = 10 * 60 * 1000;

/// Runs in every frame of the sign-in window. Ends by navigating to the sentinel address with the session.
const SCRIPT: &str = r#"(function () {
  if (window.__ledgerPortflow) return;
  window.__ledgerPortflow = true;
  var host = location.hostname;
  if (/(^|\.)instructure\.com$/.test(host)) {
    var fix = function (form) {
      try { if (form && String(form.action || '').indexOf('drieam.app') >= 0) form.target = '_self'; } catch (e) {}
    };
    var submit = HTMLFormElement.prototype.submit;
    HTMLFormElement.prototype.submit = function () { fix(this); return submit.apply(this, arguments); };
    if (HTMLFormElement.prototype.requestSubmit) {
      var requestSubmit = HTMLFormElement.prototype.requestSubmit;
      HTMLFormElement.prototype.requestSubmit = function () { fix(this); return requestSubmit.apply(this, arguments); };
    }
    document.addEventListener('submit', function (e) { fix(e.target); }, true);
    return;
  }
  if (host !== 'portfolio.drieam.app') return;
  var portfolio = null;
  var sniff = function (url) {
    try { var m = String(url).match(/\/api\/v1\/portfolios\/(\d+)(\/|\?|$)/); if (m) portfolio = Number(m[1]); } catch (e) {}
  };
  if (window.fetch) {
    var f = window.fetch;
    window.fetch = function (input) { sniff(input && input.url ? input.url : input); return f.apply(this, arguments); };
  }
  var open = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url) { sniff(url); return open.apply(this, arguments); };
  var tries = 0;
  var timer = setInterval(function () {
    tries++;
    var body = document.body;
    var session = body && body.dataset && body.dataset.session;
    if (!session || (!portfolio && tries < 60)) return;
    clearInterval(timer);
    var meta = document.querySelector('meta[name="csrf-token"]');
    var data = { session: session, csrf: meta ? meta.content : null, portfolio: portfolio, expiresAt: body.dataset.sessionExpiresAt || null };
    var target = 'https://ledger.invalid/portflow#' + encodeURIComponent(JSON.stringify(data));
    try { window.top.location.href = target; } catch (e) { location.href = target; }
  }, 250);
})();"#;

/// The session. Deliberately not `Debug` or `Serialize`, so it can't end up in a log or an answer to the screen.
#[derive(Clone)]
pub struct Session {
    pub token: String,
    pub csrf: Option<String>,
    /// The portfolio Portflow's page read, when it could be seen.
    pub portfolio: Option<i64>,
    /// When the token stops working, in milliseconds since the epoch.
    pub expires_at: Option<i64>,
}

impl Session {
    pub fn credentials(&self) -> Credentials {
        Credentials { token: self.token.clone(), csrf: self.csrf.clone() }
    }

    pub fn expired(&self, now_ms: i64) -> bool {
        self.expires_at.is_some_and(|at| at <= now_ms)
    }

    pub fn ending(&self, now_ms: i64) -> bool {
        self.expires_at.is_some_and(|at| at - REFRESH_MARGIN_MS <= now_ms)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandOver {
    session: String,
    csrf: Option<String>,
    portfolio: Option<i64>,
    expires_at: Option<String>,
}

/// The `exp` of a JWT, in milliseconds. The token isn't verified here (Portflow does that); this is only for knowing
/// when to refresh.
pub fn jwt_expiry_ms(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims["exp"].as_i64().map(|s| s * 1000)
}

fn parse_hand_over(fragment: &str) -> Option<Session> {
    let decoded = percent_decode(fragment)?;
    let data: HandOver = serde_json::from_str(&decoded).ok()?;
    let token = data.session.trim().to_string();
    // A JWT is three base64url parts; anything else isn't a session.
    if token.split('.').count() != 3 || !token.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) {
        return None;
    }
    let expires_at = jwt_expiry_ms(&token).or_else(|| {
        data.expires_at
            .as_deref()
            .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
            .map(|at| at.timestamp_millis())
    });
    Some(Session { token, csrf: data.csrf.filter(|c| !c.is_empty()), portfolio: data.portfolio.filter(|p| *p > 0), expires_at })
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

// ---- kept in .ledger/.env -------------------------------------------------------------------------------

const SESSION_SECRET: &str = "PORTFLOW_SESSION";
const CSRF_SECRET: &str = "PORTFLOW_CSRF";

/// Saves the session in the vault's `.ledger/.env`, next to the Trello key.
pub fn save_session(root: &Path, session: &Session) -> crate::error::Result<()> {
    settings::write_secret(root, SESSION_SECRET, &session.token)?;
    settings::write_secret(root, CSRF_SECRET, session.csrf.as_deref().unwrap_or_default())
}

/// The saved session, if there is one that hasn't run out.
pub fn load_session(root: &Path, now_ms: i64) -> Option<Session> {
    let token = settings::read_secret(root, SESSION_SECRET).filter(|t| t.split('.').count() == 3)?;
    let csrf = settings::read_secret(root, CSRF_SECRET).filter(|c| !c.is_empty());
    let session = Session { expires_at: jwt_expiry_ms(&token), token, csrf, portfolio: None };
    (!session.expired(now_ms)).then_some(session)
}

/// Removes the saved session (an empty value, as the Trello key is removed).
pub fn clear_session(root: &Path) {
    if settings::read_secret(root, SESSION_SECRET).is_some_and(|t| !t.is_empty()) {
        let _ = settings::write_secret(root, SESSION_SECRET, "");
        let _ = settings::write_secret(root, CSRF_SECRET, "");
    }
}

type Waiter = Sender<std::result::Result<Session, String>>;

/// The session and the sign-in in progress, if any. Kept as app state.
#[derive(Default)]
pub struct Auth {
    session: Mutex<Option<Session>>,
    waiting: Arc<Mutex<Vec<Waiter>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Auth {
    pub fn session(&self) -> Option<Session> {
        lock(&self.session).clone()
    }

    pub fn set(&self, session: Session) {
        *lock(&self.session) = Some(session);
    }

    /// Drops the token from memory (the saved one is cleared by [`clear_session`]).
    pub fn forget(&self) {
        *lock(&self.session) = None;
    }

    pub fn signed_in(&self, now_ms: i64) -> bool {
        self.session().is_some_and(|s| !s.expired(now_ms))
    }
}

/// Where the sign-in window keeps its cookies: the app's data folder, never the vault.
pub fn data_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join("portflow-login"))
}

fn finish(waiting: &Mutex<Vec<Waiter>>, outcome: std::result::Result<Session, String>) {
    for waiter in lock(waiting).drain(..) {
        let _ = waiter.send(outcome.clone());
    }
}

/// Opens the sign-in window (or, `visible` false, a hidden one for a refresh) and waits for the session. Blocking: call
/// it off the main thread.
pub fn sign_in(app: &AppHandle, auth: &Auth, launch_url: &str, visible: bool) -> PResult<Session> {
    let url = tauri::Url::parse(launch_url).map_err(|_| PortflowError::message("The Portflow launch address isn't a web address"))?;
    if url.scheme() != "https" {
        return Err(PortflowError::message("The Portflow launch address must start with https://"));
    }

    let (tx, rx) = channel();
    let first = {
        let mut waiting = lock(&auth.waiting);
        waiting.push(tx);
        waiting.len() == 1
    };

    if let Some(window) = app.get_webview_window(LOGIN_LABEL) {
        if visible {
            let _ = window.show();
            let _ = window.set_focus();
        }
    } else if first {
        let waiting = auth.waiting.clone();
        let closer = app.clone();
        let mut builder = WebviewWindowBuilder::new(app, LOGIN_LABEL, WebviewUrl::External(url))
            .title("Sign in to Portflow")
            .inner_size(560.0, 760.0)
            .visible(visible)
            .initialization_script_for_all_frames(SCRIPT)
            .on_navigation(move |target| {
                if target.host_str() != Some(SENTINEL_HOST) {
                    return target.scheme() == "https" || target.scheme() == "about";
                }
                let outcome = target.fragment().and_then(parse_hand_over).ok_or_else(|| "Portflow didn't hand over a session".to_string());
                finish(&waiting, outcome);
                let closer = closer.clone();
                std::thread::spawn(move || {
                    if let Some(window) = closer.get_webview_window(LOGIN_LABEL) {
                        let _ = window.destroy();
                    }
                });
                false
            });
        if let Some(dir) = data_dir(app) {
            builder = builder.data_directory(dir);
        }
        let window = builder.build().map_err(|e| PortflowError::message(format!("Could not open the sign-in window: {e}")))?;
        let waiting = auth.waiting.clone();
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                finish(&waiting, Err("The sign-in window was closed before signing in finished".into()));
            }
        });
    }

    let timeout = if visible { SIGN_IN_TIMEOUT } else { REFRESH_TIMEOUT };
    let outcome = rx.recv_timeout(timeout);
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(_) => {
            if let Some(window) = app.get_webview_window(LOGIN_LABEL) {
                let _ = window.destroy();
            }
            Err(if visible { "Signing in took too long".to_string() } else { "Portflow needs you to sign in again".to_string() })
        }
    };
    match outcome {
        Ok(session) => {
            auth.set(session.clone());
            Ok(session)
        }
        Err(_) if !visible => Err(PortflowError::SignedOut),
        Err(message) => Err(PortflowError::Message(message)),
    }
}

/// Closes the sign-in window, if it is open.
pub fn close_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LOGIN_LABEL) {
        let _ = window.destroy();
    }
}

/// Forgets the sign-in completely: the token, and the window's cookies and data.
pub fn forget_everything(app: &AppHandle, auth: &Auth) {
    auth.forget();
    close_window(app);
    if let Some(dir) = data_dir(app) {
        // The window may only just have gone; give the webview a moment to let go of its files.
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(exp: i64) -> String {
        let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        format!("{}.{}.sig", enc.encode(br#"{"alg":"HS256"}"#), enc.encode(format!(r#"{{"exp":{exp},"sub":"1"}}"#)))
    }

    #[test]
    fn reads_the_expiry_of_a_token() {
        assert_eq!(jwt_expiry_ms(&jwt(1_800_000_000)), Some(1_800_000_000_000));
        assert_eq!(jwt_expiry_ms("not a token"), None);
    }

    #[test]
    fn parses_the_hand_over() {
        let token = jwt(1_800_000_000);
        let fragment = format!("%7B%22session%22%3A%22{token}%22%2C%22csrf%22%3A%22abc%22%2C%22portfolio%22%3A22497%2C%22expiresAt%22%3Anull%7D");
        let session = parse_hand_over(&fragment).expect("a session");
        assert_eq!(session.token, token);
        assert_eq!(session.csrf.as_deref(), Some("abc"));
        assert_eq!(session.portfolio, Some(22497));
        assert_eq!(session.expires_at, Some(1_800_000_000_000));
        assert!(session.ending(1_800_000_000_000 - 60_000));
        assert!(!session.expired(1_800_000_000_000 - 60_000));
    }

    #[test]
    fn refuses_what_isnt_a_session() {
        assert!(parse_hand_over("%7B%22session%22%3A%22hello%22%7D").is_none());
        assert!(parse_hand_over("%7B%22session%22%3A%22a.b%3Cscript%3E.c%22%7D").is_none());
        assert!(parse_hand_over("garbage%").is_none());
    }
}
