//! The Jira sync: descriptions in Atlassian Document Format, and the sync itself against a fake Jira (in-memory
//! projects, issues, statuses and workflows): no network, no real credentials.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rusqlite::Connection;
use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

use crate::jira::{self, Connect, Emit, JiraEvent, Timing};
use crate::jira_adf::{self, canonical, to_adf, to_markdown};
use crate::jira_api::*;
use crate::jira_config::{self as config, ProjectLink};
use crate::jira_ledger::{CoreLedger, Ledger};
use crate::jira_sync::{self, LABEL, Report};
use crate::models::{Criterion, StoryDraft, StoryView, TaskUpdate, TaskView};
use crate::state::Core;
use crate::trello_api::{Reply, Request, Wire};
use crate::trello_card::{Check, Kind, StoryText, render_story};
use crate::{board, ids, projects, stories};

// ---- descriptions -------------------------------------------------------------------------------------

fn types(nodes: &Value) -> Vec<String> {
    nodes.as_array().unwrap().iter().map(|n| n["type"].as_str().unwrap().to_string()).collect()
}

#[test]
fn common_markdown_becomes_the_matching_adf() {
    let md = "# Plan\n\nFirst line\nsecond line with **bold**, _italic_, ~~gone~~, `code` and [a link](https://example.com).\n\n- one\n- two\n  - nested\n\n3. three\n4. four\n\n```rust\nfn main() {}\n```\n\n> quoted\n\n---";
    let doc = to_adf(md).unwrap();
    assert_eq!((doc["type"].as_str(), doc["version"].as_u64()), (Some("doc"), Some(1)));
    let content = &doc["content"];
    assert_eq!(types(content), ["heading", "paragraph", "bulletList", "orderedList", "codeBlock", "blockquote", "rule"]);
    assert_eq!(content[0]["attrs"]["level"], 1);

    let paragraph = &content[1]["content"];
    assert_eq!(paragraph[0], json!({ "type": "text", "text": "First line" }));
    assert_eq!(paragraph[1], json!({ "type": "hardBreak" }), "a line break stays a line break");
    let marked = |text: &str| paragraph.as_array().unwrap().iter().find(|n| n["text"] == text).unwrap_or_else(|| panic!("{text}"))["marks"].clone();
    assert_eq!(marked("bold"), json!([{ "type": "strong" }]));
    assert_eq!(marked("italic"), json!([{ "type": "em" }]));
    assert_eq!(marked("gone"), json!([{ "type": "strike" }]));
    assert_eq!(marked("code"), json!([{ "type": "code" }]));
    assert_eq!(marked("a link"), json!([{ "type": "link", "attrs": { "href": "https://example.com" } }]));

    let items = &content[2]["content"];
    assert_eq!(items.as_array().unwrap().len(), 2);
    assert_eq!(types(&items[1]["content"]), ["paragraph", "bulletList"], "the indented item is a list inside the one above");
    assert_eq!(content[3]["attrs"]["order"], 3);
    assert_eq!(content[4]["attrs"]["language"], "rust");
    assert_eq!(content[4]["content"][0]["text"], "fn main() {}");
    assert_eq!(to_adf("  \n\n "), None, "nothing is no description");
}

#[test]
fn what_is_written_comes_back_as_it_was() {
    for md in [
        "# Plan\n\nSome **bold** and _italic_ text.",
        "First line\nsecond line",
        "- one\n- two\n  - nested\n    - deeper\n- three",
        "1. one\n2. two\n\n5. five",
        "```\nplain code\n```",
        "```js\nlet x = 1;\n\nlet y = 2;\n```",
        "> a quote\n> on two lines",
        "**bold _and italic_ inside**",
        "a [link with **bold**](https://example.com/a?b=c) here",
        "snake_case_words stay as they are, and so does 2 * 3",
        "~~struck~~ and `code with *stars*`",
        "---",
    ] {
        assert_eq!(canonical(md), md, "{md:?} came back as {:?}", canonical(md));
    }
}

#[test]
fn every_description_has_one_canonical_form() {
    // Whatever is typed, a second trip changes nothing: two sides that agree stay agreeing.
    for md in [
        "**unclosed bold",
        "*mixed **marks* here**",
        "***three***",
        "_a_b_ and __c__",
        "[not a link] and [also](not a link) and [](empty)",
        "# \n#nospace\n####### seven",
        "- \n-\n- [ ] open\n- [x] done",
        "2026. was a year\n- not a list item here?\n> nor a quote",
        "line with trailing spaces   \n  indented line",
        "\\*escaped\\* and a lone \\ backslash",
        "text\n\n\n\nfar below",
        "* star list\n+ plus list\n1) paren list",
        "a ~~b~~~c~~ d",
        "`` empty code `` and ` spaced `",
        "**bold**_italic_~~strike~~`code`",
        "> # heading in a quote\n> - list in a quote\n>\n> ---",
        "    - deep first\n- then shallow",
        "1. one\n   - nested under a number",
    ] {
        let once = canonical(md);
        assert_eq!(canonical(&once), once, "{md:?} → {once:?}");
    }
}

#[test]
fn text_that_looks_like_markup_is_escaped_and_comes_back_as_text() {
    let doc = json!({ "type": "doc", "version": 1, "content": [
        { "type": "paragraph", "content": [{ "type": "text", "text": "# not a heading" }] },
        { "type": "paragraph", "content": [{ "type": "text", "text": "- not a list" }, { "type": "hardBreak" }, { "type": "text", "text": "2026. not a number" }] },
        { "type": "paragraph", "content": [{ "type": "text", "text": "*not italic* and [not](a link)" }] },
    ]});
    let (md, lossless) = to_markdown(Some(&doc));
    assert!(lossless);
    assert_eq!(md, "\\# not a heading\n\n\\- not a list\n2026\\. not a number\n\n\\*not italic\\* and \\[not\\](a link)");
    assert_eq!(to_adf(&md).unwrap(), doc, "and it reads back as the same text");
}

#[test]
fn a_story_keeps_its_parts_through_jira() {
    let text = StoryText {
        role: "customer".into(),
        want: "see prices in **bold**".into(),
        reason: "I can decide".into(),
        criteria: vec![Check { text: "A page lists plans".into(), done: true }, Check { text: "Prices in euro".into(), done: false }],
        notes: "Raised on the first call.\n\n- a list in the notes".into(),
    };
    let rendered = render_story(&text);
    let doc = to_adf(&rendered).unwrap();
    assert_eq!(types(&doc["content"]), ["paragraph", "paragraph", "bulletList", "paragraph", "bulletList"]);
    let (back, lossless) = jira_adf::from_jira(Some(&doc));
    assert!(lossless);
    assert_eq!(jira_sync::story_from_jira(&back), text);
}

