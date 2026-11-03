//! The Ledger side of the Jira sync: writing what Jira changed back into the vault. Reading it is the Trello
//! sync's [`snapshot`](crate::trello_ledger::snapshot), which holds every task and story with its Jira link too.
//!
//! As there, the app's version ([`CoreLedger`]) takes the vault lock only for the moment it writes, changes the
//! in-memory database through the same stores the screens use, and writes the difference through to the files.
//! Each change says what the item looked like when it was read (`expect`), and is left undone if it has been
//! edited since: the next round deals with it.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, params};

use crate::error::{Error, Result};
use crate::models::{Criterion, StoryDraft, StoryUpdate, TaskUpdate};
use crate::state::Core;
use crate::trello_card::{Kind, StoryText};
use crate::trello_ledger::{Applied, Snapshot, snapshot};
use crate::{board, stories};

/// One change to make to the vault.
#[derive(Debug, Clone)]
pub enum Change {
    /// An issue was made for the item: remember its key. Doesn't count as an edit.
    Link { project: String, kind: Kind, id: String, issue: String },
    /// The item stops syncing, and says why. Its issue key stays, so it never gets a second issue.
    Unlink { project: String, kind: Kind, id: String, reason: String },
    UpdateTask { project: String, id: String, expect: String, title: Option<String>, body: Option<String>, column: Option<String>, stamp: String },
    UpdateStory { project: String, id: String, expect: String, summary: Option<String>, text: Option<StoryText>, stamp: String },
    CreateTask { project: String, id: String, title: String, body: String, column: String, issue: String, created_at: String, updated_at: String },
    CreateStory { project: String, id: String, summary: String, text: StoryText, issue: String, created_at: String, updated_at: String },
}

pub trait Ledger: Sync {
    fn snapshot(&self) -> Result<Snapshot>;
    /// Applies the changes in order; one answer for each.
    fn apply(&self, changes: Vec<Change>) -> Result<Vec<Applied>>;
}

pub struct CoreLedger {
    pub core: Arc<Mutex<Core>>,
    /// Called after the vault's files were written, so the GitHub sync picks the change up.
    pub changed: Arc<dyn Fn() + Send + Sync>,
}

fn lock(core: &Mutex<Core>) -> MutexGuard<'_, Core> {
    core.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Ledger for CoreLedger {
    fn snapshot(&self) -> Result<Snapshot> {
        snapshot(&lock(&self.core).conn)
    }

    fn apply(&self, changes: Vec<Change>) -> Result<Vec<Applied>> {
        let (results, wrote) = {
            let mut guard = lock(&self.core);
            let core = &mut *guard;
            let results: Vec<Applied> = changes
                .iter()
                .map(|change| apply_one(&core.conn, change).unwrap_or_else(|e| Applied::Failed(e.to_string())))
                .collect();
            let wrote = results.contains(&Applied::Done);
            if wrote {
                core.vault.persist(&core.conn)?;
            }
            (results, wrote)
        };
        if wrote {
            (self.changed)();
        }
        Ok(results)
    }
}

fn table(kind: Kind) -> &'static str {
    match kind {
        Kind::Task => "tasks",
        Kind::Story => "user_stories",
    }
}

/// Records the issue key on an item without counting it as an edit. Returns whether the item exists.
pub fn set_link(conn: &Connection, project: &str, kind: Kind, id: &str, issue: Option<&str>, unlinked: Option<&str>) -> Result<bool> {
    let sql = format!("UPDATE {} SET jira_issue = ?3, jira_unlinked = ?4 WHERE project_slug = ?1 AND id = ?2", table(kind));
    Ok(conn.execute(&sql, params![project, id, issue, unlinked])? > 0)
}

fn stamp_item(conn: &Connection, project: &str, kind: Kind, id: &str, created_at: Option<&str>, updated_at: &str) -> Result<()> {
    let sql = match created_at {
        Some(_) => format!("UPDATE {} SET created_at = ?3, updated_at = ?4 WHERE project_slug = ?1 AND id = ?2", table(kind)),
        None => format!("UPDATE {} SET updated_at = ?4 WHERE project_slug = ?1 AND id = ?2", table(kind)),
    };
    conn.execute(&sql, params![project, id, created_at, updated_at])?;
    Ok(())
}

