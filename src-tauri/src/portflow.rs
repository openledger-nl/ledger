//! Portflow: the settings screen's commands, the screens' reads, and the background worker.
//!
//! **Off by default, and off means off.** While Portflow is switched off (`.ledger/portflow.json`, per machine):
//! the worker isn't running, no window is opened, no request leaves (the gate in `portflow_api.rs` refuses them all),
//! the saved session is cleared, every command below except reading and changing the switch answers "Portflow is
//! switched off", and the vault
//! doesn't read Portflow's files, so it doesn't write or delete them either (`vault.rs`). Switching it on or off
//! reloads the vault, so its files come in or go out of view at once.
//!
//! Switched on, the worker pulls a round every ten minutes and when asked (signing in, linking a collection, placing
//! an evidence item, "Sync now"). It reports through the `portflow-status` event. Without a session it waits: the
//! session is refreshed in a hidden window before it runs out, and when that can't be done silently the screen says to
//! sign in again. Phase 1 only reads: no write to Portflow is built, and the access setting stays read only.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{Error, Result};
use crate::portflow_api::{Gate, HttpPortflow, PResult, Portflow, PortflowError};
use crate::portflow_auth::{self as auth, Auth, Session};
use crate::portflow_gamma::{self as gamma, EvidenceChoice, GammaGoals, GammaView, SectionView};
use crate::portflow_config::{self as config, Portfolio, PortflowSettings, WRITE_KINDS};
use crate::portflow_store::{self as store, CollectionView, EvidenceView, Imported, PersonView, RecentFeedback};
use crate::portflow_sync::{self, CoreLedger, Report};
use crate::state::{App, Core};
use crate::{ids, threads};

// ---- status -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortflowStatus {
    /// "off", "signed-out", "idle", "syncing" or "error".
    pub state: String,
    pub signed_in: bool,
    /// When the session ends, in milliseconds since the epoch.
    pub expires_at: Option<i64>,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub summary: Option<String>,
    pub report: Option<Report>,
}

pub type Emit = Arc<dyn Fn(PortflowStatus) + Send + Sync>;
/// Makes the client for a round. Tests supply a fake one.
pub type Connect = Arc<dyn Fn(&Session, Gate) -> PResult<Box<dyn Portflow>> + Send + Sync>;
/// Gets a fresh session without asking the person (the hidden window). `None` in tests.
pub type Refresh = Option<Arc<dyn Fn(&str) -> PResult<Session> + Send + Sync>>;

pub fn connect_to_portflow() -> Connect {
    Arc::new(|session, gate| Ok(Box::new(HttpPortflow::new(&session.credentials(), gate)?)))
}

pub const POLL: Duration = Duration::from_secs(10 * 60);
pub const RETRY: Duration = Duration::from_secs(2 * 60);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ---- the worker ---------------------------------------------------------------------------------------

enum Msg {
    Now,
}

/// The worker, when it runs, and what it last reported. Kept as app state.
pub struct PortflowSyncer {
    tx: Mutex<Option<Sender<Msg>>>,
    status: Arc<Mutex<PortflowStatus>>,
    emit: Emit,
    pub auth: Arc<Auth>,
    core: Arc<Mutex<Core>>,
    changed: Arc<dyn Fn() + Send + Sync>,
    connect: Connect,
    refresh: Refresh,
    poll: Duration,
}

fn update(status: &Mutex<PortflowStatus>, emit: &Emit, change: impl FnOnce(&mut PortflowStatus)) {
    let snapshot = {
        let mut guard = lock(status);
        change(&mut guard);
        guard.clone()
    };
    emit(snapshot);
}

impl PortflowSyncer {
    pub fn new(core: Arc<Mutex<Core>>, emit: Emit, changed: Arc<dyn Fn() + Send + Sync>, connect: Connect, refresh: Refresh, poll: Duration) -> PortflowSyncer {
        PortflowSyncer {
            tx: Mutex::new(None),
            status: Arc::new(Mutex::new(PortflowStatus { state: "off".into(), ..PortflowStatus::default() })),
            emit,
            auth: Arc::new(Auth::default()),
            core,
            changed,
            connect,
            refresh,
            poll,
        }
    }