#[test]
fn what_markdown_here_cannot_say_is_kept_as_text_and_reported() {
    let doc = json!({ "type": "doc", "version": 1, "content": [
        { "type": "paragraph", "content": [
            { "type": "text", "text": "Ask " },
            { "type": "mention", "attrs": { "id": "abc", "text": "@Ana" } },
            { "type": "text", "text": " about it", "marks": [{ "type": "textColor", "attrs": { "color": "#ff0000" } }] },
        ]},
        { "type": "table", "content": [{ "type": "tableRow", "content": [{ "type": "tableCell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "cell" }] }] }] }] },
        { "type": "mediaSingle", "content": [{ "type": "media", "attrs": { "id": "1", "type": "file" } }] },
    ]});
    let (md, lossless) = to_markdown(Some(&doc));
    assert!(!lossless);
    assert!(md.contains("Ask @Ana about it") && md.contains("cell"), "{md}");

    let plain = json!({ "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "fine" }] }] });
    assert_eq!(to_markdown(Some(&plain)), ("fine".to_string(), true));
    assert_eq!(to_markdown(None), (String::new(), true));
    assert_eq!(to_markdown(Some(&Value::Null)), (String::new(), true));
}

// ---- a fake Jira --------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fail {
    RateLimited,
    Unauthorized,
}

fn failure(fail: Fail) -> JiraError {
    match fail {
        Fail::RateLimited => JiraError::RateLimited { retry_after: Duration::from_millis(1) },
        Fail::Unauthorized => JiraError::Unauthorized,
    }
}

const STORY: &str = "10001";
const TASK: &str = "10002";
const BUG: &str = "10003";
// The Task workflow's statuses. The board's default columns are Backlog, Doing and Done; Review has no column.
const BACKLOG: &str = "1";
const DOING: &str = "2";
const DONE: &str = "3";
const REVIEW: &str = "4";

#[derive(Default)]
struct World {
    projects: Vec<Project>,
    issues: BTreeMap<String, Issue>,
    /// An issue moved to another project: its old key → its new one.
    moved: HashMap<String, String>,
    next: HashMap<String, u32>,
    /// Transitions the workflow doesn't offer: (from, to).
    blocked: HashSet<(String, String)>,
    /// Every call fails like this.
    fail: Option<Fail>,
    /// Updating these issues fails.
    fail_update: HashSet<String>,
    calls: Vec<(String, Instant)>,
    /// How often each issue was written to (made, changed or moved).
    writes: HashMap<String, usize>,
    clock: i64,
}

struct FakeJira {
    world: Mutex<World>,
}

fn task_type() -> IssueType {
    let status = |id: &str, name: &str| Status { id: id.into(), name: name.into() };
    IssueType { id: TASK.into(), name: "Task".into(), subtask: false, statuses: vec![status(BACKLOG, "Backlog"), status(DOING, "Doing"), status(DONE, "Done"), status(REVIEW, "Review")] }
}

fn issue_types() -> Vec<IssueType> {
    let status = |id: &str, name: &str| Status { id: id.into(), name: name.into() };
    vec![
        IssueType { id: STORY.into(), name: "Story".into(), subtask: false, statuses: vec![status("11", "To Do"), status("13", "Done")] },
        task_type(),
        IssueType { id: BUG.into(), name: "Bug".into(), subtask: false, statuses: vec![status(BACKLOG, "Backlog")] },
        IssueType { id: "10004".into(), name: "Subtask".into(), subtask: true, statuses: vec![] },
    ]
}

/// Jira rejects a description that isn't well-formed ADF; so does the fake, loudly.
fn check_adf(node: &Value) {
    let kind = node["type"].as_str().unwrap_or_else(|| panic!("a node without a type: {node}"));
    let known = ["doc", "paragraph", "text", "hardBreak", "heading", "bulletList", "orderedList", "listItem", "codeBlock", "blockquote", "rule"];
    assert!(known.contains(&kind), "Ledger wrote a {kind} node");
    if kind == "doc" {
        assert_eq!(node["version"], 1);
    }
    if kind == "text" {
        assert!(!node["text"].as_str().unwrap_or_default().is_empty(), "an empty text node");
    }
    if kind == "listItem" {
        assert_eq!(node["content"][0]["type"], "paragraph", "a list item starts with a paragraph");
    }
    for child in node["content"].as_array().into_iter().flatten() {
        check_adf(child);
    }
}

impl FakeJira {
    fn standard() -> FakeJira {
        let world = World {
            projects: vec![
                Project { id: "100".into(), key: "ACME".into(), name: "Acme Jira".into() },
                Project { id: "200".into(), key: "OTHER".into(), name: "Other".into() },
            ],
            ..World::default()
        };
        FakeJira { world: Mutex::new(world) }
    }

    fn w(&self) -> MutexGuard<'_, World> {
        self.world.lock().unwrap()
    }

    fn call(&self, name: &str) -> JResult<()> {
        let mut w = self.w();
        w.calls.push((name.to_string(), Instant::now()));
        match w.fail {
            Some(fail) => Err(failure(fail)),
            None => Ok(()),
        }
    }

    fn calls(&self, name: &str) -> usize {
        self.w().calls.iter().filter(|(n, _)| n == name).count()
    }

    fn call_times(&self, name: &str) -> Vec<Instant> {
        self.w().calls.iter().filter(|(n, _)| n == name).map(|(_, at)| *at).collect()
    }

    fn get(&self, key: &str) -> Issue {
        self.w().issues.get(key).cloned().unwrap_or_else(|| panic!("no issue {key}"))
    }

    fn keys(&self, project: &str) -> Vec<String> {
        self.w().issues.values().filter(|i| i.project == project).map(|i| i.key.clone()).collect()
    }

    fn writes(&self, key: &str) -> usize {
        self.w().writes.get(key).copied().unwrap_or(0)
    }

    fn total_writes(&self) -> usize {
        self.w().writes.values().sum()
    }

    fn markdown(&self, key: &str) -> String {
        to_markdown(self.get(key).description.as_ref()).0
    }

    /// Something a person did on Jira.
    fn add(&self, project: &str, issue_type: &str, summary: &str, description: &str, status: &str) -> String {
        let mut w = self.w();
        let at = tick(&mut w);
        let n = w.next.entry(project.to_string()).or_insert(0);
        *n += 1;
        let key = format!("{project}-{n}");
        let type_name = issue_types().into_iter().find(|t| t.id == issue_type).unwrap().name;
        w.issues.insert(
            key.clone(),
            Issue {
                key: key.clone(),
                project: project.into(),
                issue_type: issue_type.into(),
                type_name,
                summary: summary.into(),
                description: to_adf(description),
                status: status.into(),
                status_name: String::new(),
                labels: vec![],
                created: at.clone(),
                updated: at,
            },
        );
        key
    }

    fn edit(&self, key: &str, change: impl FnOnce(&mut Issue)) {
        let mut w = self.w();
        let at = tick(&mut w);
        let issue = w.issues.get_mut(key).unwrap();
        change(issue);
        issue.updated = at;
    }

    /// Moves an issue to another project, which gives it another key.
    fn move_to(&self, key: &str, project: &str) -> String {
        let mut issue = self.get(key);
        let mut w = self.w();
        w.issues.remove(key);
        let n = w.next.entry(project.to_string()).or_insert(0);
        *n += 1;
        issue.key = format!("{project}-{n}");
        issue.project = project.into();
        w.moved.insert(key.to_string(), issue.key.clone());
        w.issues.insert(issue.key.clone(), issue.clone());
        issue.key
    }
}

/// An issue as Jira hands it out: with the name of its status.
fn named(mut issue: Issue) -> Issue {
    issue.status_name = task_type().statuses.into_iter().find(|s| s.id == issue.status).map(|s| s.name).unwrap_or_default();
    issue
}

/// The fake's clock: a second further on every time it is read.
fn tick(w: &mut World) -> String {
    w.clock += 1;
    ids::format_millis(1_790_000_000_000 + w.clock * 1000)
}

impl Jira for FakeJira {
    fn myself(&self) -> JResult<Myself> {
        self.call("myself")?;
        Ok(Myself { account_id: "a1".into(), display_name: "Ana Smith".into(), email_address: "ana@example.com".into() })
    }

    fn projects(&self) -> JResult<Vec<Project>> {
        self.call("projects")?;
        Ok(self.w().projects.clone())
    }

    fn issue_types(&self, project: &str) -> JResult<Vec<IssueType>> {
        self.call("issue_types")?;
        if !self.w().projects.iter().any(|p| p.key == project) {
            return Err(JiraError::message("Jira can't find that project or issue."));
        }
        Ok(issue_types())
    }

    fn issues(&self, project: &str, wanted: &[String]) -> JResult<Vec<Issue>> {
        self.call("issues")?;
        Ok(self.w().issues.values().filter(|i| i.project == project && wanted.contains(&i.issue_type)).cloned().map(named).collect())
    }

    fn issue(&self, key: &str) -> JResult<Option<Issue>> {
        self.call("issue")?;
        let w = self.w();
        let key = w.moved.get(key).cloned().unwrap_or_else(|| key.to_string());
        Ok(w.issues.get(&key).cloned().map(named))
    }

    fn create_issue(&self, new: &NewIssue) -> JResult<String> {
        self.call("create")?;
        if let Some(description) = &new.description {
            check_adf(description);
        }
        let key = self.add(&new.project, &new.issue_type, &new.summary, "", BACKLOG);
        let mut w = self.w();
        let issue = w.issues.get_mut(&key).unwrap();
        issue.description = new.description.clone();
        issue.labels = new.labels.clone();
        *w.writes.entry(key.clone()).or_default() += 1;
        Ok(key)
    }

    fn update_issue(&self, key: &str, update: &IssueUpdate) -> JResult<()> {
        self.call("update")?;
        if self.w().fail_update.contains(key) {
            return Err(JiraError::message("Jira refused the request: summary: too long"));
        }
        if let Some(Some(description)) = &update.description {
            check_adf(description);
        }
        self.edit(key, |issue| {
            if let Some(summary) = &update.summary {
                issue.summary = summary.clone();
            }
            if let Some(description) = &update.description {
                issue.description = description.clone();
            }
            if let Some(label) = &update.add_label
                && !issue.labels.contains(label)
            {
                issue.labels.push(label.clone());
            }
        });
        *self.w().writes.entry(key.to_string()).or_default() += 1;
        Ok(())
    }

    fn transitions(&self, key: &str) -> JResult<Vec<Transition>> {
        self.call("transitions")?;
        let issue = self.get(key);
        let w = self.w();
        Ok(task_type()
            .statuses
            .iter()
            .filter(|s| s.id != issue.status && !w.blocked.contains(&(issue.status.clone(), s.id.clone())))
            .map(|s| Transition { id: format!("t{}", s.id), to: s.id.clone() })
            .collect())
    }

    fn transition(&self, key: &str, transition: &str) -> JResult<()> {
        self.call("transition")?;
        let to = transition.trim_start_matches('t').to_string();
        let from = self.get(key).status;
        assert!(!self.w().blocked.contains(&(from, to.clone())), "a transition the workflow doesn't offer");
        self.edit(key, |issue| issue.status = to);
        *self.w().writes.entry(key.to_string()).or_default() += 1;
        Ok(())
    }
}

impl Jira for Arc<FakeJira> {
    fn myself(&self) -> JResult<Myself> {
        (**self).myself()
    }
    fn projects(&self) -> JResult<Vec<Project>> {
        (**self).projects()
    }
    fn issue_types(&self, project: &str) -> JResult<Vec<IssueType>> {
        (**self).issue_types(project)
    }
    fn issues(&self, project: &str, types: &[String]) -> JResult<Vec<Issue>> {
        (**self).issues(project, types)
    }
    fn issue(&self, key: &str) -> JResult<Option<Issue>> {
        (**self).issue(key)
    }
    fn create_issue(&self, new: &NewIssue) -> JResult<String> {
        (**self).create_issue(new)
    }
    fn update_issue(&self, key: &str, update: &IssueUpdate) -> JResult<()> {
        (**self).update_issue(key, update)
    }
    fn transitions(&self, key: &str) -> JResult<Vec<Transition>> {
        (**self).transitions(key)
    }
    fn transition(&self, key: &str, transition: &str) -> JResult<()> {
        (**self).transition(key, transition)
    }
}

// ---- a machine: a vault, loaded ------------------------------------------------------------------------

struct Machine {
    dir: TempDir,
    _config: TempDir,
    core: Arc<Mutex<Core>>,
    ledger: CoreLedger,
}

impl Machine {
    fn new() -> Machine {
        let dir = tempdir().unwrap();
        let config = tempdir().unwrap();
        let core = Arc::new(Mutex::new(Core::open_at(dir.path().to_path_buf(), config.path().to_path_buf(), false).unwrap()));
        let ledger = CoreLedger { core: core.clone(), changed: Arc::new(|| {}) };
        Machine { dir, _config: config, core, ledger }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn read<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        f(&self.core.lock().unwrap().conn)
    }

    /// Changes the database and writes the difference to the files, as the app does for every edit.
    fn edit<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let mut guard = self.core.lock().unwrap();
        let core = &mut *guard;
        let out = f(&core.conn);
        core.vault.persist(&core.conn).unwrap();
        out
    }

    fn project(&self, name: &str) -> String {
        self.edit(|c| projects::create(c, name).unwrap().slug)
    }

    fn task(&self, project: &str, title: &str, body: &str) -> String {
        self.edit(|c| board::create_task(c, project, title, body, None, None).unwrap().id)
    }

    fn update_task(&self, project: &str, id: &str, updates: TaskUpdate) {
        self.edit(|c| board::update_task(c, project, id, updates).unwrap());
    }

    fn story(&self, project: &str, draft: StoryDraft) -> String {
        self.edit(|c| stories::create(c, project, draft).unwrap().id)
    }

    fn get_task(&self, project: &str, id: &str) -> TaskView {
        self.read(|c| board::task(c, project, id).unwrap_or_else(|e| panic!("task {id}: {e}")))
    }

    fn get_story(&self, project: &str, id: &str) -> StoryView {
        self.read(|c| stories::get(c, project, id).unwrap_or_else(|e| panic!("story {id}: {e}")))
    }

    fn tasks(&self, project: &str) -> Vec<TaskView> {
        self.read(|c| board::tasks(c, project).unwrap())
    }

    fn stories(&self, project: &str) -> Vec<StoryView> {
        self.read(|c| stories::list(c, project).unwrap())
    }

    /// Links the project to a Jira project, as the settings screen would.
    fn link(&self, project: &str, key: &str) {
        config::update_settings(self.root(), |s| {
            s.site = "https://acme.atlassian.net".into();
            s.projects.insert(project.into(), ProjectLink { project: key.into(), project_name: format!("{key} on Jira"), columns: BTreeMap::new() });
        })
        .unwrap();
    }

    fn try_run(&self, jira: &dyn Jira) -> JResult<Report> {
        let settings = config::read_settings(self.root());
        let snapshot = self.ledger.snapshot().unwrap();
        jira_sync::run(self.root(), jira, &self.ledger, &settings, &snapshot)
    }

    fn run(&self, jira: &FakeJira) -> Report {
        self.try_run(jira).unwrap_or_else(|e| panic!("the round failed: {e}"))
    }

    fn file(&self, rel: &str) -> String {
        fs::read_to_string(self.root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    fn reload(&self) {
        self.core.lock().unwrap().reload().unwrap();
    }

    /// What another machine would get from the GitHub sync: every file under `projects/`, copied over.
    fn copy_vault_to(&self, other: &Machine) {
        fn copy(from: &Path, to: &Path) {
            for entry in fs::read_dir(from).unwrap().flatten() {
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    fs::create_dir_all(&target).unwrap();
                    copy(&entry.path(), &target);
                } else {
                    fs::write(&target, fs::read(entry.path()).unwrap()).unwrap();
                }
            }
        }
        fs::create_dir_all(other.root().join("projects")).unwrap();
        copy(&self.root().join("projects"), &other.root().join("projects"));
        other.reload();
    }

    fn conflict_files(&self) -> Vec<(String, String)> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, String)>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                if entry.file_type().unwrap().is_dir() {
                    walk(&entry.path(), base, out);
                } else {
                    let rel = entry.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                    out.push((rel, fs::read_to_string(entry.path()).unwrap()));
                }
            }
        }
        let base = self.root().join(".ledger/conflicts");
        let mut out = vec![];
        walk(&base, &base, &mut out);
        out.sort();
        out
    }
}

/// One machine with a project "acme" linked to the Jira project ACME, and the fake Jira it talks to.
fn setup() -> (FakeJira, Machine, String) {
    let jira = FakeJira::standard();
    let machine = Machine::new();
    let project = machine.project("Acme");
    machine.link(&project, "ACME");
    (jira, machine, project)
}

fn story_draft(id: &str, summary: &str) -> StoryDraft {
    StoryDraft {
        id: id.into(),
        summary: summary.into(),
        role: "customer".into(),
        want: "see prices".into(),
        reason: "I can decide".into(),
        notes: "Raised on the first call.".into(),
        criteria: vec![Criterion { text: "A page lists plans".into(), done: true }, Criterion { text: "Prices in euro".into(), done: false }],
        thread_slugs: vec![],
    }
}

const STORY_DESC: &str = "**As a** customer\n**I want to** see prices\n**Such that** I can decide\n\n**Acceptance criteria**\n- [x] A page lists plans\n- [ ] Prices in euro\n\n**Notes**\nRaised on the first call.";

fn moved(column: &str) -> TaskUpdate {
    TaskUpdate { column_id: Some(column.into()), ..TaskUpdate::default() }
}

// ---- Ledger to Jira -----------------------------------------------------------------------------------

#[test]
fn a_new_task_becomes_a_task_issue_in_the_status_of_its_column_with_the_label() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Write the plan", "First **draft**\n\n- one\n- two");
    m.update_task(&p, &id, moved("doing"));

    let report = m.run(&jira);
    assert_eq!(report.issues_created, 1);
    let key = m.get_task(&p, &id).jira_issue.expect("the task knows its issue");
    assert_eq!(key, "ACME-1");
    let issue = jira.get(&key);
    assert_eq!((issue.issue_type.as_str(), issue.summary.as_str(), issue.status.as_str()), (TASK, "Write the plan", DOING));
    assert_eq!(issue.labels, [LABEL]);
    assert_eq!(jira.markdown(&key), "First **draft**\n\n- one\n- two");
    assert!(m.file(&format!("projects/{p}/tasks/{id}.md")).contains("jiraIssue: ACME-1"), "the key travels with the item");
}

