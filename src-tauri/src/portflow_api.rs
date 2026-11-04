//! Talking to Portflow (Drieam's portfolio, `portfolio.drieam.app`). Its web app is a client of a JSON REST API at
//! `/api/v1`; that API is not public or documented, so everything here is read leniently: every field has a default,
//! unknown fields are ignored, and a list that isn't a list is an error a person can read, not a crash.
//!
//! The API sits behind the [`Portflow`] trait so that the sync can be tested against a fake one; [`HttpPortflow`] is
//! the real one. Every request goes through one [`Gate`] first, which holds the access the person chose:
//!
//! - **Off**: nothing is sent at all.
//! - **Read only** (the default once on): only `GET`, plus the one read Portflow makes with a `POST` (resolving
//!   mentions to names).
//! - **Read and write**: also the kinds of write the person switched on, one by one.
//!
//! The session token goes in an `Authorization: Bearer …` header and nowhere else: never in a URL, a body, an error
//! message or a log (it is kept on disk only in the vault's `.ledger/.env`, see `portflow_auth.rs`). Nothing here
//! downloads Portflow's files: what was uploaded in Portflow stays there, and is opened there. `LEDGER_PORTFLOW_API` points the client at another API base, for testing.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Error;
use crate::portflow_config::Access;

pub const DEFAULT_API: &str = "https://portfolio.drieam.app/api/v1";
/// Where the student's pages are in Portflow's web app (its router's base).
pub const WEB_APP: &str = "https://portfolio.drieam.app/portfolio";

/// Never two requests closer together than this: Portflow is a school's system, not a bulk API.
pub const MIN_GAP: Duration = Duration::from_millis(150);
const MAX_ATTEMPTS: usize = 4;
/// Lists are asked for in pages of this many; more pages than [`MAX_PAGES`] means something is off.
const PER_PAGE: usize = 50;
const MAX_PAGES: usize = 40;

// ---- what Portflow holds ------------------------------------------------------------------------------

/// A person: the student, a teacher, a peer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Person {
    pub id: i64,
    pub name: String,
    /// "student", "teacher", …
    pub current_role: Option<String>,
    pub pronouns: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Import {
    /// The template import: a section and the goals that came with it share it, which is how a semester's learning
    /// outcomes are told apart from another semester's.
    pub id: Option<i64>,
    pub imported_at: Option<String>,
    pub template_name: Option<String>,
}

/// A section of the portfolio: in practice a semester ("Open Learning FT Semester 3 – Fall 26").
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Section {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub last_import: Option<Import>,
}

/// A collection: one piece of work in one semester.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub subtitle: Option<String>,
    /// HTML, with `@` mentions as `<span data-type="mention" …>`.
    pub description: Option<String>,
    pub color: Option<String>,
    pub position: i64,
    pub section_id: Option<i64>,
    pub is_private: bool,
    pub evidence_count: i64,
    pub last_import: Option<Import>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Named {
    pub id: i64,
    pub name: String,
}

/// A learning outcome ("Analysis", "Professional standard").
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Goal {
    pub id: i64,
    pub name: String,
    pub nickname: Option<String>,
    pub position: i64,
    /// "not_yet_started", …
    pub status: Option<String>,
    /// HTML.
    pub explanation: Option<String>,
    pub description: Option<String>,
    pub goal_set: Option<Named>,
    pub evidence_count: i64,
    pub last_import: Option<Import>,
}