    fn root(&self) -> PathBuf {
        lock(&self.core).vault.root().to_path_buf()
    }

    pub fn status(&self) -> PortflowStatus {
        let mut status = lock(&self.status).clone();
        let session = self.auth.session();
        status.signed_in = session.as_ref().is_some_and(|s| !s.expired(now_ms()));
        status.expires_at = session.and_then(|s| s.expires_at);
        status
    }

    /// Whether the worker is running.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn running(&self) -> bool {
        lock(&self.tx).is_some()
    }

    /// Starts or stops the worker to match the switch, and with it on, starts a round.
    pub fn reconfigure(&self) {
        let enabled = config::is_enabled(&self.root());
        if enabled && self.auth.session().is_none()
            && let Some(saved) = auth::load_session(&self.root(), now_ms())
        {
            self.auth.set(saved);
        }
        let root = self.root();
        if !enabled {
            // Dropping the sender ends the worker.
            lock(&self.tx).take();
            self.auth.forget();
            auth::clear_session(&root);
            update(&self.status, &self.emit, |s| *s = PortflowStatus { state: "off".into(), ..PortflowStatus::default() });
            return;
        }
        let mut tx = lock(&self.tx);
        if tx.is_none() {
            let (sender, receiver) = channel();
            let worker = Worker {
                core: self.core.clone(),
                status: self.status.clone(),
                emit: self.emit.clone(),
                auth: self.auth.clone(),
                changed: self.changed.clone(),
                connect: self.connect.clone(),
                refresh: self.refresh.clone(),
            };
            let poll = self.poll;
            std::thread::spawn(move || worker.run(receiver, poll));
            *tx = Some(sender);
        }
        if let Some(tx) = tx.as_ref() {
            let _ = tx.send(Msg::Now);
        }
    }

    /// A round right away (does nothing while switched off).
    pub fn now(&self) {
        if let Some(tx) = lock(&self.tx).as_ref() {
            let _ = tx.send(Msg::Now);
        }
    }
}

struct Worker {
    core: Arc<Mutex<Core>>,
    status: Arc<Mutex<PortflowStatus>>,
    emit: Emit,
    auth: Arc<Auth>,
    changed: Arc<dyn Fn() + Send + Sync>,
    connect: Connect,
    refresh: Refresh,
}