#[test]
fn a_new_story_becomes_a_story_issue_with_a_readable_description() {
    let (jira, m, p) = setup();
    let id = m.story(&p, story_draft("MM01", "Show prices"));
    m.run(&jira);
    let key = m.get_story(&p, &id).jira_issue.unwrap();
    let issue = jira.get(&key);
    assert_eq!((issue.issue_type.as_str(), issue.summary.as_str()), (STORY, "Show prices"));
    assert_eq!(jira.markdown(&key), STORY_DESC.replace("**Acceptance criteria**\n", "**Acceptance criteria**\n\n"));
    let content = &issue.description.as_ref().unwrap()["content"];
    assert_eq!(content[2]["type"], "bulletList", "the criteria are a list in Jira");
}

#[test]
fn a_task_in_a_column_with_no_status_is_left_out_and_reported() {
    let (jira, m, p) = setup();
    let column = m.edit(|c| board::add_column(c, &p, "Waiting").unwrap().into_iter().find(|c| c.name == "Waiting").unwrap().id);
    let id = m.task(&p, "Parked", "");
    m.update_task(&p, &id, moved(&column));
    let report = m.run(&jira);
    assert_eq!((report.issues_created, report.unmatched), (0, 1));
    assert!(report.notes.iter().any(|n| n.contains("“Waiting”")), "{:?}", report.notes);
    assert!(jira.keys("ACME").is_empty());

    // Given a status of its own, it goes up.
    jira::set_column_status(m.root(), &p, &column, REVIEW).unwrap();
    assert_eq!(m.run(&jira).issues_created, 1);
    assert_eq!(jira.get("ACME-1").status, REVIEW);
}