fn criteria(text: &StoryText) -> Vec<Criterion> {
    text.criteria.iter().map(|c| Criterion { text: c.text.clone(), done: c.done }).collect()
}

fn apply_one(conn: &Connection, change: &Change) -> Result<Applied> {
    Ok(match change {
        Change::Link { project, kind, id, issue } => {
            if set_link(conn, project, *kind, id, Some(issue), None)? { Applied::Done } else { Applied::Skipped }
        }
        Change::Unlink { project, kind, id, reason } => {
            let issue = match kind {
                Kind::Task => board::task(conn, project, id).ok().and_then(|t| t.jira_issue),
                Kind::Story => stories::get(conn, project, id).ok().and_then(|s| s.jira_issue),
            };
            match issue {
                Some(issue) if set_link(conn, project, *kind, id, Some(&issue), Some(reason))? => Applied::Done,
                _ => Applied::Skipped,
            }
        }
        Change::UpdateTask { project, id, expect, title, body, column, stamp } => {
            match board::task(conn, project, id) {
                Ok(task) if task.updated_at == *expect => {}
                _ => return Ok(Applied::Skipped),
            }
            if let Some(column) = column
                && !board::column_exists(conn, project, column)?
            {
                return Ok(Applied::Failed(format!("the column {column} no longer exists")));
            }
            board::update_task(conn, project, id, TaskUpdate { title: title.clone(), body: body.clone(), column_id: column.clone(), ..TaskUpdate::default() })?;
            stamp_item(conn, project, Kind::Task, id, None, stamp)?;
            Applied::Done
        }
        Change::UpdateStory { project, id, expect, summary, text, stamp } => {
            let story = match stories::get(conn, project, id) {
                Ok(story) if story.updated_at == *expect => story,
                _ => return Ok(Applied::Skipped),
            };
            let mut update = StoryUpdate { summary: summary.clone(), ..StoryUpdate::default() };
            if let Some(text) = text {
                update.role = Some(text.role.clone());
                update.want = Some(text.want.clone());
                update.reason = Some(text.reason.clone());
                update.notes = Some(text.notes.clone());
                update.criteria = Some(criteria(text));
            }
            stories::update(conn, project, &story.id, update)?;
            stamp_item(conn, project, Kind::Story, &story.id, None, stamp)?;
            Applied::Done
        }
        Change::CreateTask { project, id, title, body, column, issue, created_at, updated_at } => {
            if title.trim().is_empty() {
                return Ok(Applied::Failed("the issue has no summary".into()));
            }
            if !board::column_exists(conn, project, column)? {
                return Ok(Applied::Failed(format!("the column {column} no longer exists")));
            }
            let inserted = conn.execute(
                "INSERT OR IGNORE INTO tasks (id, project_slug, title, body, column_id, story_id, thread_slug, created_at, updated_at, jira_issue)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7, ?8)",
                params![id, project, title.trim(), body.trim(), column, created_at, updated_at, issue],
            )?;
            if inserted == 0 { Applied::Skipped } else { Applied::Done }
        }
        Change::CreateStory { project, id, summary, text, issue, created_at, updated_at } => {
            if stories::exists(conn, project, id)? {
                return Ok(Applied::Skipped);
            }
            let draft = StoryDraft {
                id: id.to_string(),
                summary: summary.to_string(),
                role: text.role.clone(),
                want: text.want.clone(),
                reason: text.reason.clone(),
                notes: text.notes.clone(),
                criteria: criteria(text),
                thread_slugs: vec![],
            };
            let created = stories::create(conn, project, draft)?;
            set_link(conn, project, Kind::Story, &created.id, Some(issue), None)?;
            stamp_item(conn, project, Kind::Story, &created.id, Some(created_at), updated_at)?;
            Applied::Done
        }
    })
}

/// Gives an item a new issue: its link is cleared, so the next sync makes one. Returns the issue it had.
pub fn clear_link(conn: &Connection, project: &str, kind: Kind, id: &str) -> Result<Option<String>> {
    let (issue, canonical) = match kind {
        Kind::Task => (board::task(conn, project, id)?.jira_issue, id.to_string()),
        Kind::Story => {
            let story = stories::get(conn, project, id)?;
            (story.jira_issue, story.id)
        }
    };
    if !set_link(conn, project, kind, &canonical, None, None)? {
        return Err(Error::not_found("Item not found"));
    }
    Ok(issue)
}