impl Worker {
    fn run(self, rx: Receiver<Msg>, poll: Duration) {
        let mut wait = Duration::ZERO;
        loop {
            match rx.recv_timeout(wait) {
                Ok(Msg::Now) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            // Several requests at once make one round.
            while rx.try_recv().is_ok() {}
            wait = match self.round() {
                Ok(()) => poll,
                Err(_) => RETRY.min(poll),
            };
        }
    }

    fn set(&self, change: impl FnOnce(&mut PortflowStatus)) {
        update(&self.status, &self.emit, change);
    }

    fn signed_out(&self, message: Option<String>) {
        self.auth.forget();
        auth::clear_session(&lock(&self.core).vault.root().to_path_buf());
        self.set(|s| {
            s.state = "signed-out".into();
            s.signed_in = false;
            s.expires_at = None;
            s.last_error = message;
        });
    }

    fn round(&self) -> PResult<()> {
        let root = lock(&self.core).vault.root().to_path_buf();
        let settings = config::read_settings(&root);
        if !settings.enabled {
            self.set(|s| s.state = "off".into());
            return Ok(());
        }
        let session = match self.auth.session() {
            Some(session) if !session.ending(now_ms()) => session,
            Some(_) | None => match (&self.refresh, self.auth.session().is_some() || settings.portfolio.is_some()) {
                // Signed in before (on this run, or a portfolio is known): try to get a session without asking.
                (Some(refresh), true) => match refresh(&settings.launch_url) {
                    Ok(session) => {
                        if let Err(e) = auth::save_session(&root, &session) {
                            crate::applog::error("portflow", format!("Could not keep the Portflow session: {e}"));
                        }
                        session
                    }
                    Err(_) => {
                        self.signed_out(None);
                        return Ok(());
                    }
                },
                _ => {
                    self.signed_out(None);
                    return Ok(());
                }
            },
        };
        self.set(|s| {
            s.state = "syncing".into();
            s.signed_in = true;
            s.expires_at = session.expires_at;
        });
        let result = (self.connect)(&session, Gate::new(settings.enabled, &settings.access)).and_then(|client| {
            let ledger = CoreLedger { core: self.core.clone(), changed: self.changed.clone() };
            portflow_sync::run(&*client, &ledger, &settings)
        });
        match result {
            Ok(report) => {
                if report.vault_changed() {
                    (self.changed)();
                }
                let summary = report.summary();
                crate::applog::debug("portflow", format!("Round done: {summary}"));
                self.set(|s| {
                    if s.last_error.take().is_some() {
                        crate::applog::info("portflow", "Syncing with Portflow works again");
                    }
                    s.state = "idle".into();
                    s.last_sync_at = Some(ids::now());
                    s.summary = Some(summary);
                    s.report = Some(report);
                });
                Ok(())
            }
            Err(PortflowError::SignedOut) => {
                self.signed_out(Some(PortflowError::SignedOut.to_string()));
                Err(PortflowError::SignedOut)
            }
            Err(PortflowError::Off) => {
                self.set(|s| s.state = "off".into());
                Ok(())
            }
            Err(e) => {
                let message = e.to_string();
                self.set(|s| {
                    s.state = "error".into();
                    if s.last_error.as_deref() != Some(message.as_str()) {
                        crate::applog::error("portflow", &message);
                    }
                    s.last_error = Some(message);
                });
                Err(e)
            }
        }
    }
}

/// What `lib.rs` calls: makes the worker (not started until Portflow is switched on) and keeps it as app state.
pub fn start_app(handle: &AppHandle, core: Arc<Mutex<Core>>) {
    let emitter = handle.clone();
    let notifier = handle.clone();
    let refresher = handle.clone();
    let syncer = PortflowSyncer::new(
        core,
        Arc::new(move |status| {
            let _ = emitter.emit("portflow-status", status);
        }),
        Arc::new(move || {
            notifier.state::<App>().syncer.dirty();
            let _ = notifier.emit("vault-changed", ());
        }),
        connect_to_portflow(),
        Some(Arc::new(move |launch_url: &str| {
            let auth = refresher.state::<PortflowSyncer>().auth.clone();
            auth::sign_in(&refresher, &auth, launch_url, false)
        })),
        POLL,
    );
    handle.manage(syncer);
    handle.state::<PortflowSyncer>().reconfigure();
}

// ---- views --------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteKind {
    pub id: String,
    pub label: String,
    pub on: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkView {
    pub project: String,
    pub project_name: String,
    pub semester: String,
    pub semester_name: Option<String>,
    pub collection: i64,
    pub collection_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortflowView {
    pub enabled: bool,
    pub launch_url: String,
    pub default_launch_url: String,
    /// Read only (false) or read and write.
    pub write: bool,
    pub writes: Vec<WriteKind>,
    /// Whether writing can be switched on in this version (not yet: phase 1 only reads).
    pub writes_available: bool,
    pub portfolio: Option<Portfolio>,
    pub status: PortflowStatus,
    pub links: Vec<LinkView>,
    pub outcomes: i64,
    pub people: i64,
    pub unplaced: i64,
}

fn view(app: &App, syncer: &PortflowSyncer) -> Result<PortflowView> {
    let (root, links, outcomes, people, unplaced) = app.read(|conn, root| {
        let links = crate::db::query_all(
            conn,
            "SELECT l.project_slug, p.name, l.semester_id, s.name, l.collection_id, m.name FROM project_collections l
             JOIN projects p ON p.slug = l.project_slug LEFT JOIN semesters s ON s.id = l.semester_id
             LEFT JOIN collection_mirrors m ON m.project_slug = l.project_slug AND m.semester_id = l.semester_id
             ORDER BY p.name, s.start_date",
            [],
            |r| Ok(LinkView { project: r.get(0)?, project_name: r.get(1)?, semester: r.get(2)?, semester_name: r.get(3)?, collection: r.get(4)?, collection_name: r.get(5)? }),
        )?;
        let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0));
        Ok((
            root.to_path_buf(),
            links,
            count("SELECT COUNT(*) FROM outcomes WHERE removed = 0")?,
            count("SELECT COUNT(*) FROM portflow_people")?,
            count("SELECT COUNT(*) FROM evidence WHERE thread_slug IS NULL")?,
        ))
    })?;
    let settings = config::read_settings(&root);
    Ok(PortflowView {
        enabled: settings.enabled,
        launch_url: settings.launch_url.clone(),
        default_launch_url: config::DEFAULT_LAUNCH_URL.to_string(),
        write: settings.access.write,
        writes: WRITE_KINDS.iter().map(|(id, label)| WriteKind { id: id.to_string(), label: label.to_string(), on: settings.access.writes.contains(*id) }).collect(),
        writes_available: false,
        portfolio: settings.portfolio.clone(),
        status: syncer.status(),
        links,
        outcomes,
        people,
        unplaced,
    })
}