#[test]
fn nothing_is_made_on_jira_for_a_project_that_is_not_linked() {
    let (jira, m, _) = setup();
    let other = m.project("Private");
    m.task(&other, "Mine alone", "");
    m.run(&jira);
    assert!(jira.w().issues.is_empty());
}

// ---- Jira to Ledger -----------------------------------------------------------------------------------

#[test]
fn a_task_issue_comes_in_in_the_column_of_its_status_and_gets_the_label() {
    let (jira, m, p) = setup();
    let key = jira.add("ACME", TASK, "From Jira", "Some _text_", DONE);
    let report = m.run(&jira);
    assert_eq!(report.items_created, 1);
    let task = m.get_task(&p, "jira-acme-1");
    assert_eq!((task.title.as_str(), task.body.as_str(), task.column_id.as_str()), ("From Jira", "Some _text_", "done"));
    assert_eq!(task.jira_issue.as_deref(), Some(key.as_str()));
    assert_eq!(task.created_at, jira.get(&key).created, "its times come from the issue, so every machine writes the same file");
    assert_eq!(jira.get(&key).labels, [LABEL], "the issue now says it has an item");
}

#[test]
fn a_story_issue_comes_in_as_a_story_with_its_parts() {
    let (jira, m, p) = setup();
    let key = jira.add("ACME", STORY, "Show prices", STORY_DESC, "11");
    m.run(&jira);
    let story = m.get_story(&p, &key);
    assert_eq!((story.role.as_str(), story.want.as_str(), story.reason.as_str()), ("customer", "see prices", "I can decide"));
    assert_eq!(story.criteria.iter().map(|c| (c.text.as_str(), c.done)).collect::<Vec<_>>(), [("A page lists plans", true), ("Prices in euro", false)]);
    assert_eq!(story.notes, "Raised on the first call.");
}

#[test]
fn other_issue_types_and_statuses_without_a_column_are_left_alone() {
    let (jira, m, p) = setup();
    let bug = jira.add("ACME", BUG, "A bug", "", BACKLOG);
    jira.add("ACME", TASK, "In review", "", REVIEW);
    let report = m.run(&jira);
    assert!(m.tasks(&p).is_empty() && m.stories(&p).is_empty());
    assert_eq!(report.unmatched, 1);
    assert!(report.notes.iter().any(|n| n.contains("“Review”")), "{:?}", report.notes);
    assert!(jira.get(&bug).labels.is_empty() && jira.writes(&bug) == 0, "a bug is never touched");
}

// ---- both ways ----------------------------------------------------------------------------------------

#[test]
fn edits_to_a_task_go_to_its_issue_and_edits_to_the_issue_come_back() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Plan", "v1");
    m.run(&jira);
    let key = m.get_task(&p, &id).jira_issue.unwrap();

    m.update_task(&p, &id, TaskUpdate { title: Some("Plan, renamed".into()), body: Some("v2 with `code`".into()), ..TaskUpdate::default() });
    assert_eq!(m.run(&jira).issues_updated, 1);
    assert_eq!((jira.get(&key).summary.as_str(), jira.markdown(&key).as_str()), ("Plan, renamed", "v2 with `code`"));

    jira.edit(&key, |i| {
        i.summary = "Renamed on Jira".into();
        i.description = to_adf("# v3\n\nfrom Jira");
    });
    assert_eq!(m.run(&jira).items_updated, 1);
    let task = m.get_task(&p, &id);
    assert_eq!((task.title.as_str(), task.body.as_str()), ("Renamed on Jira", "# v3\n\nfrom Jira"));
    assert_eq!(task.updated_at, jira.get(&key).updated);
}

#[test]
fn moving_a_task_moves_its_issue_and_moving_the_issue_moves_the_task() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Move me", "");
    m.run(&jira);
    let key = m.get_task(&p, &id).jira_issue.unwrap();
    assert_eq!(jira.get(&key).status, BACKLOG);

    m.update_task(&p, &id, moved("doing"));
    m.run(&jira);
    assert_eq!(jira.get(&key).status, DOING);

    jira.edit(&key, |i| i.status = DONE.into());
    m.run(&jira);
    assert_eq!(m.get_task(&p, &id).column_id, "done");

    // A status no column uses leaves the task where it is.
    jira.edit(&key, |i| i.status = REVIEW.into());
    let report = m.run(&jira);
    assert_eq!((m.get_task(&p, &id).column_id.as_str(), report.unmatched), ("done", 1));
}

#[test]
fn a_move_the_workflow_does_not_allow_is_a_note_and_is_tried_again() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Skip ahead", "");
    m.run(&jira);
    let key = m.get_task(&p, &id).jira_issue.unwrap();
    jira.w().blocked.insert((BACKLOG.into(), DONE.into()));

    m.update_task(&p, &id, moved("done"));
    let report = m.run(&jira);
    assert_eq!(jira.get(&key).status, BACKLOG);
    assert!(report.notes.iter().any(|n| n.contains("no transition")), "{:?}", report.notes);
    assert_eq!(m.get_task(&p, &id).column_id, "done", "the task stays where it was put");

    jira.w().blocked.clear();
    m.run(&jira);
    assert_eq!(jira.get(&key).status, DONE);
}

#[test]
fn a_task_made_in_another_column_is_moved_there_after_it_is_made() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Already done", "");
    m.update_task(&p, &id, moved("done"));
    m.run(&jira);
    assert_eq!(jira.get("ACME-1").status, DONE);
    assert_eq!(m.run(&jira), Report::default(), "and nothing is left to do");
}

#[test]
fn edits_to_a_story_go_to_its_issue_and_its_parts_come_back_whole() {
    let (jira, m, p) = setup();
    let id = m.story(&p, story_draft("MM01", "Show prices"));
    m.run(&jira);
    let key = m.get_story(&p, &id).jira_issue.unwrap();

    m.edit(|c| stories::update(c, &p, &id, crate::models::StoryUpdate { reason: Some("I can compare".into()), ..Default::default() }).unwrap());
    m.run(&jira);
    assert!(jira.markdown(&key).contains("**Such that** I can compare"), "{}", jira.markdown(&key));

    let edited = STORY_DESC.replace("- [ ] Prices in euro", "- [x] Prices in euro\n- [ ] Prices in dollars");
    jira.edit(&key, |i| i.description = to_adf(&edited));
    m.run(&jira);
    let story = m.get_story(&p, &id);
    assert_eq!(story.criteria.iter().map(|c| (c.text.as_str(), c.done)).collect::<Vec<_>>(), [("A page lists plans", true), ("Prices in euro", true), ("Prices in dollars", false)]);
    assert_eq!(story.reason, "I can decide", "the description says everything, and Ledger follows it");
}

#[test]
fn a_description_written_freely_in_jira_keeps_the_storys_parts() {
    let (jira, m, p) = setup();
    let id = m.story(&p, story_draft("MM01", "Show prices"));
    m.run(&jira);
    let key = m.get_story(&p, &id).jira_issue.unwrap();
    jira.edit(&key, |i| i.description = to_adf("Just some words from Jira."));
    m.run(&jira);
    let story = m.get_story(&p, &id);
    assert_eq!((story.notes.as_str(), story.role.as_str(), story.criteria.len()), ("Just some words from Jira.", "customer", 2));
    // The next round writes the parts back into the description.
    m.run(&jira);
    assert!(jira.markdown(&key).contains("**As a** customer") && jira.markdown(&key).contains("Just some words from Jira."));
}

#[test]
fn a_description_with_things_ledger_cannot_write_is_never_overwritten() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Has a picture", "words");
    m.run(&jira);
    let key = m.get_task(&p, &id).jira_issue.unwrap();
    let with_picture = json!({ "type": "doc", "version": 1, "content": [
        { "type": "paragraph", "content": [{ "type": "text", "text": "words" }] },
        { "type": "mediaSingle", "content": [{ "type": "media", "attrs": { "id": "1", "type": "file" } }] },
    ]});
    jira.edit(&key, |i| i.description = Some(with_picture.clone()));
    m.run(&jira);

    m.update_task(&p, &id, TaskUpdate { body: Some("words, edited here".into()), title: Some("Renamed".into()), ..TaskUpdate::default() });
    let report = m.run(&jira);
    let issue = jira.get(&key);
    assert_eq!(issue.description, Some(with_picture.clone()), "the picture is still there");
    assert_eq!(issue.summary, "Renamed", "the title still goes up");
    assert!(report.notes.iter().any(|n| n.contains("can't write")), "{:?}", report.notes);
    assert_eq!(m.get_task(&p, &id).body, "words, edited here", "and the edit stays here");

    // When Jira's description changes too, Jira's is taken and this side's is set aside.
    let newer = json!({ "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "newer" }] }, with_picture["content"][1].clone()] });
    jira.edit(&key, |i| i.description = Some(newer.clone()));
    let report = m.run(&jira);
    assert_eq!(report.conflicts, 1);
    assert_eq!(m.get_task(&p, &id).body, "newer");
    assert_eq!(jira.get(&key).description, Some(newer));
    let kept = m.conflict_files();
    assert!(kept.iter().any(|(path, text)| path.ends_with(&format!("jira/{p}/tasks-{id}.ledger.md")) && text.contains("words, edited here")), "{kept:?}");
}

#[test]
fn a_task_edited_while_the_round_runs_is_left_for_the_next_round() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Racy", "");
    m.run(&jira);
    let key = m.get_task(&p, &id).jira_issue.unwrap();
    jira.edit(&key, |i| i.summary = "From Jira".into());

    // The snapshot is taken, then the task is edited here before the change from Jira is written.
    let settings = config::read_settings(m.root());
    let snapshot = m.ledger.snapshot().unwrap();
    m.update_task(&p, &id, TaskUpdate { body: Some("typed meanwhile".into()), ..TaskUpdate::default() });
    let report = jira_sync::run(m.root(), &jira, &m.ledger, &settings, &snapshot).unwrap();
    assert_eq!(report.skipped, 1);
    assert_eq!(m.get_task(&p, &id).title, "Racy");

    m.run(&jira);
    let task = m.get_task(&p, &id);
    assert_eq!((task.title.as_str(), task.body.as_str()), ("From Jira", "typed meanwhile"));
    assert_eq!(jira.markdown(&key), "typed meanwhile");
}