/// A part of a learning outcome at a level ("Analysis-U1").
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct SubGoal {
    pub id: i64,
    pub name: String,
    pub nickname: Option<String>,
    /// HTML.
    pub description: Option<String>,
    pub goal_id: i64,
    pub position: i64,
    /// How many evidence items in the whole portfolio count for it.
    pub evidence_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct GoalSet {
    pub id: i64,
    pub name: String,
    pub goal_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct CollectionGoal {
    pub id: i64,
    pub position: i64,
    pub goal: Goal,
}

/// Who the portfolio (or a collection or an evidence item) is shared with.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Share {
    pub id: i64,
    /// "portfolio", or a collection or evidence item shared on its own.
    pub share_type: Option<String>,
    pub expiration_date: Option<String>,
    pub feedback_restricted: bool,
    pub invitee: Person,
}

/// An evidence item, as a collection lists it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Evidence {
    pub id: i64,
    pub name: String,
    /// "file", "free_text", "url", "video", "lms_submission", "badge".
    pub latest_version_type: Option<String>,
    pub versions_count: i64,
    pub created_at: Option<String>,
    pub latest_version_created_at: Option<String>,
    /// The outcomes it counts for, when the list includes them (otherwise asked for separately).
    pub goals: Option<Vec<Named>>,
    pub sub_goals: Option<Vec<Named>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct CollectionEvidence {
    pub id: i64,
    pub position: i64,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct FileInfo {
    pub file_name: String,
    #[serde(rename = "type")]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct VersionFile {
    pub id: i64,
    pub display_name: Option<String>,
    pub file: FileInfo,
}

impl VersionFile {
    /// The name the file had when it was uploaded.
    pub fn name(&self) -> String {
        let shown = self.display_name.as_deref().map(str::trim).filter(|n| !n.is_empty());
        shown.map(str::to_string).unwrap_or_else(|| self.file.file_name.clone())
    }
}

/// A version of an evidence item. Listed without its files; asked for one by one, with them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Version {
    pub id: i64,
    pub evidence_id: i64,
    pub version_number: i64,
    pub version_type: Option<String>,
    /// HTML: the changelog the student wrote.
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub co_created: bool,
    pub files: Vec<VersionFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct VersionRef {
    pub version_number: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct FeedbackRequest {
    pub invitees: Vec<Person>,
}

/// A comment on an evidence item: a root (often a feedback request) or a reply to one.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Comment {
    pub id: i64,
    pub parent_id: Option<i64>,
    /// HTML.
    pub text: Option<String>,
    /// "feedback_request", "comment", "criterion_evaluation", …
    pub comment_type: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub created_by_role: Option<String>,
    pub created_by: Person,
    pub child_count: i64,
    pub hidden: bool,
    pub evidence_version: Option<VersionRef>,
    pub feedback_request: Option<FeedbackRequest>,
}

// ---- errors -------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum PortflowError {
    /// The feature is switched off: nothing may be sent.
    Off,
    /// No session, or Portflow no longer accepts it: sign in again.
    SignedOut,
    /// The access setting doesn't allow this request.
    NotAllowed(String),
    /// Portflow answered something Ledger doesn't understand: its API probably changed.
    Changed(String),
    /// Something a person can read and act on.
    Message(String),
    /// The vault side failed.
    Ledger(Error),
}

pub type PResult<T> = std::result::Result<T, PortflowError>;

impl PortflowError {
    pub fn message(text: impl Into<String>) -> Self {
        PortflowError::Message(text.into())
    }
}

impl fmt::Display for PortflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortflowError::Off => f.write_str("Portflow is switched off. Switch it on under Settings → Sync & external → Portflow."),
            PortflowError::SignedOut => f.write_str("Sign in to Portflow again: the session ended."),
            PortflowError::NotAllowed(text) | PortflowError::Message(text) => f.write_str(text),
            PortflowError::Changed(what) => write!(
                f,
                "Portflow answered in a way Ledger doesn't recognise ({what}). Its API may have changed; syncing paused and nothing in the vault was changed."
            ),
            PortflowError::Ledger(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PortflowError {}

impl From<Error> for PortflowError {
    fn from(e: Error) -> Self {
        PortflowError::Ledger(e)
    }
}

impl From<PortflowError> for Error {
    fn from(e: PortflowError) -> Self {
        match e {
            PortflowError::Ledger(e) => e,
            other => Error::sync(other.to_string()),
        }
    }
}

// ---- the gate -----------------------------------------------------------------------------------------

/// A request without the session: that is added by the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: &'static str,
    /// Below the API base, starting with `/`.
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Option<Value>,
}

impl Request {
    pub fn get(path: impl Into<String>, query: &[(&str, String)]) -> Request {
        Request { method: "GET", path: path.into(), query: query.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(), body: None }
    }
}

/// Which kind of write a request is, so the access setting can allow or refuse it. `None` for a read.
pub fn write_kind(request: &Request) -> Option<&'static str> {
    if request.method == "GET" || request.method == "HEAD" {
        return None;
    }
    let path = request.path.as_str();
    // Portflow resolves mention ids to names with a POST; it reads, so it is a read.
    if request.method == "POST" && path.ends_with("/mentions") {
        return None;
    }
    Some(if path.contains("/direct-uploads") || (path.contains("/evidence/") && path.ends_with("/versions")) {
        "upload-versions"
    } else if path.ends_with("/evidence") {
        "create-evidence"
    } else if path.contains("/link-to-goals-and-sub-goals") || path.contains("/link-to-sub-goals") || path.contains("/collection-goals") {
        "link-outcomes"
    } else if path.contains("/comment-validations") {
        "confirm-feedback"
    } else if path.contains("/feedback-requests") {
        "ask-feedback"
    } else if path.contains("/comments") {
        "reply-feedback"
    } else if path.contains("/collections/") {
        "edit-descriptions"
    } else {
        // A write this version doesn't know: never allowed.
        "unknown"
    })
}