fn root_of(app: &App) -> PathBuf {
    app.lock().vault.root().to_path_buf()
}

/// The signed-in student's Portflow id (0 before the first sign-in).
fn me(settings: &PortflowSettings) -> i64 {
    settings.portfolio.as_ref().map_or(0, |p| p.user_id)
}

/// Every Portflow command but the switch itself starts here.
fn require_on(root: &Path) -> Result<PortflowSettings> {
    let settings = config::read_settings(root);
    if !settings.enabled {
        return Err(PortflowError::Off.into());
    }
    Ok(settings)
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|e| Error::sync(format!("That stopped unexpectedly: {e}")))?
}

fn reload(app: &App) -> Result<()> {
    app.lock().reload()
}

// ---- commands: the switch and signing in --------------------------------------------------------------

#[tauri::command]
pub fn portflow_get(app: State<App>, syncer: State<PortflowSyncer>) -> Result<PortflowView> {
    view(&app, &syncer)
}

#[tauri::command]
pub fn portflow_status(syncer: State<PortflowSyncer>) -> PortflowStatus {
    syncer.status()
}

/// Switches Portflow on or off. Off stops the worker, drops and clears the session and closes the sign-in window; with `forget`,
/// the window's cookies go too, so the next time starts with a fresh Fontys login.
#[tauri::command]
pub async fn portflow_set_enabled(handle: AppHandle, app: State<'_, App>, syncer: State<'_, PortflowSyncer>, enabled: bool, forget: Option<bool>) -> Result<PortflowView> {
    let root = root_of(&app);
    config::update_settings(&root, |s| s.enabled = enabled)?;
    if !enabled {
        auth::close_window(&handle);
        if forget.unwrap_or(false) {
            let (handle, auth) = (handle.clone(), syncer.auth.clone());
            blocking(move || {
                auth::forget_everything(&handle, &auth);
                Ok(())
            })
            .await?;
        }
    }
    // Its files come into view, or go out of it.
    reload(&app)?;
    syncer.reconfigure();
    let _ = handle.emit("vault-changed", ());
    crate::applog::info("portflow", if enabled { "Portflow switched on" } else { "Portflow switched off" });
    view(&app, &syncer)
}

#[tauri::command]
pub fn portflow_set_launch_url(app: State<App>, syncer: State<PortflowSyncer>, url: String) -> Result<PortflowView> {
    let root = root_of(&app);
    require_on(&root)?;
    let url = if url.trim().is_empty() { config::DEFAULT_LAUNCH_URL.to_string() } else { config::check_launch_url(&url)? };
    config::update_settings(&root, |s| s.launch_url = url)?;
    view(&app, &syncer)
}

