//! Talking to Jira Cloud (REST API v3). The API sits behind the [`Jira`] trait so that the sync can be tested
//! against a fake one; [`HttpJira`] is the real one.
//!
//! Credentials: the email address and API token go in an `Authorization: Basic …` header and nowhere else: never
//! in a URL, a query, a body, an error message or a log. They are only ever handled by [`JiraWire`]; everything
//! above it works with requests that carry no credentials at all (the same [`Request`] the Trello client uses).
//!
//! Rate limits: requests are paced, a 429 (or a 503 with a `Retry-After`) is waited out for as long as Jira says
//! and then retried, and a server error or a dropped connection is retried with a growing pause. When that doesn't
//! help, the caller gets [`JiraError::RateLimited`] and backs off for longer. `LEDGER_JIRA_API` points the client
//! at another API base (in place of `https://<site>/rest/api/3`), for testing.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::Error;
use crate::trello_api::{Reply, Request, Sleeper, Wire};

/// Never send two requests closer together than this.
pub const MIN_GAP: Duration = Duration::from_millis(120);
const MAX_ATTEMPTS: usize = 5;
/// The longest single pause honoured for a 429; a longer `Retry-After` is passed on to the caller instead.
const MAX_WAIT: Duration = Duration::from_secs(60);
const PAGE: usize = 100;
/// Enough pages for any project Ledger is meant for; a runaway pagination stops here.
const MAX_PAGES: usize = 200;

pub const ISSUE_FIELDS: [&str; 8] = ["summary", "description", "status", "issuetype", "project", "labels", "created", "updated"];

// ---- what Jira holds ----------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Myself {
    pub account_id: String,
    pub display_name: String,
    pub email_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    pub id: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Status {
    pub id: String,
    pub name: String,
}

/// An issue type of a project, with the statuses its workflow has.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IssueType {
    pub id: String,
    pub name: String,
    pub subtask: bool,
    pub statuses: Vec<Status>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Issue {
    pub key: String,
    /// The key of the project it is in now.
    pub project: String,
    /// The id of its issue type, and the type's name.
    pub issue_type: String,
    pub type_name: String,
    pub summary: String,
    /// The description in ADF; `None` when there is none.
    pub description: Option<Value>,
    pub status: String,
    pub status_name: String,
    pub labels: Vec<String>,
    /// When it was made and last changed, as Ledger writes times (UTC, milliseconds).
    pub created: String,
    pub updated: String,
}

pub struct NewIssue {
    pub project: String,
    pub issue_type: String,
    pub summary: String,
    pub description: Option<Value>,
    pub labels: Vec<String>,
}

/// Only what is present changes.
#[derive(Debug, Default, Clone)]
pub struct IssueUpdate {
    pub summary: Option<String>,
    /// `Some(None)` clears the description.
    pub description: Option<Option<Value>>,
    pub add_label: Option<String>,
}