/// What may be sent: decided once per client, from the settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gate {
    pub enabled: bool,
    pub write: bool,
    pub writes: BTreeSet<String>,
}

impl Gate {
    pub fn new(enabled: bool, access: &Access) -> Gate {
        Gate { enabled, write: access.write, writes: access.writes.clone() }
    }

    /// Lets a request through, or says why not.
    pub fn check(&self, request: &Request) -> PResult<()> {
        if !self.enabled {
            return Err(PortflowError::Off);
        }
        match write_kind(request) {
            None => Ok(()),
            Some(kind) if self.write && self.writes.contains(kind) => Ok(()),
            Some(_) if !self.write => Err(PortflowError::NotAllowed(
                "Portflow is read only: Ledger doesn't change anything there. Switch on read and write under Settings → Portflow to allow it.".into(),
            )),
            Some(_) => Err(PortflowError::NotAllowed("That kind of change in Portflow isn't switched on under Settings → Portflow.".into())),
        }
    }
}

// ---- the wire -----------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

/// Sends requests with the session. `Err` is a failure to get an answer at all, in words fit for a person.
pub trait Wire: Send + Sync {
    fn send(&self, request: &Request) -> std::result::Result<Reply, String>;
}

/// The session as the wire needs it. Deliberately not `Debug`/`Serialize`.
#[derive(Clone)]
pub struct Credentials {
    pub token: String,
    pub csrf: Option<String>,
}

pub struct ReqwestWire {
    client: Client,
    api: String,
    auth: HeaderValue,
    csrf: Option<HeaderValue>,
}

pub fn api_base() -> String {
    std::env::var("LEDGER_PORTFLOW_API")
        .ok()
        .filter(|api| !api.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_API.to_string())
        .trim_end_matches('/')
        .to_string()
}

impl ReqwestWire {
    pub fn new(credentials: &Credentials) -> PResult<ReqwestWire> {
        let mut auth = HeaderValue::from_str(&format!("Bearer {}", credentials.token))
            .map_err(|_| PortflowError::message("The Portflow session can't be sent. Sign in again."))?;
        auth.set_sensitive(true);
        let csrf = match credentials.csrf.as_deref().filter(|c| !c.is_empty()) {
            Some(c) => Some(HeaderValue::from_str(c).map_err(|_| PortflowError::message("The Portflow session can't be sent. Sign in again."))?),
            None => None,
        };
        // No redirects are followed: the session must not be carried to another host.
        let client = Client::builder()
            .user_agent(concat!("ledger/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| PortflowError::message(format!("Could not set up the connection: {e}")))?;
        Ok(ReqwestWire { client, api: api_base(), auth, csrf })
    }
}

impl Wire for ReqwestWire {
    fn send(&self, request: &Request) -> std::result::Result<Reply, String> {
        let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|e| e.to_string())?;
        let mut builder = self
            .client
            .request(method, format!("{}{}", self.api, request.path))
            .header(AUTHORIZATION, self.auth.clone())
            .header("Accept", "application/json")
            .query(&request.query);
        if let Some(csrf) = &self.csrf {
            builder = builder.header("X-CSRF-Token", csrf.clone());
        }
        if let Some(body) = &request.body {
            builder = builder.json(body);
        }
        let response = builder.send().map_err(|e| format!("Could not reach Portflow: {}", e.without_url()))?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|e| format!("Could not read Portflow's answer: {}", e.without_url()))?;
        Ok(Reply { status, body })
    }

}

// ---- the API ------------------------------------------------------------------------------------------