// ---- conflicts ----------------------------------------------------------------------------------------

#[test]
fn when_both_sides_change_a_field_this_machine_wins_and_jiras_version_is_kept() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Title", "body");
    m.run(&jira);
    let key = m.get_task(&p, &id).jira_issue.unwrap();

    m.update_task(&p, &id, TaskUpdate { title: Some("Title from Ledger".into()), ..TaskUpdate::default() });
    jira.edit(&key, |i| {
        i.summary = "Title from Jira".into();
        i.description = to_adf("body from Jira");
    });
    let report = m.run(&jira);
    assert_eq!(report.conflicts, 1);
    assert_eq!(jira.get(&key).summary, "Title from Ledger");
    let task = m.get_task(&p, &id);
    assert_eq!((task.title.as_str(), task.body.as_str()), ("Title from Ledger", "body from Jira"), "fields are merged one by one");

    let kept = m.conflict_files();
    assert_eq!(kept.len(), 1, "{kept:?}");
    let (path, text) = &kept[0];
    assert!(path.contains("/jira/") && path.ends_with(&format!("tasks-{id}.jira.md")), "{path}");
    assert!(text.contains("Title from Jira") && text.contains("body from Jira") && text.contains(&key), "{text}");
    assert_eq!(m.run(&jira), Report::default());
}

#[test]
fn a_second_machine_with_no_record_of_the_last_sync_settles_a_difference_its_own_way() {
    let (jira, a, p) = setup();
    let id = a.task(&p, "Shared", "");
    a.run(&jira);
    let key = a.get_task(&p, &id).jira_issue.unwrap();

    let b = Machine::new();
    a.copy_vault_to(&b);
    b.link(&p, "ACME");
    jira.edit(&key, |i| i.summary = "Changed on Jira".into());
    b.update_task(&p, &id, TaskUpdate { title: Some("Changed on B".into()), ..TaskUpdate::default() });
    let report = b.run(&jira);
    assert_eq!(report.conflicts, 1);
    assert_eq!(jira.get(&key).summary, "Changed on B");
}

// ---- no duplicates ------------------------------------------------------------------------------------

#[test]
fn syncing_again_and_again_changes_nothing_and_makes_nothing() {
    let (jira, m, p) = setup();
    m.task(&p, "A task", "with **bold**, _italic_ and a [link](https://example.com)\n\n1. one\n2. two");
    m.story(&p, story_draft("MM01", "Show prices"));
    jira.add("ACME", TASK, "From Jira", "> quoted\n\n```\ncode\n```", DOING);
    jira.add("ACME", STORY, "Story from Jira", STORY_DESC, "11");
    m.run(&jira);
    let writes = jira.total_writes();
    let files = (m.tasks(&p).len(), m.stories(&p).len());
    for _ in 0..3 {
        let report = m.run(&jira);
        assert_eq!(report, Report::default(), "{report:?}");
    }
    assert_eq!(jira.total_writes(), writes);
    assert_eq!((m.tasks(&p).len(), m.stories(&p).len()), files);
    assert_eq!(jira.w().issues.len(), 4);
}

#[test]
fn a_second_machine_that_has_the_item_makes_no_issue_and_no_duplicate() {
    let (jira, a, p) = setup();
    a.task(&p, "Made on A", "");
    a.run(&jira);
    let b = Machine::new();
    a.copy_vault_to(&b);
    b.link(&p, "ACME");
    let report = b.run(&jira);
    assert_eq!((report.issues_created, report.items_created), (0, 0));
    assert_eq!(jira.keys("ACME").len(), 1);
    assert_eq!(b.tasks(&p).len(), 1);
}

#[test]
fn an_issue_ledger_made_is_never_imported_by_a_machine_that_does_not_have_its_item_yet() {
    let (jira, a, p) = setup();
    a.task(&p, "Made on A", "");
    a.run(&jira);

    // B polls Jira before A's task has reached it through GitHub.
    let b = Machine::new();
    let pb = b.project("Acme");
    assert_eq!(pb, p);
    b.link(&p, "ACME");
    let report = b.run(&jira);
    assert!(b.tasks(&p).is_empty(), "no second item for the same issue");
    assert!(report.notes.iter().any(|n| n.contains(LABEL)), "{:?}", report.notes);

    a.copy_vault_to(&b);
    assert_eq!(b.run(&jira), Report::default());
    assert_eq!(b.tasks(&p).len(), 1);
}

#[test]
fn two_machines_that_import_the_same_issue_make_the_same_file() {
    let jira = FakeJira::standard();
    let key = jira.add("ACME", TASK, "From Jira", "text", BACKLOG);
    let (a, b) = (Machine::new(), Machine::new());
    for m in [&a, &b] {
        let p = m.project("Acme");
        m.link(&p, "ACME");
    }
    // Both poll before either has labelled the issue: B sees it as it was before A's label went on.
    let before = jira.get(&key);
    a.run(&jira);
    jira.w().issues.insert(key.clone(), before);
    b.run(&jira);
    let file = |m: &Machine| m.file("projects/acme/tasks/jira-acme-1.md");
    assert_eq!(file(&a), file(&b));
}

#[test]
fn two_items_claiming_one_issue_leave_one_syncing_and_the_other_marked() {
    let (jira, m, p) = setup();
    let first = m.task(&p, "First", "");
    m.run(&jira);
    let key = m.get_task(&p, &first).jira_issue.unwrap();
    // A copy of the file, say, with the same key in it.
    let second = m.task(&p, "Second", "");
    m.edit(|c| crate::jira_ledger::set_link(c, &p, Kind::Task, &second, Some(&key), None).unwrap());
    let report = m.run(&jira);
    assert_eq!(report.detached, 1);
    assert!(m.get_task(&p, &first).jira_unlinked.is_none());
    assert!(m.get_task(&p, &second).jira_unlinked.as_deref().unwrap().contains("same Jira issue"));
}

// ---- deletes never propagate --------------------------------------------------------------------------

fn assert_detached(m: &Machine, p: &str, id: &str, report: &Report, reason: &str) {
    assert_eq!(report.detached, 1, "{report:?}");
    let task = m.get_task(p, id);
    assert!(task.jira_unlinked.as_deref().is_some_and(|r| r.contains(reason)), "{:?}", task.jira_unlinked);
    assert!(task.jira_issue.is_some(), "the key stays, so it never gets a second issue by accident");
    assert!(m.file(&format!("projects/{p}/tasks/{id}.md")).contains("jiraUnlinked:"));
}

#[test]
fn a_deleted_issue_leaves_its_item_in_place_and_stops_the_sync() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Keep me", "my words");
    m.run(&jira);
    let key = m.get_task(&p, &id).jira_issue.unwrap();
    jira.w().issues.remove(&key);
    let report = m.run(&jira);
    assert_detached(&m, &p, &id, &report, "no longer exists");
    assert_eq!(m.get_task(&p, &id).body, "my words");

    // It isn't made again on Jira, and the next rounds leave it alone.
    assert_eq!(m.run(&jira).issues_created, 0);
    assert!(jira.w().issues.is_empty());
    let view = jira::view(m.root(), &m.ledger.snapshot().unwrap(), Default::default());
    assert_eq!(view.unlinked.len(), 1);
}

#[test]
fn an_issue_moved_to_another_project_or_made_another_type_stops_its_sync() {
    let (jira, m, p) = setup();
    let a = m.task(&p, "Moved", "");
    let b = m.task(&p, "Retyped", "");
    m.run(&jira);
    let (ka, kb) = (m.get_task(&p, &a).jira_issue.unwrap(), m.get_task(&p, &b).jira_issue.unwrap());
    let now = jira.move_to(&ka, "OTHER");
    jira.edit(&kb, |i| {
        i.issue_type = BUG.into();
        i.type_name = "Bug".into();
    });
    let report = m.run(&jira);
    assert_eq!(report.detached, 2);
    assert!(m.get_task(&p, &a).jira_unlinked.unwrap().contains(&now));
    assert!(m.get_task(&p, &b).jira_unlinked.unwrap().contains("Bug"));
}

