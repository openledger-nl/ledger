//! Syncing with Jira: the settings screen's commands and the background worker.
//!
//! The worker works like the Trello one (`trello.rs`): shortly after the vault changes (once it has been quiet for a
//! few seconds), every few minutes to notice what changed on Jira, and when asked; it reports its progress through
//! an event, and does nothing at all until the sync is set up. It never holds the vault lock while it talks to Jira.
//!
//! Setting up takes the Jira site, an email address and an API token, then a Jira project for each Ledger project
//! that should sync, and optionally which status a column belongs to. The email address and token are kept in the
//! vault's `.ledger/.env`; the rest in `.ledger/jira.json` (see `jira_config.rs`). None of it is ever uploaded, and
//! the screen is only ever told whether a token exists, plus its last four characters.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{Error, Result};
use crate::jira_api::{HttpJira, JResult, Jira, JiraError, JiraWire, Project, Status, normalize_site, valid_email, valid_token};
use crate::jira_config::{self as config, Credentials, ProjectLink};
use crate::jira_ledger::{self, CoreLedger, Ledger};
use crate::jira_sync::{self, Mapping, Report};
use crate::models::Column;
use crate::state::{App, Core};
use crate::trello_card::Kind;
use crate::trello_config::hint;
use crate::trello_ledger;
use crate::{ids, projects};

// ---- status -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraStatus {
    /// There is a site, credentials and at least one linked project.
    pub configured: bool,
    /// "disabled", "idle", "syncing" or "error".
    pub state: String,
    /// Changes made here that haven't been looked at yet.
    pub pending: bool,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub summary: Option<String>,
    /// What the last round did, with the things it wants a person to know.
    pub report: Option<Report>,
}

pub enum JiraEvent {
    Status(JiraStatus),
    /// A round changed the vault: screens should refresh.
    VaultChanged,
}

pub type Emit = Arc<dyn Fn(JiraEvent) + Send + Sync>;

/// How the worker is paced. Tests use short ones.
#[derive(Clone, Copy)]
pub struct Timing {
    /// How long the vault must stay quiet before a round, and the longest a steady stream of edits can postpone it.
    pub debounce: Duration,
    pub max_debounce: Duration,
    /// How often to look for changes made on Jira, and how soon to try again after a failure.
    pub poll: Duration,
    pub retry: Duration,
    /// The shortest wait after Jira said to slow down.
    pub rate_limited: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            debounce: Duration::from_secs(5),
            max_debounce: Duration::from_secs(60),
            poll: Duration::from_secs(180),
            retry: Duration::from_secs(60),
            rate_limited: Duration::from_secs(120),
        }
    }
}

/// Makes the client for a round from the saved site and credentials. Tests supply a fake one.
pub type Connect = Arc<dyn Fn(&str, &Credentials) -> JResult<Box<dyn Jira>> + Send + Sync>;

fn connect_to_jira() -> Connect {
    Arc::new(|site, credentials| Ok(Box::new(HttpJira::new(site, &credentials.email, &credentials.token)?)))
}

// ---- the worker ---------------------------------------------------------------------------------------

enum Msg {
    Dirty,
    Now,
}

pub struct JiraSyncer {
    tx: Mutex<Sender<Msg>>,
    status: Arc<Mutex<JiraStatus>>,
    emit: Emit,
}

/// How the rest of the app tells the worker the vault changed, without having to hold on to it.
static NOTIFY: OnceLock<Mutex<Sender<Msg>>> = OnceLock::new();

