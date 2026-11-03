//! The two-way sync between a project's tasks and user stories and the issues of a Jira project.
//!
//! * **Scope.** Each Ledger project is linked to one Jira project. Its issues of type Story and Task are synced;
//!   other types (bugs, epics, subtasks) are never read or written.
//! * **What syncs.** A user story ↔ a Story: the summary, and a description holding "As a / I want to / Such that",
//!   the acceptance criteria and the notes, in the format the Trello sync uses (see `trello_card.rs`). A task ↔ a
//!   Task: the title, the body as its description, and its column as the issue's status. Descriptions are
//!   converted between markdown and ADF (`jira_adf.rs`).
//! * **Identity.** An item and its issue are tied together by the issue key in the item's own frontmatter
//!   (`jiraIssue`), which the GitHub sync carries to other machines. An issue with no item here becomes an item
//!   whose id comes from the key (`jira-<key>` for a task, the key itself for a story), so two machines importing
//!   the same issue make the same file.
//! * **The `ledger` label.** Every issue that has an item carries it: Ledger adds it to the issues it makes and to
//!   the ones it brings in. An issue with the label and no item here is never imported. It belongs to an item that
//!   is still on its way from another machine, or that was deleted, and so a deleted item stays deleted on every
//!   machine. Take the label off an issue to bring it in again.
//! * **Changes.** For every linked pair the title, the description and (for a task) the status are compared with
//!   what both sides agreed on when they were last in step (`.ledger/jira-sync.json`). A field only one side changed
//!   goes to the other, field by field. When both changed the same field, this machine's version wins and Jira's is
//!   kept in `.ledger/conflicts/<time>/jira/`.
//! * **Descriptions Ledger can't write.** A description with pictures, tables, mentions or colours is read as text
//!   but never written back, since that would remove them: an edit made here stays here, with a note.
//! * **Statuses.** A column takes the status with the same name (ignoring case) unless the project's link says
//!   otherwise. Moving a task moves its issue through a transition of the workflow; when none leads there it is a
//!   note, and the next round tries again.
//! * **Deletes never propagate.** An issue that is deleted, moved to another project or turned into another type
//!   leaves its item in place, marked as no longer linked (`jiraUnlinked`, with the reason). An item deleted here
//!   leaves its issue alone. Nothing is ever deleted on Jira.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use serde::Serialize;
use serde_yaml_ng::Mapping as Meta;

use crate::error::Error;
use crate::jira_adf::{self, canonical, to_adf};
use crate::jira_api::{Issue, IssueType, IssueUpdate, JResult, Jira, JiraError, NewIssue, Status};
use crate::jira_config::*;
use crate::jira_ledger::{Change, Ledger};
use crate::models::{Column, StoryView, TaskView};
use crate::trello_card::{Kind, StoryText, norm, parse_story, parse_story_marked, render_story, same_name, story_text};
use crate::trello_ledger::{Applied, ProjectData, Snapshot};
use crate::{frontmatter, ids};

pub const LABEL: &str = "ledger";
const STORY_TYPE: &str = "Story";
const TASK_TYPE: &str = "Task";

// ---- what is compared ---------------------------------------------------------------------------------

/// A story's description as markdown from Jira, read as a story. The criteria are a list, which comes back from
/// Jira a blank line below their heading; [`parse_story`] wants it right under it.
pub fn story_from_jira(markdown: &str) -> StoryText {
    parse_story(&join_criteria(markdown))
}

fn join_criteria(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut out: Vec<&str> = vec![];
    for (i, line) in lines.iter().enumerate() {
        let after_heading = i > 0 && same_name(lines[i - 1], "**Acceptance criteria**");
        let list_follows = lines.get(i + 1).is_some_and(|next| next.trim_start().starts_with(['-', '*', '+']));
        if after_heading && line.trim().is_empty() && list_follows {
            continue;
        }
        out.push(line);
    }
    out.join("\n")
}

/// A story description in the form the two sides are compared in.
fn story_desc(markdown: &str) -> String {
    canonical(&render_story(&story_from_jira(&canonical(markdown))))
}

pub fn task_content(task: &TaskView, status: Option<&str>) -> Content {
    Content { title: norm(&task.title), desc: canonical(&task.body), status: status.map(str::to_string) }
}

pub fn story_content(story: &StoryView) -> Content {
    Content { title: norm(&story.summary), desc: story_desc(&render_story(&story_text(story))), status: None }
}

/// The issue as a task or story would hold it, and whether its description says nothing Ledger can't.
pub fn issue_content(issue: &Issue, kind: Kind) -> (Content, bool) {
    let (desc, lossless) = jira_adf::from_jira(issue.description.as_ref());
    let content = match kind {
        Kind::Task => Content { title: norm(&issue.summary), desc, status: Some(issue.status.clone()) },
        Kind::Story => Content { title: norm(&issue.summary), desc: story_desc(&desc), status: None },
    };
    (content, lossless)
}

// ---- columns and statuses -----------------------------------------------------------------------------