#[test]
fn deleting_an_item_here_leaves_its_issue_alone_and_it_does_not_come_back() {
    let (jira, m, p) = setup();
    let made = m.task(&p, "Made here", "");
    let brought = jira.add("ACME", TASK, "Brought in", "", BACKLOG);
    m.run(&jira);
    let made_key = m.get_task(&p, &made).jira_issue.unwrap();
    let writes = jira.total_writes();

    m.edit(|c| board::delete_task(c, &p, &made).unwrap());
    m.edit(|c| board::delete_task(c, &p, &format!("jira-{}", brought.to_lowercase())).unwrap());
    m.run(&jira);
    assert!(m.tasks(&p).is_empty(), "neither comes back here");
    assert_eq!(jira.keys("ACME").len(), 2, "and both issues are still on Jira");
    assert_eq!(jira.get(&made_key).summary, "Made here");
    assert_eq!(jira.total_writes(), writes, "untouched");

    // Not on a machine that never knew them either.
    let fresh = Machine::new();
    fresh.project("Acme");
    fresh.link(&p, "ACME");
    fresh.run(&jira);
    assert!(fresh.tasks(&p).is_empty());
}

#[test]
fn an_item_given_a_new_issue_leaves_the_old_one_alone() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Again", "");
    m.run(&jira);
    let old = m.get_task(&p, &id).jira_issue.unwrap();
    jira.w().issues.remove(&old);
    m.run(&jira);

    m.edit(|c| crate::jira_ledger::clear_link(c, &p, Kind::Task, &id).unwrap());
    let report = m.run(&jira);
    assert_eq!(report.issues_created, 1);
    let task = m.get_task(&p, &id);
    assert!(task.jira_unlinked.is_none());
    assert_ne!(task.jira_issue.unwrap(), old);
}

#[test]
fn a_project_that_cannot_be_reached_is_a_note_and_the_others_still_sync() {
    let (jira, m, p) = setup();
    let other = m.project("Gone");
    m.link(&other, "MISSING");
    m.task(&p, "Still syncs", "");
    let report = m.run(&jira);
    assert_eq!(report.issues_created, 1);
    assert!(report.notes.iter().any(|n| n.starts_with("Gone:")), "{:?}", report.notes);
}

#[test]
fn a_failure_on_one_issue_is_a_note_and_the_others_carry_on() {
    let (jira, m, p) = setup();
    let a = m.task(&p, "A", "");
    let b = m.task(&p, "B", "");
    m.run(&jira);
    let ka = m.get_task(&p, &a).jira_issue.unwrap();
    jira.w().fail_update.insert(ka.clone());
    m.update_task(&p, &a, TaskUpdate { title: Some("A2".into()), ..TaskUpdate::default() });
    m.update_task(&p, &b, TaskUpdate { title: Some("B2".into()), ..TaskUpdate::default() });
    let report = m.run(&jira);
    assert!(report.notes.iter().any(|n| n.contains("too long")), "{:?}", report.notes);
    assert_eq!(jira.get(&m.get_task(&p, &b).jira_issue.unwrap()).summary, "B2");

    jira.w().fail_update.clear();
    m.run(&jira);
    assert_eq!(jira.get(&ka).summary, "A2", "and it goes through once Jira takes it");
}

#[test]
fn a_rate_limit_or_a_rejected_token_ends_the_round_and_keeps_what_was_done() {
    let (jira, m, p) = setup();
    m.task(&p, "A task", "");
    jira.w().fail = Some(Fail::RateLimited);
    assert!(matches!(m.try_run(&jira), Err(JiraError::RateLimited { .. })));
    jira.w().fail = Some(Fail::Unauthorized);
    assert!(matches!(m.try_run(&jira), Err(JiraError::Unauthorized)));
    jira.w().fail = None;
    assert_eq!(m.run(&jira).issues_created, 1);
}

// ---- setting up ---------------------------------------------------------------------------------------

#[test]
fn testing_the_connection_names_the_account() {
    assert_eq!(jira::test_connection(&FakeJira::standard()).unwrap(), "Connected to Jira as Ana Smith.");
}

#[test]
fn a_site_is_written_as_its_https_address() {
    assert_eq!(normalize_site("https://acme.atlassian.net/jira/software/projects").as_deref(), Some("https://acme.atlassian.net"));
    assert_eq!(normalize_site("acme.atlassian.net/").as_deref(), Some("https://acme.atlassian.net"));
    assert_eq!(normalize_site("acme").as_deref(), Some("https://acme.atlassian.net"));
    for bad in ["", "http://acme.atlassian.net", "https://user:pw@acme.atlassian.net", "https://acme.atlassian.net:8080", "not a url"] {
        assert_eq!(normalize_site(bad), None, "{bad}");
    }
}

#[test]
fn the_site_email_and_token_are_checked_and_the_secrets_kept_in_the_env_file_only() {
    let m = Machine::new();
    let token = "ATATT3xFfGF0-secret-token-1234";
    assert!(jira::save_credentials(m.root(), Some("acme"), Some("ana@example.com"), None).is_err(), "a token is needed");
    assert!(jira::save_credentials(m.root(), Some("acme"), Some("not an email"), Some(token)).is_err());
    assert!(jira::save_credentials(m.root(), Some("http://acme.atlassian.net"), Some("ana@example.com"), Some(token)).is_err());
    jira::save_credentials(m.root(), Some("acme"), Some("ana@example.com"), Some(token)).unwrap();

    assert_eq!(config::read_settings(m.root()).site, "https://acme.atlassian.net");
    let env = m.file(".ledger/.env");
    assert!(env.contains("JIRA_EMAIL=ana@example.com") && env.contains(&format!("JIRA_API_TOKEN={token}")), "{env}");
    assert!(!m.file(".ledger/jira.json").contains(token));

    // Blank keeps what is saved.
    jira::save_credentials(m.root(), None, None, None).unwrap();
    assert_eq!(config::read_credentials(m.root()).token, token);

    // The screen gets whether there is a token and how it ends, never the token.
    let view = jira::view(m.root(), &m.ledger.snapshot().unwrap(), Default::default());
    assert!(view.has_token && view.token_hint == "…1234" && view.email == "ana@example.com");
    assert!(!serde_json::to_string(&view).unwrap().contains(token));

    // Removed again: the site and links stay, the secrets go.
    jira::disconnect(m.root()).unwrap();
    let creds = config::read_credentials(m.root());
    assert!(creds.email.is_empty() && creds.token.is_empty());
    assert!(!m.file(".ledger/.env").contains(token));
    assert_eq!(config::read_settings(m.root()).site, "https://acme.atlassian.net");
}

#[test]
fn the_token_never_appears_anywhere_but_the_env_file() {
    let (jira, m, p) = setup();
    let token = "ATATT3xFfGF0-never-elsewhere-9876";
    jira::save_credentials(m.root(), Some("acme"), Some("ana@example.com"), Some(token)).unwrap();
    m.task(&p, "A task", "");
    jira.add("ACME", TASK, "From Jira", "", BACKLOG);
    m.run(&jira);
    fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), out);
            } else {
                out.push((entry.path().display().to_string(), String::from_utf8_lossy(&fs::read(entry.path()).unwrap()).to_string()));
            }
        }
    }
    let mut files = vec![];
    walk(m.root(), &mut files);
    for (path, text) in files {
        if path.replace('\\', "/").ends_with("/.ledger/.env") {
            assert!(text.contains(token));
        } else {
            assert!(!text.contains(token), "the token is in {path}");
        }
    }
    assert!(crate::sync::Ignore::load(m.root()).is_ignored(".ledger/jira.json", false), "and the GitHub sync never picks up the settings");
}

#[test]
fn a_project_is_linked_to_a_jira_project_and_a_jira_project_serves_one_project() {
    let (jira, m, p) = setup();
    let other = m.project("Other work");
    assert!(jira::link_project(m.root(), &jira, &other, Some("NOPE")).unwrap_err().to_string().contains("can see"));
    let error = jira::link_project(m.root(), &jira, &other, Some("ACME")).unwrap_err().to_string();
    assert!(error.contains("already linked") && error.contains(&p), "{error}");
    jira::link_project(m.root(), &jira, &other, Some("OTHER")).unwrap();
    let link = config::read_settings(m.root()).projects[&other].clone();
    assert_eq!((link.project.as_str(), link.project_name.as_str()), ("OTHER", "Other"));

    // Linking to another Jira project stops what was synced with the old one.
    m.task(&p, "On ACME", "");
    m.run(&jira);
    assert_eq!(config::read_state(m.root()).items.len(), 1);
    jira::link_project(m.root(), &jira, &p, None).unwrap();
    assert!(config::read_state(m.root()).items.is_empty());
    assert!(!config::read_settings(m.root()).projects.contains_key(&p));
}