/// Opens the sign-in window and waits until the Fontys login is done and Portflow handed over its session.
#[tauri::command]
pub async fn portflow_sign_in(handle: AppHandle, app: State<'_, App>, syncer: State<'_, PortflowSyncer>) -> Result<PortflowView> {
    let root = root_of(&app);
    let settings = require_on(&root)?;
    let auth = syncer.auth.clone();
    let connect = syncer.connect.clone();
    let gate = Gate::new(true, &settings.access);
    let portfolio = blocking(move || {
        let session = auth::sign_in(&handle, &auth, &settings.launch_url, true)?;
        let client = connect(&session, gate)?;
        let me = client.current_user()?;
        let id = session.portfolio.or(settings.portfolio.as_ref().map(|p| p.id)).ok_or_else(|| {
            PortflowError::message("Signed in, but Ledger couldn't see which portfolio is yours. Open your portfolio in the window and try again.")
        })?;
        // The portfolio must be one this account can read.
        client.sections(id)?;
        Ok(Portfolio { id, user_id: me.id, user_name: me.name })
    })
    .await;
    let portfolio = match portfolio {
        Ok(p) => p,
        Err(e) => {
            syncer.auth.forget();
            return Err(e);
        }
    };
    if let Some(session) = syncer.auth.session() {
        auth::save_session(&root, &session)?;
    }
    crate::applog::info("portflow", format!("Signed in to Portflow as {}", portfolio.user_name));
    config::update_settings(&root, |s| s.portfolio = Some(portfolio))?;
    syncer.reconfigure();
    view(&app, &syncer)
}

/// Drops the session. With `forget`, also the sign-in window's cookies, and which portfolio is yours.
#[tauri::command]
pub async fn portflow_sign_out(handle: AppHandle, app: State<'_, App>, syncer: State<'_, PortflowSyncer>, forget: Option<bool>) -> Result<PortflowView> {
    let root = root_of(&app);
    syncer.auth.forget();
    auth::clear_session(&root);
    auth::close_window(&handle);
    if forget.unwrap_or(false) {
        let auth = syncer.auth.clone();
        blocking(move || {
            auth::forget_everything(&handle, &auth);
            Ok(())
        })
        .await?;
        config::update_settings(&root, |s| s.portfolio = None)?;
    }
    update(&syncer.status, &syncer.emit, |s| {
        if s.state != "off" {
            s.state = "signed-out".into();
        }
    });
    view(&app, &syncer)
}

#[tauri::command]
pub fn portflow_sync_now(app: State<App>, syncer: State<PortflowSyncer>) -> Result<()> {
    require_on(&root_of(&app))?;
    if !syncer.auth.signed_in(now_ms()) {
        return Err(Error::invalid("Sign in to Portflow first"));
    }
    syncer.now();
    Ok(())
}

// ---- commands: linking --------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionChoice {
    pub id: i64,
    pub name: String,
    pub subtitle: String,
    pub section: String,
    pub evidence_count: i64,
    /// The project (and semester) it is linked to, if any.
    pub linked_project: Option<String>,
    pub linked_semester: Option<String>,
}