/// What Ledger reads from Portflow. Writes are added kind by kind, each behind the gate.
pub trait Portflow: Send + Sync {
    fn current_user(&self) -> PResult<Person>;
    fn sections(&self, portfolio: i64) -> PResult<Vec<Section>>;
    fn collections(&self, portfolio: i64) -> PResult<Vec<Collection>>;
    fn collection_goals(&self, portfolio: i64, collection: i64) -> PResult<Vec<CollectionGoal>>;
    fn collection_evidence(&self, portfolio: i64, collection: i64) -> PResult<Vec<CollectionEvidence>>;
    fn goal_sets(&self, portfolio: i64) -> PResult<Vec<GoalSet>>;
    fn goals(&self, portfolio: i64) -> PResult<Vec<Goal>>;
    fn sub_goals(&self, portfolio: i64, goal: i64) -> PResult<Vec<SubGoal>>;
    fn shares(&self, portfolio: i64) -> PResult<Vec<Share>>;
    fn evidence_sub_goals(&self, portfolio: i64, evidence: i64) -> PResult<Vec<SubGoal>>;
    fn evidence_goals(&self, portfolio: i64, evidence: i64) -> PResult<Vec<Goal>>;
    /// Newest first, without files.
    fn versions(&self, portfolio: i64, evidence: i64) -> PResult<Vec<Version>>;
    /// One version, with its files.
    fn version(&self, portfolio: i64, version: i64) -> PResult<Version>;
    fn evidence_comments(&self, portfolio: i64, evidence: i64) -> PResult<Vec<Comment>>;
    fn comment_children(&self, portfolio: i64, comment: i64) -> PResult<Vec<Comment>>;
}

pub type Sleeper = Arc<dyn Fn(Duration) + Send + Sync>;

pub struct HttpPortflow<W: Wire> {
    wire: W,
    gate: Gate,
    sleep: Sleeper,
    last: Mutex<Option<Instant>>,
}

impl HttpPortflow<ReqwestWire> {
    pub fn new(credentials: &Credentials, gate: Gate) -> PResult<Self> {
        if !gate.enabled {
            return Err(PortflowError::Off);
        }
        Ok(HttpPortflow::with_wire(ReqwestWire::new(credentials)?, gate, Arc::new(std::thread::sleep)))
    }
}

impl<W: Wire> HttpPortflow<W> {
    pub fn with_wire(wire: W, gate: Gate, sleep: Sleeper) -> Self {
        HttpPortflow { wire, gate, sleep, last: Mutex::new(None) }
    }

    fn pace(&self) {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = *last {
            let elapsed = at.elapsed();
            if elapsed < MIN_GAP {
                (self.sleep)(MIN_GAP - elapsed);
            }
        }
        *last = Some(Instant::now());
    }

    /// Sends a request through the gate, retrying what is worth retrying. Returns the body of a 2xx answer.
    pub fn send(&self, request: &Request) -> PResult<Value> {
        self.gate.check(request)?;
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.pace();
            let last = attempt >= MAX_ATTEMPTS;
            match self.wire.send(request) {
                Err(e) => {
                    if last {
                        return Err(PortflowError::Message(e));
                    }
                    (self.sleep)(backoff(attempt));
                }
                Ok(reply) if reply.status == 429 || (500..600).contains(&reply.status) => {
                    if last {
                        return Err(PortflowError::message(format!("Portflow is having trouble right now (it answered {}). Syncing will try again.", reply.status)));
                    }
                    (self.sleep)(backoff(attempt));
                }
                Ok(reply) => return answer(reply),
            }
        }
    }

    fn get<T: DeserializeOwned>(&self, path: String, what: &str) -> PResult<T> {
        parse(self.send(&Request::get(path, &[]))?, what)
    }

    /// Every page of a list.
    fn get_all<T: DeserializeOwned>(&self, path: String, what: &str) -> PResult<Vec<T>> {
        let mut out = vec![];
        for page in 1..=MAX_PAGES {
            let body = self.send(&Request::get(path.clone(), &[("page", page.to_string()), ("per_page", PER_PAGE.to_string())]))?;
            let Value::Array(items) = body else { return Err(PortflowError::Changed(format!("{what} is not a list"))) };
            let count = items.len();
            for item in items {
                out.push(parse(item, what)?);
            }
            if count < PER_PAGE {
                return Ok(out);
            }
        }
        Err(PortflowError::message(format!("Portflow kept sending more {what}; stopped after {MAX_PAGES} pages")))
    }
}