#[test]
fn the_sync_is_on_only_when_there_is_something_to_sync() {
    let m = Machine::new();
    let p = m.project("Acme");
    assert!(!config::is_configured(m.root()));
    jira::save_credentials(m.root(), Some("acme"), Some("ana@example.com"), Some("token-1234567890")).unwrap();
    assert!(!config::is_configured(m.root()), "no project is linked yet");
    m.link(&p, "ACME");
    assert!(config::is_configured(m.root()));
    jira::disconnect(m.root()).unwrap();
    assert!(!config::is_configured(m.root()));
}

#[test]
fn the_screen_is_told_which_status_each_column_uses() {
    let (jira, m, p) = setup();
    let columns = m.read(|c| board::columns(c, &p).unwrap());
    let link = config::read_settings(m.root()).projects[&p].clone();
    let mapping = jira::mapping_view(&columns, &jira, &link).unwrap();
    let uses = |mapping: &jira::ProjectMapping| mapping.columns.iter().map(|c| (c.name.clone(), c.status_name.clone())).collect::<Vec<_>>();
    assert_eq!(uses(&mapping), [("Backlog".into(), Some("Backlog".into())), ("Doing".into(), Some("Doing".into())), ("Done".into(), Some("Done".into()))]);
    assert_eq!(mapping.unused_statuses, ["Review"]);
    assert!(mapping.missing_types.is_empty());

    jira::set_column_status(m.root(), &p, "doing", REVIEW).unwrap();
    jira::set_column_status(m.root(), &p, "done", "none").unwrap();
    let link = config::read_settings(m.root()).projects[&p].clone();
    let mapping = jira::mapping_view(&columns, &jira, &link).unwrap();
    assert_eq!(uses(&mapping)[1..], [("Doing".into(), Some("Review".into())), ("Done".into(), None)]);
    assert_eq!(mapping.columns[2].choice, "none");
    jira::set_column_status(m.root(), &p, "done", "auto").unwrap();
    assert!(!config::read_settings(m.root()).projects[&p].columns.contains_key("done"));
}

// ---- the client: rate limits, retries, credentials ----------------------------------------------------

struct Scripted {
    replies: Mutex<VecDeque<std::result::Result<Reply, String>>>,
    seen: Mutex<Vec<Request>>,
}

struct Shared(Arc<Scripted>);

impl Wire for Shared {
    fn send(&self, request: &Request) -> std::result::Result<Reply, String> {
        self.0.seen.lock().unwrap().push(request.clone());
        self.0.replies.lock().unwrap().pop_front().unwrap_or_else(|| Err("the script ran out".into()))
    }
}

fn reply(status: u16, body: &str) -> std::result::Result<Reply, String> {
    Ok(Reply { status, retry_after: None, body: body.into() })
}

fn limited(status: u16, seconds: Option<u64>) -> std::result::Result<Reply, String> {
    Ok(Reply { status, retry_after: seconds.map(Duration::from_secs), body: "Rate limit exceeded".into() })
}

/// A client over scripted replies; its pauses are recorded instead of taken.
fn scripted(replies: Vec<std::result::Result<Reply, String>>) -> (HttpJira<Shared>, Arc<Scripted>, Arc<Mutex<Vec<Duration>>>) {
    let script = Arc::new(Scripted { replies: Mutex::new(replies.into()), seen: Mutex::new(vec![]) });
    let pauses = Arc::new(Mutex::new(vec![]));
    let record = pauses.clone();
    let client = HttpJira::with_wire(Shared(script.clone()), Arc::new(move |d| record.lock().unwrap().push(d)));
    (client, script, pauses)
}

const ME: &str = r#"{"accountId":"a1","displayName":"Ana Smith","emailAddress":"ana@example.com"}"#;

fn long_pauses(pauses: &Mutex<Vec<Duration>>) -> Vec<Duration> {
    pauses.lock().unwrap().iter().copied().filter(|d| *d >= Duration::from_secs(1)).collect()
}

#[test]
fn a_rate_limited_request_waits_as_long_as_jira_says_and_is_tried_again() {
    let (client, script, pauses) = scripted(vec![limited(429, Some(3)), limited(503, Some(2)), reply(200, ME)]);
    assert_eq!(client.myself().unwrap().display_name, "Ana Smith");
    assert_eq!(script.seen.lock().unwrap().len(), 3);
    assert_eq!(long_pauses(&pauses), [Duration::from_secs(3), Duration::from_secs(2)]);
}

#[test]
fn without_a_retry_after_the_wait_grows_and_in_the_end_the_caller_is_told_to_slow_down() {
    let (client, script, pauses) = scripted((0..5).map(|_| limited(429, None)).collect());
    let error = client.myself().unwrap_err();
    assert!(matches!(error, JiraError::RateLimited { .. }), "{error}");
    assert!(error.to_string().contains("rate limiting"));
    assert_eq!(script.seen.lock().unwrap().len(), 5);
    assert_eq!(long_pauses(&pauses), [1, 2, 4, 8].map(Duration::from_secs));

    let (client, script, _) = scripted(vec![limited(429, Some(300)), reply(200, ME)]);
    assert!(matches!(client.myself().unwrap_err(), JiraError::RateLimited { retry_after } if retry_after == Duration::from_secs(300)));
    assert_eq!(script.seen.lock().unwrap().len(), 1, "a long wait is passed on instead of blocking for it");
}

#[test]
fn server_errors_and_dropped_connections_are_retried_with_a_growing_pause() {
    let (client, _, pauses) = scripted(vec![reply(503, ""), Err("Could not reach Jira: connection reset".into()), reply(502, ""), reply(200, ME)]);
    assert_eq!(client.myself().unwrap().account_id, "a1");
    assert_eq!(long_pauses(&pauses), [1, 2, 4].map(Duration::from_secs));

    let (client, script, _) = scripted((0..5).map(|_| reply(500, "")).collect());
    assert!(client.myself().unwrap_err().to_string().contains("try again"));
    assert_eq!(script.seen.lock().unwrap().len(), 5);
}