/// The portfolio's collections, by section, to pick from when linking a project. Asked from Portflow.
#[tauri::command]
pub async fn portflow_collections(app: State<'_, App>, syncer: State<'_, PortflowSyncer>) -> Result<Vec<CollectionChoice>> {
    let root = root_of(&app);
    let settings = require_on(&root)?;
    let Some(session) = syncer.auth.session().filter(|s| !s.expired(now_ms())) else {
        return Err(Error::invalid("Sign in to Portflow first"));
    };
    let links = app.read(|conn, _| store::collection_links(conn))?;
    let connect = syncer.connect.clone();
    blocking(move || {
        let p = settings.portfolio.as_ref().map(|p| p.id).ok_or_else(|| Error::invalid("Sign in to Portflow first"))?;
        let client = connect(&session, Gate::new(true, &settings.access))?;
        let sections: std::collections::HashMap<i64, (i64, String)> = client.sections(p)?.into_iter().map(|s| (s.id, (s.position, s.name))).collect();
        let mut out: Vec<(i64, i64, CollectionChoice)> = client
            .collections(p)?
            .into_iter()
            .map(|c| {
                let (position, section) = c.section_id.and_then(|s| sections.get(&s).cloned()).unwrap_or((i64::MAX, String::new()));
                let link = links.iter().find(|l| l.collection == c.id);
                (
                    position,
                    c.position,
                    CollectionChoice {
                        id: c.id,
                        name: c.name,
                        subtitle: c.subtitle.unwrap_or_default(),
                        section,
                        evidence_count: c.evidence_count,
                        linked_project: link.map(|l| l.project.clone()),
                        linked_semester: link.map(|l| l.semester.clone()),
                    },
                )
            })
            .collect();
        out.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        Ok(out.into_iter().map(|(_, _, c)| c).collect())
    })
    .await
}

/// Links a project to a collection for one semester, or unlinks it (no collection).
#[tauri::command]
pub fn portflow_link_collection(app: State<App>, syncer: State<PortflowSyncer>, project: String, semester: String, collection: Option<i64>) -> Result<PortflowView> {
    require_on(&root_of(&app))?;
    app.mutate(|conn, _| {
        crate::semesters::get(conn, &semester)?;
        store::link_collection(conn, &project, &semester, collection)
    })?;
    syncer.now();
    view(&app, &syncer)
}

/// Makes a new project from one of the portfolio's collections, linked to it for `semester`. With `threads`, each of
/// its evidence items gets a thread of its own. Then a sync brings in the outcomes, versions and feedback.
#[tauri::command]
pub async fn portflow_import_collection(
    app: State<'_, App>,
    syncer: State<'_, PortflowSyncer>,
    collection: i64,
    semester: String,
    name: Option<String>,
    threads: bool,
) -> Result<Imported> {
    let root = root_of(&app);
    let settings = require_on(&root)?;
    let Some(session) = syncer.auth.session().filter(|s| !s.expired(now_ms())) else {
        return Err(Error::invalid("Sign in to Portflow first"));
    };
    app.read(|conn, _| crate::semesters::get(conn, &semester).map(|_| ()))?;
    let connect = syncer.connect.clone();
    let (found, evidence) = blocking(move || {
        let p = settings.portfolio.as_ref().map(|p| p.id).ok_or_else(|| Error::invalid("Sign in to Portflow first"))?;
        let client = connect(&session, Gate::new(true, &settings.access))?;
        let found = client.collections(p)?.into_iter().find(|c| c.id == collection).ok_or_else(|| Error::not_found("That collection isn't in your portfolio"))?;
        let evidence: Vec<_> = client.collection_evidence(p, collection)?.into_iter().map(|e| e.evidence).collect();
        Ok((found, evidence))
    })
    .await?;
    let imported = app.mutate(|conn, _| store::import_collection(conn, &found, &evidence, &semester, name.as_deref(), threads))?;
    crate::applog::info("portflow", format!("Imported the collection {} as the project {}", found.name, imported.project_name));
    syncer.now();
    Ok(imported)
}

/// Opens Portflow in the default browser, through the Canvas page that launches it: Portflow only runs inside that
/// launch, so a link straight to one evidence item can't work there (it was tried: Portflow answers "Something went
/// wrong" without Canvas's session), and the screen says which item to go to.
#[tauri::command]
pub fn portflow_open(handle: AppHandle, app: State<App>) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let settings = require_on(&root_of(&app))?;
    config::check_launch_url(&settings.launch_url)?;
    handle.opener().open_url(&settings.launch_url, None::<&str>).map_err(|e| Error::invalid(format!("Could not open the browser: {e}")))
}

// ---- commands: what the screens show ------------------------------------------------------------------