impl IssueUpdate {
    pub fn is_empty(&self) -> bool {
        self.summary.is_none() && self.description.is_none() && self.add_label.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Transition {
    pub id: String,
    /// The status it leads to.
    pub to: String,
}

// ---- errors -------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum JiraError {
    /// Jira kept answering 429 (or asked for a longer wait than is worth blocking for).
    RateLimited { retry_after: Duration },
    /// The email address or API token was rejected: nothing else will work either, so a sync stops.
    Unauthorized,
    /// Something a person can read and act on.
    Message(String),
    /// The vault side of a sync failed.
    Ledger(Error),
}

pub type JResult<T> = std::result::Result<T, JiraError>;

impl JiraError {
    pub fn message(text: impl Into<String>) -> Self {
        JiraError::Message(text.into())
    }
}

impl fmt::Display for JiraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JiraError::RateLimited { .. } => f.write_str("Jira is rate limiting this account. Syncing will resume on its own."),
            JiraError::Unauthorized => f.write_str(
                "Jira rejected the email address or API token. Check both, and that the token hasn't expired or been revoked.",
            ),
            JiraError::Message(text) => f.write_str(text),
            JiraError::Ledger(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for JiraError {}

impl From<Error> for JiraError {
    fn from(e: Error) -> Self {
        JiraError::Ledger(e)
    }
}

impl From<JiraError> for Error {
    fn from(e: JiraError) -> Self {
        match e {
            JiraError::Ledger(e) => e,
            other => Error::sync(other.to_string()),
        }
    }
}

// ---- the API ------------------------------------------------------------------------------------------

pub trait Jira: Sync {
    /// The account the credentials belong to.
    fn myself(&self) -> JResult<Myself>;
    /// The projects the account can see.
    fn projects(&self) -> JResult<Vec<Project>>;
    /// The project's issue types, each with the statuses of its workflow.
    fn issue_types(&self, project: &str) -> JResult<Vec<IssueType>>;
    /// Every issue in the project of one of the given types.
    fn issues(&self, project: &str, types: &[String]) -> JResult<Vec<Issue>>;
    /// One issue by key, wherever it is now; `None` once it no longer exists (or can't be seen).
    fn issue(&self, key: &str) -> JResult<Option<Issue>>;
    /// Makes an issue: its key.
    fn create_issue(&self, new: &NewIssue) -> JResult<String>;
    fn update_issue(&self, key: &str, update: &IssueUpdate) -> JResult<()>;
    /// The transitions the issue's workflow offers from where it is now.
    fn transitions(&self, key: &str) -> JResult<Vec<Transition>>;
    fn transition(&self, key: &str, transition: &str) -> JResult<()>;
}

// ---- the site and the credentials ---------------------------------------------------------------------

/// The site as `https://<host>`: from a full address (a path after it is dropped) or a bare name, which is taken to
/// be `<name>.atlassian.net`.
pub fn normalize_site(input: &str) -> Option<String> {
    let input = input.trim().trim_end_matches('/');
    if input.is_empty() {
        return None;
    }
    let full = if input.contains("://") {
        input.to_string()
    } else if input.contains('.') {
        format!("https://{input}")
    } else {
        format!("https://{input}.atlassian.net")
    };
    let url = Url::parse(&full).ok()?;
    let host = url.host_str()?;
    (url.scheme() == "https" && url.username().is_empty() && url.password().is_none() && url.port().is_none()).then(|| format!("https://{host}"))
}

pub fn valid_email(text: &str) -> bool {
    text.contains('@') && text.chars().all(|c| c.is_ascii_graphic())
}

pub fn valid_token(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_graphic())
}

/// The header value for Jira's basic authorization.
pub fn auth_header(email: &str, token: &str) -> String {
    format!("Basic {}", STANDARD.encode(format!("{email}:{token}")))
}

/// A key or id as one segment of a path: anything but letters, digits, `-` and `_` is escaped, so text from a
/// file can't reach another endpoint.
fn segment(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

// ---- the wire -----------------------------------------------------------------------------------------

pub struct JiraWire {
    client: Client,
    api: String,
    auth: HeaderValue,
}

impl JiraWire {
    pub fn new(site: &str, email: &str, token: &str) -> JResult<JiraWire> {
        let api = match std::env::var("LEDGER_JIRA_API").ok().filter(|api| !api.trim().is_empty()) {
            Some(api) => api,
            None => format!("{}/rest/api/3", normalize_site(site).ok_or_else(|| JiraError::message("That isn't a Jira site address, such as https://yourteam.atlassian.net."))?),
        };
        if !valid_email(email) || !valid_token(token) {
            return Err(JiraError::message("The email address or API token contains characters that can't be sent. Copy them again."));
        }
        let mut auth = HeaderValue::from_str(&auth_header(email, token))
            .map_err(|_| JiraError::message("The email address or API token contains characters that can't be sent. Copy them again."))?;
        auth.set_sensitive(true);
        let client = Client::builder()
            .user_agent(concat!("ledger/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| JiraError::message(format!("Could not set up the connection: {e}")))?;
        Ok(JiraWire { client, api: api.trim_end_matches('/').to_string(), auth })
    }
}

impl Wire for JiraWire {
    fn send(&self, request: &Request) -> std::result::Result<Reply, String> {
        let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|e| e.to_string())?;
        let mut builder = self
            .client
            .request(method, format!("{}{}", self.api, request.path))
            .header(AUTHORIZATION, self.auth.clone())
            .header("Accept", "application/json")
            .query(&request.query);
        if let Some(body) = &request.body {
            builder = builder.json(body);
        }
        let response = builder.send().map_err(|e| format!("Could not reach Jira: {}", e.without_url()))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let body = response.text().map_err(|e| format!("Could not read Jira's answer: {}", e.without_url()))?;
        Ok(Reply { status, retry_after, body })
    }
}

// ---- the client ---------------------------------------------------------------------------------------

pub struct HttpJira<W: Wire> {
    wire: W,
    sleep: Sleeper,
    last: Mutex<Option<Instant>>,
}

impl HttpJira<JiraWire> {
    pub fn new(site: &str, email: &str, token: &str) -> JResult<Self> {
        Ok(HttpJira::with_wire(JiraWire::new(site, email, token)?, Arc::new(std::thread::sleep)))
    }
}

impl<W: Wire> HttpJira<W> {
    /// `sleep` is how pauses are made; tests pass one that only records them.
    pub fn with_wire(wire: W, sleep: Sleeper) -> Self {
        HttpJira { wire, sleep, last: Mutex::new(None) }
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

    /// Sends the request, waiting out rate limits and retrying server errors. Returns the status and the JSON body
    /// (a body that isn't JSON comes back as a string) of the first answer that is neither.
    fn send(&self, request: &Request) -> JResult<(u16, Value)> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.pace();
            let last = attempt >= MAX_ATTEMPTS;
            match self.wire.send(request) {
                Err(e) => {
                    if last {
                        return Err(JiraError::Message(e));
                    }
                    (self.sleep)(backoff(attempt));
                }
                Ok(reply) if reply.status == 429 || (reply.status == 503 && reply.retry_after.is_some()) => {
                    let wait = reply.retry_after.unwrap_or_else(|| backoff(attempt));
                    if last || wait > MAX_WAIT {
                        return Err(JiraError::RateLimited { retry_after: wait });
                    }
                    (self.sleep)(wait);
                }
                Ok(reply) if (500..600).contains(&reply.status) => {
                    if last {
                        return Err(JiraError::message(format!("Jira is having trouble right now (it answered {}). Syncing will try again.", reply.status)));
                    }
                    (self.sleep)(backoff(attempt));
                }
                Ok(reply) => {
                    let body = serde_json::from_str(&reply.body).unwrap_or_else(|_| Value::String(reply.body.trim().to_string()));
                    return Ok((reply.status, body));
                }
            }
        }
    }

    fn ok(&self, request: Request) -> JResult<Value> {
        let (status, body) = self.send(&request)?;
        if (200..300).contains(&status) { Ok(body) } else { Err(failure(status, &body)) }
    }

    fn get(&self, path: impl Into<String>, query: &[(&str, &str)]) -> JResult<Value> {
        self.ok(Request {
            method: "GET",
            path: path.into(),
            query: query.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            body: None,
        })
    }

    fn write(&self, method: &'static str, path: impl Into<String>, body: Value) -> JResult<Value> {
        self.ok(Request { method, path: path.into(), query: vec![], body: Some(body) })
    }
}

fn backoff(attempt: usize) -> Duration {
    Duration::from_secs(1u64 << (attempt - 1).min(5))
}

/// What a refusal means, as words for the settings screen. Jira says why in `errorMessages` and `errors`.
fn failure(status: u16, body: &Value) -> JiraError {
    let mut details: Vec<String> = body["errorMessages"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(str::to_string)).collect();
    if let Some(errors) = body["errors"].as_object() {
        details.extend(errors.iter().map(|(field, message)| format!("{field}: {}", message.as_str().unwrap_or_default())));
    }
    if let Value::String(text) = body {
        details.push(text.clone());
    }
    let detail: String = details.join(" ").chars().take(300).collect();
    match status {
        401 => JiraError::Unauthorized,
        403 => JiraError::message("Jira says this account isn't allowed to do that. Check its permissions in the Jira project."),
        404 => JiraError::message("Jira can't find that project or issue (it may have been deleted, or this account can't see it)."),
        400 => JiraError::message(if detail.is_empty() { "Jira refused the request.".to_string() } else { format!("Jira refused the request: {detail}") }),
        _ => JiraError::message(format!("Jira answered {status}{}", if detail.is_empty() { String::new() } else { format!(": {detail}") })),
    }
}

fn parse<T: for<'de> Deserialize<'de>>(value: Value) -> JResult<T> {
    serde_json::from_value(value).map_err(|_| unexpected())
}

fn unexpected() -> JiraError {
    JiraError::message("Jira's answer was not what was expected.")
}

/// A time as Jira writes it (`2026-11-03T10:23:05.123+0100`), as Ledger does.
pub fn ledger_time(jira: &str) -> Option<String> {
    let at = chrono::DateTime::parse_from_str(jira, "%Y-%m-%dT%H:%M:%S%.f%z").or_else(|_| chrono::DateTime::parse_from_rfc3339(jira)).ok()?;
    Some(crate::ids::format_millis(at.timestamp_millis()))
}

pub fn parse_issue(value: &Value) -> JResult<Issue> {
    let fields = &value["fields"];
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let key = text(&value["key"]);
    if key.is_empty() {
        return Err(unexpected());
    }
    let time = |v: &Value| ledger_time(v.as_str().unwrap_or_default()).unwrap_or_default();
    Ok(Issue {
        key,
        project: text(&fields["project"]["key"]),
        issue_type: text(&fields["issuetype"]["id"]),
        type_name: text(&fields["issuetype"]["name"]),
        summary: text(&fields["summary"]),
        description: Some(fields["description"].clone()).filter(|d| d.is_object()),
        status: text(&fields["status"]["id"]),
        status_name: text(&fields["status"]["name"]),
        labels: fields["labels"].as_array().into_iter().flatten().filter_map(|l| l.as_str().map(str::to_string)).collect(),
        created: time(&fields["created"]),
        updated: time(&fields["updated"]),
    })
}

/// A JQL string literal.
fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

impl<W: Wire> Jira for HttpJira<W> {
    fn myself(&self) -> JResult<Myself> {
        // The name and email address are null for an account that hides them.
        let me = self.get("/myself", &[])?;
        let text = |field: &str| me[field].as_str().unwrap_or_default().to_string();
        if text("accountId").is_empty() {
            return Err(unexpected());
        }
        Ok(Myself { account_id: text("accountId"), display_name: text("displayName"), email_address: text("emailAddress") })
    }

    fn projects(&self) -> JResult<Vec<Project>> {
        let mut projects = vec![];
        for page in 0..MAX_PAGES {
            let start = (page * PAGE).to_string();
            let body = self.get("/project/search", &[("startAt", &start), ("maxResults", &PAGE.to_string()), ("orderBy", "name")])?;
            let values: Vec<Project> = parse(body["values"].clone())?;
            let done = values.is_empty() || body["isLast"].as_bool().unwrap_or(true);
            projects.extend(values);
            if done {
                break;
            }
        }
        Ok(projects)
    }

    fn issue_types(&self, project: &str) -> JResult<Vec<IssueType>> {
        parse(self.get(format!("/project/{}/statuses", segment(project)), &[])?)
    }

    fn issues(&self, project: &str, types: &[String]) -> JResult<Vec<Issue>> {
        if types.is_empty() {
            return Ok(vec![]);
        }
        let types: Vec<String> = types.iter().map(|t| quote(t)).collect();
        let jql = format!("project = {} AND issuetype in ({}) ORDER BY key", quote(project), types.join(", "));
        let mut issues = vec![];
        let mut token: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut body = json!({ "jql": jql, "fields": ISSUE_FIELDS, "maxResults": PAGE });
            if let Some(token) = &token {
                body["nextPageToken"] = json!(token);
            }
            let page = self.write("POST", "/search/jql", body)?;
            for issue in page["issues"].as_array().into_iter().flatten() {
                issues.push(parse_issue(issue)?);
            }
            token = page["nextPageToken"].as_str().map(str::to_string).filter(|_| !page["isLast"].as_bool().unwrap_or(false));
            if token.is_none() {
                break;
            }
        }
        Ok(issues)
    }

    fn issue(&self, key: &str) -> JResult<Option<Issue>> {
        let (status, body) = self.send(&Request {
            method: "GET",
            path: format!("/issue/{}", segment(key)),
            query: vec![("fields".into(), ISSUE_FIELDS.join(","))],
            body: None,
        })?;
        match status {
            404 => Ok(None),
            200..=299 => parse_issue(&body).map(Some),
            _ => Err(failure(status, &body)),
        }
    }

    fn create_issue(&self, new: &NewIssue) -> JResult<String> {
        let mut fields = json!({
            "project": { "key": new.project },
            "issuetype": { "id": new.issue_type },
            "summary": new.summary,
            "labels": new.labels,
        });
        if let Some(description) = &new.description {
            fields["description"] = description.clone();
        }
        let made = self.write("POST", "/issue", json!({ "fields": fields }))?;
        made["key"].as_str().map(str::to_string).ok_or_else(unexpected)
    }

    fn update_issue(&self, key: &str, update: &IssueUpdate) -> JResult<()> {
        let mut body = json!({});
        if let Some(summary) = &update.summary {
            body["fields"]["summary"] = json!(summary);
        }
        if let Some(description) = &update.description {
            body["fields"]["description"] = description.clone().unwrap_or(Value::Null);
        }
        if let Some(label) = &update.add_label {
            body["update"]["labels"] = json!([{ "add": label }]);
        }
        self.write("PUT", format!("/issue/{}", segment(key)), body)?;
        Ok(())
    }

    fn transitions(&self, key: &str) -> JResult<Vec<Transition>> {
        let body = self.get(format!("/issue/{}/transitions", segment(key)), &[])?;
        Ok(body["transitions"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| Transition { id: t["id"].as_str().unwrap_or_default().to_string(), to: t["to"]["id"].as_str().unwrap_or_default().to_string() })
            .filter(|t| !t.id.is_empty())
            .collect())
    }

    fn transition(&self, key: &str, transition: &str) -> JResult<()> {
        self.write("POST", format!("/issue/{}/transitions", segment(key)), json!({ "transition": { "id": transition } }))?;
        Ok(())
    }
}