fn backoff(attempt: usize) -> Duration {
    Duration::from_secs(1u64 << (attempt - 1).min(4))
}

fn answer(reply: Reply) -> PResult<Value> {
    // An empty answer (204 No Content) is no content, not a text.
    let body: Value = if reply.body.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&reply.body).unwrap_or_else(|_| Value::String(reply.body.trim().chars().take(200).collect()))
    };
    match reply.status {
        200..=299 => Ok(body),
        401 => Err(PortflowError::SignedOut),
        403 => Err(PortflowError::message("Portflow says this account isn't allowed to see that.")),
        404 => Err(PortflowError::message("Portflow can't find that (it may have been deleted or moved).")),
        status => {
            let detail = match &body {
                Value::String(text) => text.clone(),
                other => other["error"].as_str().or(other["message"].as_str()).unwrap_or_default().to_string(),
            };
            Err(PortflowError::message(format!("Portflow answered {status}{}", if detail.is_empty() { String::new() } else { format!(": {detail}") })))
        }
    }
}

fn parse<T: DeserializeOwned>(value: Value, what: &str) -> PResult<T> {
    serde_json::from_value(value).map_err(|_| PortflowError::Changed(what.to_string()))
}

impl<W: Wire> Portflow for HttpPortflow<W> {
    fn current_user(&self) -> PResult<Person> {
        self.get("/users/current".into(), "the signed-in user")
    }

    fn sections(&self, p: i64) -> PResult<Vec<Section>> {
        self.get_all(format!("/portfolios/{p}/sections"), "sections")
    }

    fn collections(&self, p: i64) -> PResult<Vec<Collection>> {
        self.get_all(format!("/portfolios/{p}/collections"), "collections")
    }

    fn collection_goals(&self, p: i64, c: i64) -> PResult<Vec<CollectionGoal>> {
        self.get_all(format!("/portfolios/{p}/collections/{c}/collection-goals"), "a collection's learning outcomes")
    }

    fn collection_evidence(&self, p: i64, c: i64) -> PResult<Vec<CollectionEvidence>> {
        self.get_all(format!("/portfolios/{p}/collections/{c}/evidence"), "a collection's evidence")
    }

    fn goal_sets(&self, p: i64) -> PResult<Vec<GoalSet>> {
        self.get_all(format!("/portfolios/{p}/goal-sets"), "goal sets")
    }

    fn goals(&self, p: i64) -> PResult<Vec<Goal>> {
        self.get_all(format!("/portfolios/{p}/goals"), "learning outcomes")
    }

    fn sub_goals(&self, p: i64, g: i64) -> PResult<Vec<SubGoal>> {
        self.get_all(format!("/portfolios/{p}/goals/{g}/sub-goals"), "sub-goals")
    }

    fn shares(&self, p: i64) -> PResult<Vec<Share>> {
        self.get_all(format!("/portfolios/{p}/shares"), "shares")
    }

    fn evidence_sub_goals(&self, p: i64, e: i64) -> PResult<Vec<SubGoal>> {
        self.get(format!("/portfolios/{p}/evidence/{e}/sub-goals"), "an evidence item's sub-goals")
    }

    fn evidence_goals(&self, p: i64, e: i64) -> PResult<Vec<Goal>> {
        self.get(format!("/portfolios/{p}/evidence/{e}/goals"), "an evidence item's learning outcomes")
    }

    fn versions(&self, p: i64, e: i64) -> PResult<Vec<Version>> {
        self.get_all(format!("/portfolios/{p}/evidence/{e}/versions"), "versions")
    }

    fn version(&self, p: i64, v: i64) -> PResult<Version> {
        self.get(format!("/portfolios/{p}/evidence-versions/{v}"), "a version")
    }

    fn evidence_comments(&self, p: i64, e: i64) -> PResult<Vec<Comment>> {
        self.get_all(format!("/portfolios/{p}/evidence/{e}/comments"), "comments")
    }

    fn comment_children(&self, p: i64, c: i64) -> PResult<Vec<Comment>> {
        self.get(format!("/portfolios/{p}/comments/{c}/children"), "replies")
    }
}