#[tauri::command]
pub fn portflow_people(app: State<App>) -> Result<Vec<PersonView>> {
    require_on(&root_of(&app))?;
    app.read(|conn, _| store::people(conn))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPortflow {
    pub collections: Vec<CollectionView>,
    pub evidence: Vec<EvidenceView>,
}

#[tauri::command]
pub fn portflow_project(app: State<App>, project: String) -> Result<ProjectPortflow> {
    let me = me(&require_on(&root_of(&app))?);
    app.read(|conn, _| {
        crate::projects::require(conn, &project)?;
        Ok(ProjectPortflow { collections: store::project_collections(conn, &project)?, evidence: store::evidence_in_project(conn, &project, me)? })
    })
}

#[tauri::command]
pub fn portflow_thread_evidence(app: State<App>, project: String, thread: String) -> Result<Vec<EvidenceView>> {
    let me = me(&require_on(&root_of(&app))?);
    app.read(|conn, _| store::evidence_in_thread(conn, &project, &thread, me))
}

/// Puts an evidence item in a thread: an existing one, or (`new_thread`) a new one with that title.
#[tauri::command]
pub fn portflow_place_evidence(
    app: State<App>,
    syncer: State<PortflowSyncer>,
    project: String,
    evidence: String,
    thread: Option<String>,
    new_thread: Option<String>,
) -> Result<Vec<EvidenceView>> {
    let me = me(&require_on(&root_of(&app))?);
    let out = app.mutate(|conn, _| {
        let thread = match new_thread.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            Some(title) => Some(threads::create(conn, &project, title)?.slug),
            None => thread.filter(|t| !t.is_empty()),
        };
        store::place_evidence(conn, &project, &evidence, thread.as_deref())?;
        store::evidence_in_project(conn, &project, me)
    })?;
    syncer.now();
    Ok(out)
}

#[tauri::command]
pub fn portflow_recent_feedback(app: State<App>, limit: Option<i64>) -> Result<Vec<RecentFeedback>> {
    let me = me(&require_on(&root_of(&app))?);
    app.read(|conn, _| store::open_portflow_feedback(conn, limit.unwrap_or(8).clamp(1, 50), me))
}

// ---- commands: semesters, sections and Gamma (the competence matrix) ----------------------------------

/// Gamma, the competence matrix of a semester (the current one when none is given), from the vault's mirror of Portflow: no
/// request is sent, so it answers at once.
#[tauri::command]
pub fn portflow_gamma(app: State<App>, semester: Option<String>) -> Result<GammaView> {
    let me = me(&require_on(&root_of(&app))?);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    app.read(|conn, _| gamma::gamma(conn, semester.as_deref(), &today, me))
}

/// The portfolio's sections (its semesters), as last synced, with the Ledger semester each is.
#[tauri::command]
pub fn portflow_sections(app: State<App>) -> Result<Vec<SectionView>> {
    require_on(&root_of(&app))?;
    app.read(|conn, _| gamma::sections(conn))
}

/// Says which Portflow section a semester is, or that it is none (`section` null).
#[tauri::command]
pub fn portflow_link_section(app: State<App>, semester: String, section: Option<i64>) -> Result<Vec<SectionView>> {
    require_on(&root_of(&app))?;
    app.mutate(|conn, _| {
        gamma::link_section(conn, &semester, section)?;
        gamma::sections(conn)
    })
}

/// Sets a semester's intended development (Gamma's goals and overview) and answers with its matrix.
#[tauri::command]
pub fn portflow_set_intended(app: State<App>, semester: String, goals: GammaGoals) -> Result<GammaView> {
    let me = me(&require_on(&root_of(&app))?);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    app.mutate(|conn, _| {
        gamma::set_goals(conn, &semester, &goals)?;
        gamma::gamma(conn, Some(&semester), &today, me)
    })
}

/// The evidence items to pick the learning plan from.
#[tauri::command]
pub fn portflow_evidence_choices(app: State<App>) -> Result<Vec<EvidenceChoice>> {
    require_on(&root_of(&app))?;
    app.read(|conn, _| gamma::evidence_choices(conn))
}