/// Which Jira status belongs to which column of a project's board.
#[derive(Debug, Default, Clone)]
pub struct Mapping {
    by_column: HashMap<String, String>,
    by_status: HashMap<String, String>,
}

impl Mapping {
    /// A column the project's link names a status for uses that status (an empty one means "don't sync this
    /// column"); every other column takes the status with the same name, ignoring case. A status belongs to one
    /// column.
    pub fn new(columns: &[Column], statuses: &[Status], overrides: &BTreeMap<String, String>) -> Mapping {
        let mut map = Mapping::default();
        let mut chosen: HashSet<&str> = HashSet::new();
        for column in columns {
            let Some(choice) = overrides.get(&column.id) else { continue };
            chosen.insert(column.id.as_str());
            if !choice.is_empty() && statuses.iter().any(|s| s.id == *choice) && !map.by_status.contains_key(choice) {
                map.by_column.insert(column.id.clone(), choice.clone());
                map.by_status.insert(choice.clone(), column.id.clone());
            }
        }
        for column in columns.iter().filter(|c| !chosen.contains(c.id.as_str())) {
            if let Some(status) = statuses.iter().find(|s| !map.by_status.contains_key(&s.id) && same_name(&s.name, &column.name)) {
                map.by_column.insert(column.id.clone(), status.id.clone());
                map.by_status.insert(status.id.clone(), column.id.clone());
            }
        }
        map
    }

    pub fn status_for(&self, column: &str) -> Option<&str> {
        self.by_column.get(column).map(String::as_str)
    }

    pub fn column_for(&self, status: &str) -> Option<&str> {
        self.by_status.get(status).map(String::as_str)
    }
}

/// The project's Story and Task issue types, found by name.
pub fn kind_types(types: &[IssueType]) -> (Option<&IssueType>, Option<&IssueType>) {
    let find = |name: &str| types.iter().find(|t| !t.subtask && same_name(&t.name, name));
    (find(STORY_TYPE), find(TASK_TYPE))
}

// ---- what a round did ---------------------------------------------------------------------------------

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub issues_created: usize,
    pub issues_updated: usize,
    pub items_created: usize,
    pub items_updated: usize,
    /// Items that just stopped syncing because their issue is gone, moved or of another type now.
    pub detached: usize,
    pub conflicts: usize,
    /// Tasks without a status and issues without a column: left out, and said so.
    pub unmatched: usize,
    /// Changes held back because the item was edited while the round ran; the next round does them.
    pub skipped: usize,
    /// Items that are not syncing (now or earlier) because their issue is gone.
    pub unlinked: usize,
    /// How many changes were written into the vault.
    pub vault_writes: usize,
    pub notes: Vec<String>,
}

impl Report {
    pub fn vault_changed(&self) -> bool {
        self.vault_writes > 0
    }

    pub fn summary(&self) -> String {
        let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut parts = vec![];
        if self.issues_created > 0 {
            parts.push(format!("created {}", plural(self.issues_created, "issue", "issues")));
        }
        if self.issues_updated > 0 {
            parts.push(format!("updated {}", plural(self.issues_updated, "issue", "issues")));
        }
        if self.items_created > 0 {
            parts.push(format!("added {} from Jira", plural(self.items_created, "item", "items")));
        }
        if self.items_updated > 0 {
            parts.push(format!("updated {} from Jira", plural(self.items_updated, "item", "items")));
        }
        if self.detached > 0 {
            parts.push(format!("stopped syncing {}", plural(self.detached, "item", "items")));
        }
        if self.conflicts > 0 {
            parts.push(format!("{} (Jira's version was saved)", plural(self.conflicts, "conflict", "conflicts")));
        }
        if self.unmatched > 0 {
            parts.push(format!("{} left out (no matching column or status)", self.unmatched));
        }
        if self.skipped > 0 {
            parts.push(format!("{} to do next time", self.skipped));
        }
        if parts.is_empty() {
            return "Up to date".to_string();
        }
        let text = parts.join(", ");
        let mut chars = text.chars();
        chars.next().map_or_else(String::new, |c| c.to_uppercase().collect::<String>() + chars.as_str())
    }
}

// ---- the round ----------------------------------------------------------------------------------------