#[test]
fn an_account_that_hides_its_name_and_email_still_connects() {
    let (client, _, _) = scripted(vec![reply(200, r#"{"accountId":"a1","displayName":null,"emailAddress":null}"#), reply(200, "{}")]);
    let me = client.myself().unwrap();
    assert_eq!((me.account_id.as_str(), me.display_name.as_str(), me.email_address.as_str()), ("a1", "", ""));
    assert!(client.myself().unwrap_err().to_string().contains("not what was expected"));
}

#[test]
fn a_refused_token_is_not_retried_and_other_refusals_say_what_jira_said() {
    let (client, script, _) = scripted(vec![reply(401, "Unauthorized"), reply(200, ME)]);
    assert!(matches!(client.myself().unwrap_err(), JiraError::Unauthorized));
    assert_eq!(script.seen.lock().unwrap().len(), 1);
    let (client, _, _) = scripted(vec![reply(400, r#"{"errorMessages":["Field 'x' is wrong"],"errors":{"summary":"too long"}}"#)]);
    let error = client.myself().unwrap_err().to_string();
    assert!(error.contains("Field 'x' is wrong") && error.contains("summary: too long"), "{error}");
    let (client, _, _) = scripted(vec![reply(403, "")]);
    assert!(client.myself().unwrap_err().to_string().contains("permissions"));
}

#[test]
fn issues_are_read_page_by_page_and_a_missing_one_is_none() {
    let issue = |key: &str| {
        json!({ "key": key, "fields": {
            "summary": format!("Issue {key}"), "description": to_adf("text"), "status": { "id": "3", "name": "Done" },
            "issuetype": { "id": TASK, "name": "Task" }, "project": { "key": "ACME" }, "labels": ["ledger"],
            "created": "2026-11-03T10:23:05.123+0100", "updated": "2026-11-03T11:00:00.000+0000",
        }})
    };
    let first = json!({ "issues": [issue("ACME-1"), issue("ACME-2")], "nextPageToken": "page2" }).to_string();
    let second = json!({ "issues": [issue("ACME-3")], "isLast": true }).to_string();
    let (client, script, _) = scripted(vec![reply(200, &first), reply(200, &second), reply(404, r#"{"errorMessages":["Issue does not exist"]}"#)]);
    let issues = client.issues("ACME", &[TASK.to_string(), STORY.to_string()]).unwrap();
    assert_eq!(issues.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), ["ACME-1", "ACME-2", "ACME-3"]);
    let one = &issues[0];
    assert_eq!((one.status.as_str(), one.issue_type.as_str(), one.project.as_str(), one.labels.as_slice()), ("3", TASK, "ACME", &["ledger".to_string()][..]));
    assert_eq!((one.created.as_str(), one.updated.as_str()), ("2026-11-03T09:23:05.123Z", "2026-11-03T11:00:00.000Z"));
    assert_eq!(client.issue("ACME-9").unwrap(), None);

    let seen = script.seen.lock().unwrap();
    assert_eq!((seen[0].method, seen[0].path.as_str()), ("POST", "/search/jql"));
    let body = seen[0].body.as_ref().unwrap();
    assert_eq!(body["jql"], format!("project = \"ACME\" AND issuetype in (\"{TASK}\", \"{STORY}\") ORDER BY key"));
    assert_eq!(seen[1].body.as_ref().unwrap()["nextPageToken"], "page2");
    assert_eq!(seen[2].path, "/issue/ACME-9");
}

#[test]
fn requests_carry_no_credentials_and_keys_cannot_reach_another_endpoint() {
    let (client, script, pauses) = scripted(vec![reply(201, r#"{"id":"1","key":"ACME-1"}"#), reply(204, ""), reply(204, "")]);
    let key = client
        .create_issue(&NewIssue { project: "ACME".into(), issue_type: TASK.into(), summary: "New".into(), description: to_adf("d"), labels: vec![LABEL.into()] })
        .unwrap();
    assert_eq!(key, "ACME-1");
    client.update_issue("ACME-1", &IssueUpdate { summary: Some("Changed".into()), add_label: Some(LABEL.into()), ..IssueUpdate::default() }).unwrap();
    client.update_issue("../../myself?x=", &IssueUpdate { description: Some(None), ..IssueUpdate::default() }).unwrap();
    assert!(pauses.lock().unwrap().iter().filter(|d| **d <= MIN_GAP && !d.is_zero()).count() >= 1);

    let seen = script.seen.lock().unwrap();
    let create = seen[0].body.as_ref().unwrap();
    assert_eq!((create["fields"]["project"]["key"].as_str(), create["fields"]["issuetype"]["id"].as_str()), (Some("ACME"), Some(TASK)));
    assert_eq!(create["fields"]["labels"], json!([LABEL]));
    // A change sends only what changed; a label is added, never set.
    assert_eq!(seen[1].body.as_ref().unwrap(), &json!({ "fields": { "summary": "Changed" }, "update": { "labels": [{ "add": LABEL }] } }));
    assert_eq!(seen[2].path, "/issue/%2E%2E%2F%2E%2E%2Fmyself%3Fx%3D");
    assert_eq!(seen[2].body.as_ref().unwrap(), &json!({ "fields": { "description": null } }));
    for request in seen.iter() {
        let text = format!("{request:?}").to_lowercase();
        assert!(!text.contains("basic") && !text.contains("token") && !text.contains("ana@"), "{text}");
    }
}

#[test]
fn credentials_go_in_a_basic_authorization_header_and_a_bad_one_is_refused_without_repeating_it() {
    assert_eq!(auth_header("ana@example.com", "tok"), "Basic YW5hQGV4YW1wbGUuY29tOnRvaw==");
    let error = match JiraWire::new("acme", "ana@example.com", "bad token\nSECRETPART") {
        Ok(_) => panic!("a token with a newline can't be sent"),
        Err(e) => e.to_string(),
    };
    assert!(!error.contains("SECRETPART"), "{error}");
    assert!(JiraWire::new("acme", "ana@example.com", "TOKEN123").is_ok());
}

// ---- the vault keeps the issue key --------------------------------------------------------------------

#[test]
fn the_issue_key_and_the_reason_survive_being_written_and_read_back() {
    let (jira, m, p) = setup();
    let id = m.task(&p, "Keep the key", "");
    let story = m.story(&p, story_draft("MM01", "Story"));
    m.run(&jira);
    m.edit(|c| crate::jira_ledger::set_link(c, &p, Kind::Story, &story, Some("ACME-2"), Some("Gone on Jira.")).unwrap());
    m.reload();
    assert_eq!(m.get_task(&p, &id).jira_issue.as_deref(), Some("ACME-1"));
    let story = m.get_story(&p, &story);
    assert_eq!((story.jira_issue.as_deref(), story.jira_unlinked.as_deref()), (Some("ACME-2"), Some("Gone on Jira.")));

    // A file from before the key existed loads as an item with no issue, and is written back unchanged.
    let plain = Machine::new();
    let pp = plain.project("Plain");
    let tid = plain.task(&pp, "Old", "body");
    let file = format!("projects/{pp}/tasks/{tid}.md");
    let before = plain.file(&file);
    assert!(!before.contains("jira"));
    plain.reload();
    assert_eq!(plain.get_task(&pp, &tid).jira_issue, None);
    plain.edit(|_| ());
    assert_eq!(plain.file(&file), before);
}

// ---- the worker ---------------------------------------------------------------------------------------

fn quick() -> Timing {
    Timing {
        debounce: Duration::from_millis(150),
        max_debounce: Duration::from_millis(900),
        poll: Duration::from_secs(60),
        retry: Duration::from_millis(80),
        rate_limited: Duration::from_millis(400),
    }
}

struct Worker {
    syncer: jira::JiraSyncer,
    vault_changes: Arc<AtomicUsize>,
}

fn start_worker(m: &Machine, fake: &Arc<FakeJira>, timing: Timing) -> Worker {
    let vault_changes = Arc::new(AtomicUsize::new(0));
    let changes = vault_changes.clone();
    let emit: Emit = Arc::new(move |event| {
        if let JiraEvent::VaultChanged = event {
            changes.fetch_add(1, Ordering::SeqCst);
        }
    });
    let fake = fake.clone();
    let connect: Connect = Arc::new(move |_, _| Ok(Box::new(fake.clone())));
    Worker { syncer: jira::spawn(m.core.clone(), emit, m.ledger.changed.clone(), connect, timing), vault_changes }
}

fn wait_for(what: &str, condition: impl Fn() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(start.elapsed() < Duration::from_secs(10), "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn configured_machine() -> (Arc<FakeJira>, Machine, String) {
    let (jira, m, p) = setup();
    jira::save_credentials(m.root(), Some("acme"), Some("ana@example.com"), Some("token-1234567890")).unwrap();
    (Arc::new(jira), m, p)
}

#[test]
fn the_worker_syncs_on_request_and_after_changes_and_says_what_it_did() {
    let (jira, m, p) = configured_machine();
    jira.add("ACME", TASK, "From Jira", "", BACKLOG);
    let worker = start_worker(&m, &jira, quick());
    worker.syncer.now();
    wait_for("the round", || worker.syncer.status().last_sync_at.is_some());
    let status = worker.syncer.status();
    assert_eq!(status.state, "idle");
    assert_eq!(status.summary.as_deref(), Some("Added 1 item from Jira"));
    assert_eq!(worker.vault_changes.load(Ordering::SeqCst), 1);

    m.task(&p, "Made here", "");
    worker.syncer.dirty();
    wait_for("the change to go up", || jira.keys("ACME").len() == 2);
}

#[test]
fn a_rate_limit_shows_in_the_status_and_makes_the_worker_wait_longer_than_a_plain_failure() {
    let (jira, m, p) = configured_machine();
    m.task(&p, "A task", "");
    jira.w().fail = Some(Fail::RateLimited);
    let worker = start_worker(&m, &jira, quick());
    worker.syncer.now();
    wait_for("the failure", || worker.syncer.status().state == "error");
    assert!(worker.syncer.status().last_error.unwrap().contains("rate limiting"));

    wait_for("the second try", || jira.calls("issue_types") >= 2);
    let times = jira.call_times("issue_types");
    assert!(times[1].duration_since(times[0]) >= Duration::from_millis(390), "{:?}", times[1].duration_since(times[0]));

    jira.w().fail = None;
    wait_for("the sync to recover", || worker.syncer.status().state == "idle" && worker.syncer.status().last_sync_at.is_some());
    assert_eq!(worker.syncer.status().last_error, None);
    assert_eq!(jira.keys("ACME").len(), 1);
}

// ---- the real wire, against a server on this machine ----------------------------------------------------

#[test]
fn the_real_client_sends_credentials_only_in_the_authorization_header() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = vec![];
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                break;
            }
            head.push(line.trim_end().to_string());
        }
        let length = head.iter().find_map(|h| h.to_lowercase().strip_prefix("content-length:").map(|n| n.trim().parse::<usize>().unwrap())).unwrap_or(0);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        let answer = r#"{"accountId":"a1","displayName":"Ana Smith"}"#;
        write!(&stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len()).unwrap();
        (head, String::from_utf8(body).unwrap())
    });

    // The only test that sets it, so no other test is pointed elsewhere.
    unsafe { std::env::set_var("LEDGER_JIRA_API", format!("http://127.0.0.1:{port}/rest/api/3")) };
    let client = HttpJira::new("acme", "ana@example.com", "SECRET-TOKEN-42").unwrap();
    unsafe { std::env::remove_var("LEDGER_JIRA_API") };
    assert_eq!(jira::test_connection(&client).unwrap(), "Connected to Jira as Ana Smith.");

    let (head, body) = server.join().unwrap();
    assert_eq!(head[0], "GET /rest/api/3/myself HTTP/1.1");
    let expected = format!("authorization: {}", auth_header("ana@example.com", "SECRET-TOKEN-42"));
    assert!(head.iter().any(|h| h.to_lowercase() == expected.to_lowercase()), "{head:?}");
    let elsewhere: Vec<&String> = head.iter().filter(|h| !h.to_lowercase().starts_with("authorization:")).collect();
    let secret = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "ana@example.com:SECRET-TOKEN-42");
    for line in elsewhere.iter().map(|l| l.as_str()).chain([body.as_str()]) {
        assert!(!line.contains("SECRET-TOKEN-42") && !line.contains(&secret) && !line.contains("ana@example.com"), "{line}");
    }
}