/// Something in the vault changed (called after every change made through the app). Does nothing when the Jira
/// sync isn't running.
pub fn notify_vault_changed() {
    if let Some(tx) = NOTIFY.get() {
        let _ = tx.lock().unwrap_or_else(PoisonError::into_inner).send(Msg::Dirty);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn update(status: &Mutex<JiraStatus>, emit: &Emit, change: impl FnOnce(&mut JiraStatus)) {
    let snapshot = {
        let mut guard = lock(status);
        change(&mut guard);
        guard.clone()
    };
    emit(JiraEvent::Status(snapshot));
}

impl JiraSyncer {
    /// Something in the vault changed (the app reaches the worker through [`notify_vault_changed`]).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn dirty(&self) {
        let _ = lock(&self.tx).send(Msg::Dirty);
    }

    /// Sync right away.
    pub fn now(&self) {
        let _ = lock(&self.tx).send(Msg::Now);
    }

    pub fn status(&self) -> JiraStatus {
        lock(&self.status).clone()
    }

    /// The settings changed: records whether there is something to sync and, if there is, starts with a round.
    pub fn reconfigure(&self, root: &Path) {
        let configured = config::is_configured(root);
        update(&self.status, &self.emit, |s| {
            s.configured = configured;
            s.pending = false;
            s.last_error = None;
            s.state = if configured { "idle" } else { "disabled" }.to_string();
            if !configured {
                s.summary = None;
                s.last_sync_at = None;
                s.report = None;
            }
        });
        if configured {
            self.now();
        }
    }
}

/// Starts the worker for the running app and makes it the one [`notify_vault_changed`] reaches.
pub fn start(core: Arc<Mutex<Core>>, emit: Emit, changed: Arc<dyn Fn() + Send + Sync>) -> JiraSyncer {
    let syncer = spawn(core.clone(), emit, changed, connect_to_jira(), Timing::default());
    let _ = NOTIFY.set(Mutex::new(lock(&syncer.tx).clone()));
    let root = lock(&core).vault.root().to_path_buf();
    syncer.reconfigure(&root);
    syncer
}

/// What `lib.rs` calls: starts the worker, reports to the window, and keeps the handle as app state.
pub fn start_app(handle: &AppHandle, core: Arc<Mutex<Core>>) {
    let emitter = handle.clone();
    let notifier = handle.clone();
    let syncer = start(
        core,
        Arc::new(move |event| {
            let _ = match event {
                JiraEvent::Status(status) => emitter.emit("jira-status", status),
                JiraEvent::VaultChanged => emitter.emit("vault-changed", ()),
            };
        }),
        // What a round wrote into the vault goes to GitHub like any other change.
        Arc::new(move || notifier.state::<App>().syncer.dirty()),
    );
    handle.manage(syncer);
}

pub fn spawn(core: Arc<Mutex<Core>>, emit: Emit, changed: Arc<dyn Fn() + Send + Sync>, connect: Connect, timing: Timing) -> JiraSyncer {
    let (tx, rx) = channel();
    let status = Arc::new(Mutex::new(JiraStatus { state: "disabled".into(), ..JiraStatus::default() }));
    {
        let (status, emit) = (status.clone(), emit.clone());
        std::thread::spawn(move || worker(rx, core, status, emit, changed, connect, timing));
    }
    JiraSyncer { tx: Mutex::new(tx), status, emit }
}

fn worker(rx: Receiver<Msg>, core: Arc<Mutex<Core>>, status: Arc<Mutex<JiraStatus>>, emit: Emit, changed: Arc<dyn Fn() + Send + Sync>, connect: Connect, timing: Timing) {
    let mut pending_since: Option<Instant> = None;
    // How long to wait before trying again after a failure.
    let mut retry_in: Option<Duration> = None;
    // What the vault looked like (as far as Jira is concerned) at the last round that worked.
    let mut last_digest: Option<u64> = None;

    loop {
        let wait = if pending_since.is_some() { timing.debounce } else { retry_in.unwrap_or(timing.poll) };

        // `forced`: a round that must look at Jira even if the vault looks the same as last time.
        let (run, forced) = match rx.recv_timeout(wait) {
            Ok(Msg::Dirty) => {
                let root = lock(&core).vault.root().to_path_buf();
                if config::is_configured(&root) {
                    let since = *pending_since.get_or_insert_with(Instant::now);
                    update(&status, &emit, |s| s.pending = true);
                    (since.elapsed() >= timing.max_debounce, false)
                } else {
                    (false, false)
                }
            }
            Ok(Msg::Now) => (true, true),
            Err(RecvTimeoutError::Timeout) => (true, pending_since.is_none()),
            Err(RecvTimeoutError::Disconnected) => return,
        };

        if run {
            pending_since = None;
            retry_in = match run_once(&core, &status, &emit, &changed, &connect, forced, &mut last_digest) {
                Ok(()) => None,
                Err(JiraError::RateLimited { retry_after }) => Some(retry_after.max(timing.rate_limited)),
                Err(_) => Some(timing.retry),
            };
        }
    }
}

/// One round. An error has already been put in the status.
fn run_once(
    core: &Arc<Mutex<Core>>,
    status: &Mutex<JiraStatus>,
    emit: &Emit,
    changed: &Arc<dyn Fn() + Send + Sync>,
    connect: &Connect,
    forced: bool,
    last_digest: &mut Option<u64>,
) -> JResult<()> {
    let root = lock(core).vault.root().to_path_buf();
    let settings = config::read_settings(&root);
    let credentials = config::read_credentials(&root);
    if !config::is_configured(&root) {
        update(status, emit, |s| {
            s.configured = false;
            s.state = "disabled".into();
            s.pending = false;
        });
        return Ok(());
    }

    let ledger = CoreLedger { core: core.clone(), changed: changed.clone() };
    let result: JResult<Option<Report>> = (|| {
        let snapshot = ledger.snapshot()?;
        let digest = jira_sync::digest(&snapshot, &settings);
        if !forced && *last_digest == Some(digest) {
            // Nothing a round looks at has changed here since the last one: no need to bother Jira.
            update(status, emit, |s| s.pending = false);
            return Ok(None);
        }
        update(status, emit, |s| {
            s.configured = true;
            s.state = "syncing".into();
        });
        let client = connect(&settings.site, &credentials)?;
        let report = jira_sync::run(&root, &*client, &ledger, &settings, &snapshot)?;
        *last_digest = Some(digest);
        Ok(Some(report))
    })();

    match result {
        Ok(None) => Ok(()),
        Ok(Some(report)) => {
            let vault_changed = report.vault_changed();
            update(status, emit, |s| {
                s.configured = true;
                s.state = "idle".into();
                s.pending = false;
                s.last_error = None;
                s.last_sync_at = Some(ids::now());
                s.summary = Some(report.summary());
                s.report = Some(report);
            });
            if vault_changed {
                emit(JiraEvent::VaultChanged);
            }
            Ok(())
        }
        Err(e) => {
            update(status, emit, |s| {
                s.state = "error".into();
                s.last_error = Some(e.to_string());
            });
            Err(e)
        }
    }
}

// ---- setting up: the logic, apart from the commands ---------------------------------------------------

/// Tries the credentials: the name of the Jira account they belong to.
pub fn test_connection(jira: &dyn Jira) -> JResult<String> {
    let me = jira.myself()?;
    let name = [me.display_name, me.email_address].into_iter().find(|n| !n.is_empty());
    Ok(name.map_or_else(|| "Connected to Jira.".to_string(), |name| format!("Connected to Jira as {name}.")))
}

/// The site and credentials to use: what was typed, or else what is saved.
pub fn resolve(root: &Path, site: Option<&str>, email: Option<&str>, token: Option<&str>) -> Result<(String, Credentials)> {
    let saved = config::read_credentials(root);
    let pick = |typed: Option<&str>, saved: String| typed.map(str::trim).filter(|t| !t.is_empty()).map_or(saved, str::to_string);
    let site = pick(site, config::read_settings(root).site);
    let credentials = Credentials { email: pick(email, saved.email), token: pick(token, saved.token) };
    if site.is_empty() {
        return Err(Error::invalid("Enter the Jira site, such as https://yourteam.atlassian.net"));
    }
    let Some(site) = normalize_site(&site) else {
        return Err(Error::invalid("That isn't a Jira site address. Use the https address you open Jira at, such as https://yourteam.atlassian.net."));
    };
    if credentials.email.is_empty() {
        return Err(Error::invalid("Enter the email address of your Atlassian account"));
    }
    if credentials.token.is_empty() {
        return Err(Error::invalid("Enter the Jira API token"));
    }
    if !valid_email(&credentials.email) {
        return Err(Error::invalid("That doesn't look like an email address."));
    }
    if !valid_token(&credentials.token) {
        return Err(Error::invalid("The API token is made of letters, digits and punctuation, with no spaces. Copy it again from Atlassian."));
    }
    Ok((site, credentials))
}

pub fn save_credentials(root: &Path, site: Option<&str>, email: Option<&str>, token: Option<&str>) -> Result<()> {
    let (site, credentials) = resolve(root, site, email, token)?;
    config::update_settings(root, |s| s.site = site)?;
    config::write_credentials(root, &credentials)
}

/// Removes the email address and token from this machine. The site and the project links stay.
pub fn disconnect(root: &Path) -> Result<()> {
    config::write_credentials(root, &Credentials::default())
}

/// Links a Ledger project to a Jira project (`None` unlinks it). A Jira project serves one Ledger project: two
/// reading the same issues would each import them.
pub fn link_project(root: &Path, jira: &dyn Jira, project: &str, key: Option<&str>) -> JResult<()> {
    let settings = config::read_settings(root);
    let Some(key) = key.filter(|k| !k.is_empty()) else {
        config::update_settings(root, |s| {
            s.projects.remove(project);
        })?;
        config::update_state(root, |s| s.items.retain(|_, t| t.project != project))?;
        return Ok(());
    };
    if let Some((other, _)) = settings.projects.iter().find(|(slug, link)| link.project == key && slug.as_str() != project) {
        return Err(JiraError::message(format!(
            "That Jira project is already linked to the project {other}. A Jira project can serve one project, or its issues would be added to both."
        )));
    }
    let found: Project =
        jira.projects()?.into_iter().find(|p| p.key == key).ok_or_else(|| JiraError::message("That isn't one of the Jira projects this account can see."))?;

    let changed = settings.projects.get(project).is_none_or(|link| link.project != key);
    config::update_settings(root, |s| {
        if changed {
            s.projects.insert(project.to_string(), ProjectLink { project: found.key.clone(), project_name: found.name.clone(), columns: Default::default() });
        }
    })?;
    if changed {
        // Issues of the old Jira project are not in this one: whatever was synced stops, and says so on the next round.
        config::update_state(root, |s| s.items.retain(|_, t| t.project != project))?;
    }
    Ok(())
}

/// Says which Jira status a column uses: "auto" (the status with the same name), "none" (not synced) or a status id.
pub fn set_column_status(root: &Path, project: &str, column: &str, choice: &str) -> Result<()> {
    if !config::read_settings(root).projects.contains_key(project) {
        return Err(Error::invalid("Link the project to a Jira project first"));
    }
    config::update_settings(root, |s| {
        if let Some(link) = s.projects.get_mut(project) {
            match choice {
                "auto" => {
                    link.columns.remove(column);
                }
                "none" => {
                    link.columns.insert(column.to_string(), String::new());
                }
                status => {
                    link.columns.insert(column.to_string(), status.to_string());
                }
            }
        }
    })?;
    Ok(())
}

// ---- the views the screen gets ------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    pub slug: String,
    pub name: String,
    pub jira_project: Option<String>,
    pub jira_project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkedItem {
    pub project: String,
    pub project_name: String,
    pub kind: Kind,
    pub id: String,
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraView {
    pub site: String,
    pub email: String,
    /// The token itself never reaches the screen: only whether it exists and how it ends.
    pub has_token: bool,
    pub token_hint: String,
    pub configured: bool,
    pub projects: Vec<ProjectState>,
    pub unlinked: Vec<UnlinkedItem>,
    pub status: JiraStatus,
    pub settings_file: String,
    pub secrets_file: String,
}

pub fn view(root: &Path, snapshot: &trello_ledger::Snapshot, status: JiraStatus) -> JiraView {
    let settings = config::read_settings(root);
    let credentials = config::read_credentials(root);
    let mut projects = vec![];
    let mut unlinked = vec![];
    for data in &snapshot.projects {
        let link = settings.projects.get(&data.slug).filter(|l| !l.project.is_empty());
        projects.push(ProjectState {
            slug: data.slug.clone(),
            name: data.name.clone(),
            jira_project: link.map(|l| l.project.clone()),
            jira_project_name: link.map(|l| l.project_name.clone()),
        });
        let item = |kind: Kind, id: &str, title: &str, reason: &str| UnlinkedItem {
            project: data.slug.clone(),
            project_name: data.name.clone(),
            kind,
            id: id.to_string(),
            title: title.to_string(),
            reason: reason.to_string(),
        };
        unlinked.extend(data.tasks.iter().filter_map(|t| Some(item(Kind::Task, &t.id, &t.title, t.jira_unlinked.as_deref()?))));
        unlinked.extend(data.stories.iter().filter_map(|s| Some(item(Kind::Story, &s.id, &s.summary, s.jira_unlinked.as_deref()?))));
    }
    projects.sort_by_key(|p| p.name.to_lowercase());
    JiraView {
        site: settings.site,
        email: credentials.email,
        has_token: !credentials.token.is_empty(),
        token_hint: hint(&credentials.token),
        configured: config::is_configured(root),
        projects,
        unlinked,
        status,
        settings_file: config::settings_path(root).display().to_string(),
        secrets_file: crate::settings::secrets_path(root).display().to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnChoice {
    pub id: String,
    pub name: String,
    /// "auto", "none" or the id of a status.
    pub choice: String,
    /// The status the column uses in the end, if it has one.
    pub status_id: Option<String>,
    pub status_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMapping {
    pub statuses: Vec<Status>,
    pub columns: Vec<ColumnChoice>,
    /// Statuses no column uses: their tasks aren't added.
    pub unused_statuses: Vec<String>,
    /// Which of the Story and Task issue types the Jira project lacks.
    pub missing_types: Vec<String>,
}

pub fn mapping_view(columns: &[Column], jira: &dyn Jira, link: &ProjectLink) -> JResult<ProjectMapping> {
    let types = jira.issue_types(&link.project)?;
    let (story, task) = jira_sync::kind_types(&types);
    let statuses = task.map(|t| t.statuses.clone()).unwrap_or_default();
    let missing_types = [("Story", story.is_none()), ("Task", task.is_none())].into_iter().filter(|(_, missing)| *missing).map(|(n, _)| n.to_string()).collect();
    let mapping = Mapping::new(columns, &statuses, &link.columns);
    let status_name = |id: &str| statuses.iter().find(|s| s.id == id).map(|s| s.name.clone());
    let columns_out = columns
        .iter()
        .map(|column| {
            let status_id = mapping.status_for(&column.id).map(str::to_string);
            ColumnChoice {
                id: column.id.clone(),
                name: column.name.clone(),
                choice: match link.columns.get(&column.id).map(String::as_str) {
                    None => "auto".to_string(),
                    Some("") => "none".to_string(),
                    Some(status) => status.to_string(),
                },
                status_name: status_id.as_deref().and_then(status_name),
                status_id,
            }
        })
        .collect();
    let unused_statuses = statuses.iter().filter(|s| mapping.column_for(&s.id).is_none()).map(|s| s.name.clone()).collect();
    Ok(ProjectMapping { statuses, columns: columns_out, unused_statuses, missing_types })
}

// ---- commands -----------------------------------------------------------------------------------------

fn root_of(app: &App) -> PathBuf {
    app.lock().vault.root().to_path_buf()
}

fn current_view(app: &App, syncer: &JiraSyncer) -> Result<JiraView> {
    let (root, snapshot) = {
        let core = app.lock();
        (core.vault.root().to_path_buf(), trello_ledger::snapshot(&core.conn)?)
    };
    Ok(view(&root, &snapshot, syncer.status()))
}

/// Runs something that talks to Jira off the UI thread.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| Error::sync(format!("That stopped unexpectedly: {e}")))?
}

fn client(root: &Path) -> Result<HttpJira<JiraWire>> {
    let settings = config::read_settings(root);
    let credentials = config::read_credentials(root);
    if settings.site.is_empty() || !credentials.complete() {
        return Err(Error::invalid("Save the Jira site, email address and API token first"));
    }
    Ok(HttpJira::new(&settings.site, &credentials.email, &credentials.token)?)
}

#[tauri::command]
pub fn jira_get(app: State<App>, syncer: State<JiraSyncer>) -> Result<JiraView> {
    current_view(&app, &syncer)
}

#[tauri::command]
pub fn jira_status(syncer: State<JiraSyncer>) -> JiraStatus {
    syncer.status()
}

/// Tries the site, email address and token on screen (a blank one means the saved one) without saving anything.
#[tauri::command]
pub async fn jira_test(app: State<'_, App>, site: Option<String>, email: Option<String>, token: Option<String>) -> Result<String> {
    let root = root_of(&app);
    blocking(move || {
        let (site, credentials) = resolve(&root, site.as_deref(), email.as_deref(), token.as_deref())?;
        let client = HttpJira::new(&site, &credentials.email, &credentials.token)?;
        Ok(test_connection(&client)?)
    })
    .await
}

/// Saves the site, email address and token (a blank one keeps the saved one).
#[tauri::command]
pub fn jira_save_credentials(app: State<App>, syncer: State<JiraSyncer>, site: Option<String>, email: Option<String>, token: Option<String>) -> Result<JiraView> {
    let root = root_of(&app);
    save_credentials(&root, site.as_deref(), email.as_deref(), token.as_deref())?;
    syncer.reconfigure(&root);
    current_view(&app, &syncer)
}

/// Removes the email address and token from this machine.
#[tauri::command]
pub fn jira_disconnect(app: State<App>, syncer: State<JiraSyncer>) -> Result<JiraView> {
    let root = root_of(&app);
    disconnect(&root)?;
    syncer.reconfigure(&root);
    current_view(&app, &syncer)
}

#[tauri::command]
pub async fn jira_projects(app: State<'_, App>) -> Result<Vec<Project>> {
    let root = root_of(&app);
    blocking(move || Ok(client(&root)?.projects()?)).await
}

/// Links a project to a Jira project, or (no Jira project) stops syncing it.
#[tauri::command]
pub async fn jira_link_project(app: State<'_, App>, syncer: State<'_, JiraSyncer>, project: String, jira_project: Option<String>) -> Result<JiraView> {
    let root = root_of(&app);
    app.read(|c, _| projects::require(c, &project))?;
    {
        let root = root.clone();
        blocking(move || Ok(link_project(&root, &client(&root)?, &project, jira_project.as_deref())?)).await?;
    }
    syncer.reconfigure(&root);
    current_view(&app, &syncer)
}

/// The project's columns, the Jira project's statuses, and which goes with which.
#[tauri::command]
pub async fn jira_project_mapping(app: State<'_, App>, project: String) -> Result<ProjectMapping> {
    let root = root_of(&app);
    let columns = app.read(|c, _| crate::board::columns(c, &project))?;
    let link = config::read_settings(&root).projects.get(&project).cloned().filter(|l| !l.project.is_empty());
    let Some(link) = link else { return Err(Error::invalid("This project isn't linked to a Jira project")) };
    blocking(move || Ok(mapping_view(&columns, &client(&root)?, &link)?)).await
}

/// "auto", "none" or a status id for one column.
#[tauri::command]
pub fn jira_set_column_status(app: State<App>, syncer: State<JiraSyncer>, project: String, column: String, choice: String) -> Result<()> {
    if !app.read(|c, _| crate::board::column_exists(c, &project, &column))? {
        return Err(Error::not_found("Column not found"));
    }
    let root = root_of(&app);
    set_column_status(&root, &project, &column, &choice)?;
    syncer.reconfigure(&root);
    Ok(())
}

#[tauri::command]
pub fn jira_sync_now(app: State<App>, syncer: State<JiraSyncer>) -> Result<()> {
    if !config::is_configured(&root_of(&app)) {
        return Err(Error::invalid("Jira sync isn't set up yet: it needs the site, email address and API token, and a project linked to a Jira project"));
    }
    syncer.now();
    Ok(())
}

/// Lets an item that stopped syncing get an issue of its own again: the old issue is left alone on Jira, and the
/// next sync makes a new one.
#[tauri::command]
pub fn jira_relink_item(app: State<App>, syncer: State<JiraSyncer>, project: String, kind: Kind, id: String) -> Result<JiraView> {
    app.mutate(|conn, root| {
        if let Some(issue) = jira_ledger::clear_link(conn, &project, kind, &id)? {
            config::update_state(root, |s| {
                s.items.remove(&issue.to_uppercase());
            })?;
        }
        Ok(())
    })?;
    syncer.now();
    current_view(&app, &syncer)
}