/// Something that changes only when a Jira sync would have something to do: the part of the snapshot and of the
/// settings that a round looks at. The background worker skips a round that follows an edit of anything else.
pub fn digest(snapshot: &Snapshot, settings: &JiraSettings) -> u64 {
    let mut hasher = DefaultHasher::new();
    settings.site.hash(&mut hasher);
    for data in &snapshot.projects {
        let Some(link) = settings.projects.get(&data.slug) else { continue };
        (&data.slug, &link.project, &link.columns).hash(&mut hasher);
        for column in &data.columns {
            (&column.id, &column.name).hash(&mut hasher);
        }
        for t in &data.tasks {
            (&t.id, &t.title, &t.body, &t.column_id, &t.jira_issue, &t.jira_unlinked, &t.updated_at).hash(&mut hasher);
        }
        for s in &data.stories {
            (&s.id, &s.summary, &s.role, &s.want, &s.reason, &s.notes, &s.jira_issue, &s.jira_unlinked, &s.updated_at).hash(&mut hasher);
            for c in &s.criteria {
                (&c.text, c.done).hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

/// One round: reconcile every linked project with its Jira project. `snapshot` is the vault as it was read for
/// this round. Progress is kept even when the round stops early (the rate limit, a rejected token).
pub fn run(root: &Path, jira: &dyn Jira, ledger: &dyn Ledger, settings: &JiraSettings, snapshot: &Snapshot) -> JResult<Report> {
    let mut run = Run { root, jira, ledger, state: read_state(root), report: Report::default(), stamp: ids::now().replace(':', "-"), claimed: HashSet::new() };
    let result = run.all(settings, snapshot);
    write_state(root, &run.state)?;
    result?;
    Ok(run.report)
}

struct Run<'a> {
    root: &'a Path,
    jira: &'a dyn Jira,
    ledger: &'a dyn Ledger,
    state: SyncState,
    report: Report,
    /// The folder name, under `.ledger/conflicts/`, for what this round sets aside.
    stamp: String,
    /// Every issue key some item in the vault is linked to, in capitals.
    claimed: HashSet<String>,
}

#[derive(Clone, Copy)]
enum Count {
    Nothing,
    ItemCreated,
    ItemUpdated,
    Detached,
}

/// A change to the vault, and what to do once it is made.
struct Step {
    change: Change,
    label: String,
    track: Option<(String, Tracked)>,
    untrack: Option<String>,
    /// An issue just brought in: it gets the label once its item exists.
    mark: Option<String>,
    count: Count,
}

impl Step {
    fn new(change: Change, label: String, count: Count) -> Step {
        Step { change, label, track: None, untrack: None, mark: None, count }
    }
}

/// An item as the vault holds it.
struct Local {
    kind: Kind,
    id: String,
    key: String,
    updated_at: String,
    content: Content,
    /// What goes in a note about the item.
    title: String,
    /// The description as the vault holds it (the body, or the story's whole text), to set aside if it loses.
    text: String,
}

/// One project's issues as this round sees them.
struct Board<'a> {
    project: &'a str,
    mapping: Mapping,
    story_type: Option<&'a IssueType>,
    task_type: Option<&'a IssueType>,
    issues: HashMap<String, &'a Issue>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fate {
    Agree,
    /// The vault's version goes to Jira.
    Push,
    /// Jira's version goes to the vault.
    Pull,
}

/// Three-way: what each side has now against what they last agreed on. When both changed it differently, this
/// machine wins; the flag says that was the case.
fn pick<T: PartialEq>(ledger: &T, jira: &T, base: Option<&T>) -> (Fate, bool) {
    if ledger == jira {
        (Fate::Agree, false)
    } else if base == Some(ledger) {
        (Fate::Pull, false)
    } else if base == Some(jira) {
        (Fate::Push, false)
    } else {
        (Fate::Push, true)
    }
}

#[derive(Default)]
struct Merge {
    /// What both sides hold once the merge is done: the next base.
    content: Content,
    push: IssueUpdate,
    push_status: Option<String>,
    pull_title: Option<String>,
    pull_desc: Option<String>,
    pull_status: Option<String>,
    /// A field both sides had changed: Jira's version needs setting aside (or, for a description that can't be
    /// written back, this side's).
    jira_lost: bool,
    ledger_lost: bool,
    /// This side's description changed, and Jira's can't take it.
    held_back: bool,
    unmatched: bool,
}

impl Merge {
    fn pulls(&self) -> bool {
        self.pull_title.is_some() || self.pull_desc.is_some() || self.pull_status.is_some()
    }
}

fn merge(kind: Kind, l: &Content, j: &Content, b: Option<&Content>, lossless: bool, mapping: &Mapping) -> Merge {
    let mut m = Merge::default();

    let (fate, conflict) = pick(&l.title, &j.title, b.map(|b| &b.title));
    m.content.title = if fate == Fate::Pull { j.title.clone() } else { l.title.clone() };
    match fate {
        Fate::Push => m.push.summary = Some(l.title.clone()),
        Fate::Pull => m.pull_title = Some(j.title.clone()),
        Fate::Agree => {}
    }
    m.jira_lost |= conflict;

    let (fate, conflict) = pick(&l.desc, &j.desc, b.map(|b| &b.desc));
    match fate {
        Fate::Agree => m.content.desc = l.desc.clone(),
        Fate::Push if lossless => {
            m.push.description = Some(to_adf(&l.desc));
            m.content.desc = l.desc.clone();
            m.jira_lost |= conflict;
        }
        // Writing it would remove what Jira's description has and markdown here can't say.
        Fate::Push if conflict => {
            m.pull_desc = Some(j.desc.clone());
            m.content.desc = j.desc.clone();
            m.ledger_lost = true;
        }
        Fate::Push => {
            m.held_back = true;
            m.content.desc = b.map_or_else(|| j.desc.clone(), |b| b.desc.clone());
        }
        Fate::Pull => {
            m.pull_desc = Some(j.desc.clone());
            m.content.desc = j.desc.clone();
        }
    }

    if kind == Kind::Task {
        m.content.status = j.status.clone();
        match (&l.status, &j.status) {
            (Some(ours), Some(theirs)) => match pick(ours, theirs, b.and_then(|b| b.status.as_ref())).0 {
                Fate::Agree => {}
                Fate::Push => m.push_status = Some(ours.clone()),
                Fate::Pull if mapping.column_for(theirs).is_some() => m.pull_status = Some(theirs.clone()),
                // The issue has a status no column uses: the task stays where it is.
                Fate::Pull => {
                    m.unmatched = true;
                    m.content.status = Some(ours.clone());
                }
            },
            (None, _) => m.unmatched = true,
            _ => {}
        }
    }
    m
}

impl Run<'_> {
    fn all(&mut self, settings: &JiraSettings, snapshot: &Snapshot) -> JResult<()> {
        let duplicates = self.take_stock(snapshot);
        let mut worked = 0;
        let mut first_failure: Option<JiraError> = None;
        for data in &snapshot.projects {
            let Some(link) = settings.projects.get(&data.slug).filter(|l| !l.project.is_empty()) else { continue };
            match self.project(data, link, &duplicates) {
                Ok(()) => worked += 1,
                Err(e @ (JiraError::RateLimited { .. } | JiraError::Unauthorized | JiraError::Ledger(_))) => return Err(e),
                Err(e) => {
                    // One project that can't be reached is a note, and the others carry on.
                    let failure = format!("{}: {e}", data.name);
                    self.report.notes.push(failure.clone());
                    first_failure.get_or_insert(JiraError::Message(failure));
                }
            }
            write_state(self.root, &self.state)?;
        }
        // If none could be reached, the round has failed (and is tried again soon) rather than being "up to date".
        match first_failure {
            Some(failure) if worked == 0 => Err(failure),
            _ => Ok(()),
        }
    }

    /// Looks over the whole vault before any project is touched: which issues are claimed, which links have lost
    /// their item, which items claim an issue another item already has. Returns the items that must stop syncing
    /// because of the last.
    fn take_stock(&mut self, snapshot: &Snapshot) -> HashSet<(String, Kind, String)> {
        // (project, kind, id) → the item's issue and whether it is detached.
        let mut live: HashMap<(String, Kind, String), (Option<String>, bool)> = HashMap::new();
        let mut claims: HashMap<String, Vec<(String, Kind, String, String)>> = HashMap::new();
        let mut note = |project: &str, kind: Kind, id: &str, issue: &Option<String>, unlinked: &Option<String>, created_at: &str, report: &mut Report| {
            live.insert((project.to_string(), kind, id.to_lowercase()), (issue.as_ref().map(|k| k.to_uppercase()), unlinked.is_some()));
            if unlinked.is_some() {
                report.unlinked += 1;
            }
            if let Some(issue) = issue {
                claims.entry(issue.to_uppercase()).or_default().push((project.to_string(), kind, id.to_string(), created_at.to_string()));
            }
        };
        for data in &snapshot.projects {
            for t in &data.tasks {
                note(&data.slug, Kind::Task, &t.id, &t.jira_issue, &t.jira_unlinked, &t.created_at, &mut self.report);
            }
            for s in &data.stories {
                note(&data.slug, Kind::Story, &s.id, &s.jira_issue, &s.jira_unlinked, &s.created_at, &mut self.report);
            }
        }

        // An item that is gone, or now has another issue, or stopped syncing elsewhere, isn't tracked any more. Its
        // issue keeps the label, so it doesn't come back.
        self.state.items.retain(|key, tracked| {
            matches!(live.get(&(tracked.project.clone(), tracked.kind, tracked.id.to_lowercase())), Some((Some(current), false)) if current == key)
        });

        self.claimed = claims.keys().cloned().collect();
        let mut duplicates = HashSet::new();
        for (key, mut owners) in claims {
            if owners.len() < 2 {
                continue;
            }
            // The item that was already syncing with the issue keeps it; failing that, the oldest.
            let tracked = self.state.items.get(&key).map(|t| (t.project.clone(), t.kind, t.id.to_lowercase()));
            let is_tracked = |o: &(String, Kind, String, String)| tracked.as_ref() == Some(&(o.0.clone(), o.1, o.2.to_lowercase()));
            owners.sort_by(|a, b| is_tracked(b).cmp(&is_tracked(a)).then_with(|| (&a.3, &a.2).cmp(&(&b.3, &b.2))));
            for (project, kind, id, _) in owners.into_iter().skip(1) {
                duplicates.insert((project, kind, id.to_lowercase()));
            }
        }
        duplicates
    }

    fn project(&mut self, data: &ProjectData, link: &ProjectLink, duplicates: &HashSet<(String, Kind, String)>) -> JResult<()> {
        let jira = self.jira;
        let types = jira.issue_types(&link.project)?;
        let (story_type, task_type) = kind_types(&types);
        if story_type.is_none() && task_type.is_none() {
            self.report.notes.push(format!(
                "{}: the Jira project {} has no Story or Task issue type, so nothing was synced.",
                data.name, link.project_name
            ));
            return Ok(());
        }
        let wanted: Vec<String> = [story_type, task_type].into_iter().flatten().map(|t| t.id.clone()).collect();
        let issues = jira.issues(&link.project, &wanted)?;
        let statuses = task_type.map_or(&[][..], |t| t.statuses.as_slice());
        let board = Board {
            project: &link.project,
            mapping: Mapping::new(&data.columns, statuses, &link.columns),
            story_type,
            task_type,
            issues: issues.iter().map(|i| (i.key.to_uppercase(), i)).collect(),
        };
        let mut unmatched_columns: BTreeSet<String> = BTreeSet::new();

        // 1. Items that have an issue.
        let mut steps = vec![];
        for task in &data.tasks {
            let (Some(key), None) = (&task.jira_issue, &task.jira_unlinked) else { continue };
            if duplicates.contains(&(data.slug.clone(), Kind::Task, task.id.to_lowercase())) {
                steps.push(duplicate(&data.slug, Kind::Task, &task.id, &task.title));
                continue;
            }
            let local = Local {
                kind: Kind::Task,
                id: task.id.clone(),
                key: key.to_uppercase(),
                updated_at: task.updated_at.clone(),
                content: task_content(task, board.mapping.status_for(&task.column_id)),
                title: task.title.clone(),
                text: task.body.clone(),
            };
            if local.content.status.is_none() {
                unmatched_columns.insert(column_name(&data.columns, &task.column_id));
            }
            self.linked(data, &board, &local, &mut steps)?;
        }
        for story in &data.stories {
            let (Some(key), None) = (&story.jira_issue, &story.jira_unlinked) else { continue };
            if duplicates.contains(&(data.slug.clone(), Kind::Story, story.id.to_lowercase())) {
                steps.push(duplicate(&data.slug, Kind::Story, &story.id, &story.summary));
                continue;
            }
            let local = Local {
                kind: Kind::Story,
                id: story.id.clone(),
                key: key.to_uppercase(),
                updated_at: story.updated_at.clone(),
                content: story_content(story),
                title: story.summary.clone(),
                text: render_story(&story_text(story)),
            };
            self.linked(data, &board, &local, &mut steps)?;
        }
        self.commit(steps)?;

        // 2. Items that don't have an issue yet.
        let mut steps = vec![];
        let mut missing_types: BTreeSet<&str> = BTreeSet::new();
        for task in &data.tasks {
            if task.jira_issue.is_some() || task.jira_unlinked.is_some() {
                continue;
            }
            let Some(issue_type) = board.task_type else {
                missing_types.insert(TASK_TYPE);
                continue;
            };
            let Some(status) = board.mapping.status_for(&task.column_id) else {
                unmatched_columns.insert(column_name(&data.columns, &task.column_id));
                self.report.unmatched += 1;
                continue;
            };
            let content = task_content(task, Some(status));
            let label = format!("task “{}”", task.title);
            if let Some(step) = self.create(&board, issue_type, &data.slug, Kind::Task, &task.id, content, &label)? {
                steps.push(step);
            }
        }
        for story in &data.stories {
            if story.jira_issue.is_some() || story.jira_unlinked.is_some() {
                continue;
            }
            let Some(issue_type) = board.story_type else {
                missing_types.insert(STORY_TYPE);
                continue;
            };
            let content = story_content(story);
            let label = format!("user story “{}”", story.summary);
            if let Some(step) = self.create(&board, issue_type, &data.slug, Kind::Story, &story.id, content, &label)? {
                steps.push(step);
            }
        }
        self.commit(steps)?;

        // 3. Issues that have no item.
        let mut steps = vec![];
        let mut unmatched_statuses: BTreeSet<String> = BTreeSet::new();
        let mut labelled = 0;
        let mut taken_stories: HashSet<String> = data.stories.iter().map(|s| s.id.to_lowercase()).collect();
        for issue in &issues {
            let key = issue.key.to_uppercase();
            if self.claimed.contains(&key) {
                continue;
            }
            // An issue with the label had an item: one on its way from another machine, or one that was deleted.
            if issue.labels.iter().any(|l| l == LABEL) {
                labelled += 1;
                continue;
            }
            let created_at = if issue.created.is_empty() { ids::now() } else { issue.created.clone() };
            let stamp = if issue.updated.is_empty() { created_at.clone() } else { issue.updated.clone() };
            let label = format!("issue {} “{}”", issue.key, issue.summary);
            let mut step = if story_type.is_some_and(|t| t.id == issue.issue_type) {
                let Some(id) = story_id_for(&issue.key, &taken_stories) else { continue };
                taken_stories.insert(id.to_lowercase());
                let (content, _) = issue_content(issue, Kind::Story);
                let change = Change::CreateStory {
                    project: data.slug.clone(),
                    id: id.clone(),
                    summary: content.title.clone(),
                    text: story_from_jira(&content.desc),
                    issue: key.clone(),
                    created_at,
                    updated_at: stamp,
                };
                let mut step = Step::new(change, label, Count::ItemCreated);
                step.track = Some((key.clone(), Tracked { project: data.slug.clone(), kind: Kind::Story, id, base: content }));
                step
            } else {
                let Some(column) = board.mapping.column_for(&issue.status) else {
                    unmatched_statuses.insert(issue.status_name.clone());
                    self.report.unmatched += 1;
                    continue;
                };
                let id = format!("jira-{}", issue.key.to_lowercase());
                if data.tasks.iter().any(|t| t.id == id) {
                    continue;
                }
                let (content, _) = issue_content(issue, Kind::Task);
                let change = Change::CreateTask {
                    project: data.slug.clone(),
                    id: id.clone(),
                    title: content.title.clone(),
                    body: content.desc.clone(),
                    column: column.to_string(),
                    issue: key.clone(),
                    created_at,
                    updated_at: stamp,
                };
                let mut step = Step::new(change, label, Count::ItemCreated);
                step.track = Some((key.clone(), Tracked { project: data.slug.clone(), kind: Kind::Task, id, base: content }));
                step
            };
            step.mark = Some(key.clone());
            self.claimed.insert(key);
            steps.push(step);
        }
        self.commit(steps)?;

        if labelled > 0 {
            self.report.notes.push(format!(
                "{}: {labelled} issue(s) with the “{LABEL}” label have no item in this vault, so they weren't added: their items may still be on their way from another machine, or were deleted here. To add one anyway, take the label off it in Jira.",
                data.name
            ));
        }
        for kind in missing_types {
            self.report.notes.push(format!(
                "{}: the Jira project {} has no {kind} issue type, so new {} aren't put on Jira.",
                data.name,
                link.project_name,
                if kind == STORY_TYPE { "user stories" } else { "tasks" }
            ));
        }
        if !unmatched_columns.is_empty() {
            self.report.notes.push(format!(
                "{}: tasks in {} have no Jira status with a matching name, so they aren't moved there (choose a status for the column in the project's settings).",
                data.name,
                quoted(&unmatched_columns)
            ));
        }
        if !unmatched_statuses.is_empty() {
            self.report.notes.push(format!(
                "{}: tasks with the status {} on Jira have no column with a matching name, so they weren't added (choose a status for a column, or add a column with that name).",
                data.name,
                quoted(&unmatched_statuses)
            ));
        }
        Ok(())
    }

    /// One linked item, against its issue.
    fn linked(&mut self, data: &ProjectData, board: &Board, local: &Local, steps: &mut Vec<Step>) -> JResult<()> {
        let label = format!("{} “{}”", local.kind.word(), local.title);
        let found = match board.issues.get(&local.key) {
            Some(issue) => Some((*issue).clone()),
            // Not among the project's issues: find out what became of it, rather than assume.
            None => self.jira.issue(&local.key)?,
        };
        let wanted = match local.kind {
            Kind::Task => board.task_type,
            Kind::Story => board.story_type,
        };
        let gone = match &found {
            None => Some("The issue no longer exists on Jira, or this account can't see it.".to_string()),
            Some(issue) if issue.project != board.project => Some(format!("The issue was moved to another Jira project, as {}.", issue.key)),
            Some(issue) if wanted.is_none_or(|t| t.id != issue.issue_type) => {
                Some(format!("The issue is a {} on Jira now, not a {}.", issue.type_name, if local.kind == Kind::Task { TASK_TYPE } else { STORY_TYPE }))
            }
            Some(_) => None,
        };
        let (Some(issue), None) = (found, gone.clone()) else {
            let mut step = Step::new(
                Change::Unlink { project: data.slug.clone(), kind: local.kind, id: local.id.clone(), reason: gone.unwrap_or_default() },
                label,
                Count::Detached,
            );
            step.untrack = Some(local.key.clone());
            steps.push(step);
            return Ok(());
        };

        let (theirs, lossless) = issue_content(&issue, local.kind);
        let base = self
            .state
            .items
            .get(&local.key)
            .filter(|t| t.project == data.slug && t.kind == local.kind && t.id.eq_ignore_ascii_case(&local.id))
            .map(|t| t.base.clone());
        let mut merged = merge(local.kind, &local.content, &theirs, base.as_ref(), lossless, &board.mapping);
        if !issue.labels.iter().any(|l| l == LABEL) {
            merged.push.add_label = Some(LABEL.to_string());
        }
        if merged.unmatched {
            self.report.unmatched += 1;
        }

        // What this side has that the issue lacks.
        if !merged.push.is_empty() {
            let result = self.jira.update_issue(&issue.key, &merged.push);
            if self.soft(&label, result)?.is_none() {
                return Ok(());
            }
            if merged.push.summary.is_some() || merged.push.description.is_some() {
                self.report.issues_updated += 1;
            }
        }
        if let Some(status) = &merged.push_status
            && self.move_issue(&issue.key, status, &label)?
        {
            merged.content.status = Some(status.clone());
            if merged.push.summary.is_none() && merged.push.description.is_none() {
                self.report.issues_updated += 1;
            }
        }

        if merged.held_back {
            self.report.notes.push(format!(
                "{}: the description of {label} was changed here, but on Jira it holds things Ledger can't write (pictures, tables, mentions or colours), so it wasn't sent. Make the change on Jira.",
                data.name
            ));
        }
        if merged.jira_lost || merged.ledger_lost {
            self.report.conflicts += 1;
            if merged.jira_lost {
                self.keep(Side::Jira, &data.slug, local.kind, &local.id, &issue.key, &issue.summary, &jira_adf::to_markdown(issue.description.as_ref()).0)?;
            }
            if merged.ledger_lost {
                self.keep(Side::Ledger, &data.slug, local.kind, &local.id, &issue.key, &local.title, &local.text)?;
            }
            self.report.notes.push(format!(
                "{}: {label} changed on both sides. {}; the other is saved under .ledger/conflicts/{}.",
                data.name,
                if merged.ledger_lost { "Jira's description was kept, since Ledger can't write it" } else { "This machine's version was kept" },
                self.stamp
            ));
        }

        let tracked = Tracked { project: data.slug.clone(), kind: local.kind, id: local.id.clone(), base: merged.content.clone() };
        // What the issue has that this side lacks.
        if merged.pulls() {
            let stamp = if issue.updated.is_empty() { ids::now() } else { issue.updated.clone() };
            let change = match local.kind {
                Kind::Task => Change::UpdateTask {
                    project: data.slug.clone(),
                    id: local.id.clone(),
                    expect: local.updated_at.clone(),
                    title: merged.pull_title.clone(),
                    body: merged.pull_desc.clone(),
                    column: merged.pull_status.as_deref().and_then(|s| board.mapping.column_for(s)).map(str::to_string),
                    stamp,
                },
                Kind::Story => Change::UpdateStory {
                    project: data.slug.clone(),
                    id: local.id.clone(),
                    expect: local.updated_at.clone(),
                    summary: merged.pull_title.clone(),
                    // Read from what Jira says, not from its normal form: that would make any text look like the format.
                    text: merged.pull_desc.as_ref().map(|_| pulled_story(&local.content.desc, &jira_adf::from_jira(issue.description.as_ref()).0)),
                    stamp,
                },
            };
            let mut step = Step::new(change, label, Count::ItemUpdated);
            step.track = Some((local.key.clone(), tracked));
            steps.push(step);
        } else if base.as_ref() != Some(&tracked.base) {
            self.state.items.insert(local.key.clone(), tracked);
        }
        Ok(())
    }

    /// Makes the issue for an item, puts it in the item's status, and returns the step that links the two.
    #[allow(clippy::too_many_arguments)]
    fn create(&mut self, board: &Board, issue_type: &IssueType, project: &str, kind: Kind, id: &str, content: Content, label: &str) -> JResult<Option<Step>> {
        let new = NewIssue {
            project: board.project.to_string(),
            issue_type: issue_type.id.clone(),
            summary: content.title.clone(),
            description: to_adf(&content.desc),
            labels: vec![LABEL.to_string()],
        };
        let result = self.jira.create_issue(&new);
        let Some(key) = self.soft(label, result)? else { return Ok(None) };
        let key = key.to_uppercase();
        self.claimed.insert(key.clone());
        self.report.issues_created += 1;

        // What Jira made of it: the status it starts in, and the description as Jira keeps it.
        let made = self.jira.issue(&key);
        let base = match self.soft(label, made)?.flatten() {
            Some(issue) => {
                let mut base = issue_content(&issue, kind).0;
                if let Some(wanted) = &content.status
                    && issue.status != *wanted
                    && self.move_issue(&key, wanted, label)?
                {
                    base.status = Some(wanted.clone());
                }
                base
            }
            // Not known, so the next round decides: as both sides then differ from it, this side wins.
            None => Content { status: None, ..content },
        };
        let change = Change::Link { project: project.to_string(), kind, id: id.to_string(), issue: key.clone() };
        let mut step = Step::new(change, label.to_string(), Count::Nothing);
        step.track = Some((key, Tracked { project: project.to_string(), kind, id: id.to_string(), base }));
        Ok(Some(step))
    }

    /// Moves an issue to a status through a transition of its workflow. `false` (and a note) when none leads there.
    fn move_issue(&mut self, key: &str, status: &str, label: &str) -> JResult<bool> {
        let result = self.jira.transitions(key);
        let Some(transitions) = self.soft(label, result)? else { return Ok(false) };
        let Some(transition) = transitions.iter().find(|t| t.to == status) else {
            self.report.notes.push(format!(
                "{label}: Jira's workflow has no transition from the issue's status to the one of its column, so it wasn't moved. Move it on Jira, or change the column's status in the project's settings."
            ));
            return Ok(false);
        };
        let result = self.jira.transition(key, &transition.id);
        Ok(self.soft(label, result)?.is_some())
    }

    /// Makes the changes in the vault, and remembers what both sides now agree on for each that worked.
    fn commit(&mut self, steps: Vec<Step>) -> JResult<()> {
        if steps.is_empty() {
            return Ok(());
        }
        let (changes, rest): (Vec<Change>, Vec<_>) = steps.into_iter().map(|s| (s.change, (s.label, s.track, s.untrack, s.mark, s.count))).unzip();
        let results = self.ledger.apply(changes)?;
        for ((label, track, untrack, mark, count), result) in rest.into_iter().zip(results) {
            match result {
                Applied::Done => {
                    self.report.vault_writes += 1;
                    match count {
                        Count::Nothing => {}
                        Count::ItemCreated => self.report.items_created += 1,
                        Count::ItemUpdated => self.report.items_updated += 1,
                        Count::Detached => {
                            self.report.detached += 1;
                            self.report.unlinked += 1;
                        }
                    }
                    if let Some((key, tracked)) = track {
                        self.state.items.insert(key, tracked);
                    }
                    if let Some(key) = untrack {
                        self.state.items.remove(&key);
                    }
                    // If this fails the next round adds it, as it does for any linked issue without the label.
                    if let Some(key) = mark {
                        let result = self.jira.update_issue(&key, &IssueUpdate { add_label: Some(LABEL.to_string()), ..IssueUpdate::default() });
                        self.soft(&label, result)?;
                    }
                }
                Applied::Skipped => self.report.skipped += 1,
                Applied::Failed(message) => self.report.notes.push(format!("{label}: {message}")),
            }
        }
        Ok(())
    }

    /// A failure of one issue is a note and the round carries on; one that would hit every issue ends the round.
    fn soft<T>(&mut self, what: &str, result: JResult<T>) -> JResult<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(e @ (JiraError::RateLimited { .. } | JiraError::Unauthorized | JiraError::Ledger(_))) => Err(e),
            Err(e) => {
                self.report.notes.push(format!("{what}: {e}"));
                Ok(None)
            }
        }
    }

    /// Saves one side's version of an item that lost a conflict, where the GitHub sync keeps its own.
    #[allow(clippy::too_many_arguments)]
    fn keep(&self, side: Side, project: &str, kind: Kind, id: &str, issue: &str, title: &str, text: &str) -> JResult<()> {
        let mut meta = Meta::new();
        frontmatter::set(&mut meta, "kept", side.word());
        frontmatter::set(&mut meta, "project", project);
        frontmatter::set(&mut meta, "kind", kind.word());
        frontmatter::set(&mut meta, "item", id);
        frontmatter::set(&mut meta, "issue", issue);
        frontmatter::set(&mut meta, "title", title);
        let file = self
            .root
            .join(crate::settings::LEDGER_DIR)
            .join("conflicts")
            .join(&self.stamp)
            .join("jira")
            .join(project)
            .join(format!("{}-{id}.{}.md", kind.folder(), side.word()));
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        fs::write(&file, frontmatter::render(&meta, text)).map_err(|e| Error::io(&file, e))?;
        Ok(())
    }
}

fn duplicate(project: &str, kind: Kind, id: &str, title: &str) -> Step {
    Step::new(
        Change::Unlink {
            project: project.to_string(),
            kind,
            id: id.to_string(),
            reason: "Another item is linked to the same Jira issue, so this one stopped syncing.".into(),
        },
        format!("{} “{title}”", kind.word()),
        Count::Detached,
    )
}

#[derive(Clone, Copy)]
enum Side {
    Ledger,
    Jira,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Side::Ledger => "ledger",
            Side::Jira => "jira",
        }
    }
}

/// The story's text after taking what Jira says. A description in Ledger's format says everything. One written
/// freely in Jira replaces the notes, and the story keeps its role, goal, reason and criteria, which the next round
/// then writes back into the description.
fn pulled_story(ours: &str, theirs: &str) -> StoryText {
    let (text, marked) = parse_story_marked(&join_criteria(theirs));
    if marked { text } else { StoryText { notes: text.notes, ..story_from_jira(ours) } }
}

fn column_name(columns: &[Column], id: &str) -> String {
    columns.iter().find(|c| c.id == id).map_or_else(|| id.to_string(), |c| c.name.clone())
}

fn quoted(names: &BTreeSet<String>) -> String {
    names.iter().map(|n| format!("“{n}”")).collect::<Vec<_>>().join(", ")
}

/// The id of the story an issue becomes: its key, made from the issue so every machine picks the same; failing that,
/// the key behind `JIRA-`. `None` if both are taken.
fn story_id_for(key: &str, taken: &HashSet<String>) -> Option<String> {
    [key.to_string(), format!("JIRA-{key}")].into_iter().find(|id| !taken.contains(&id.to_lowercase()))
}
