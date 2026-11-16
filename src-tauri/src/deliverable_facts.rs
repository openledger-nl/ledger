//! What a deliverable is for, beyond its file: its number (01, 02, … in the order deliverables are made, never reused),
//! its goal in your own words, the AI-use label it is meant to get, the question it answers and the template it started
//! from. Any kind of file can be a deliverable, so these are kept in the project's `project.md`, by the live path:
//!
//! ```text
//! nextDeliverable: 3            the number the next one gets
//! deliverables:
//! - path: 01-project-plan.md
//!   number: 1
//!   goal: Agree with the client on what is built first.
//!   mmm: cyborg
//!   answers: main               "main", or a sub-question in words
//!   template: plan
//! ```
//!
//! Deleting a deliverable drops its facts. A file renamed outside Ledger keeps them when its new name starts with the same
//! number ("01-plan.md" → "01-project-plan.md"); facts whose file is gone otherwise are kept, so a file that comes back
//! finds them again.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::frontmatter::{self, MetaExt};
use crate::{deliverables, projects, provenance};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeliverableMeta {
    pub project: String,
    /// The live path, as the deliverables folder has it.
    pub path: String,
    pub number: i64,
    pub goal: String,
    pub mmm: Option<String>,
    /// The question it answers: "main", or a sub-question's text.
    pub answers: Option<String>,
    pub template: Option<String>,
}

const COLUMNS: &str = "project_slug, path, number, goal, mmm, answers, template";

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<DeliverableMeta> {
    Ok(DeliverableMeta {
        project: r.get(0)?,
        path: r.get(1)?,
        number: r.get(2)?,
        goal: r.get(3)?,
        mmm: r.get(4)?,
        answers: r.get(5)?,
        template: r.get(6)?,
    })
}

pub fn get(conn: &Connection, project: &str, path: &str) -> Result<Option<DeliverableMeta>> {
    projects::require(conn, project)?;
    query_opt(conn, &format!("SELECT {COLUMNS} FROM deliverable_facts WHERE project_slug = ?1 AND path = ?2"), [project, path], from_row)
}

/// The facts of the project's deliverables, by number.
pub fn list(conn: &Connection, project: &str) -> Result<Vec<DeliverableMeta>> {
    projects::require(conn, project)?;
    query_all(conn, &format!("SELECT {COLUMNS} FROM deliverable_facts WHERE project_slug = ?1 ORDER BY number, path"), [project], from_row)
}

/// The number a file's name starts with: "01-project-plan.md" (as Ledger names it) or "01 Project plan.md" → 1.
fn number_in(path: &str) -> Option<i64> {
    let name = deliverables::file_name(path);
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() >= 2 && name[digits.len()..].starts_with([' ', '-', '_']) { digits.parse().ok() } else { None }
}

/// The number the next deliverable of the project gets: one past the highest ever given (kept in `nextDeliverable`, so a
/// deleted one's number isn't given again) and past any "NN name" file already there.
pub fn next_number(conn: &Connection, project: &str) -> Result<i64> {
    projects::require(conn, project)?;
    let next: i64 = conn.query_row("SELECT next_deliverable FROM projects WHERE slug = ?1", [project], |r| r.get(0))?;
    let highest: i64 = conn.query_row("SELECT COALESCE(MAX(number), 0) FROM deliverable_facts WHERE project_slug = ?1", [project], |r| r.get(0))?;
    let paths: Vec<String> = query_all(conn, "SELECT path FROM deliverables WHERE project_slug = ?1", [project], |r| r.get(0))?;
    let in_names = paths.iter().filter_map(|p| number_in(&deliverables::split_version(p).0)).max().unwrap_or(0);
    Ok(next.max(highest + 1).max(in_names + 1).max(1))
}

/// Sets the facts of a deliverable that is there; its number counts as given from then on.
pub fn save(conn: &Connection, meta: &DeliverableMeta) -> Result<DeliverableMeta> {
    let project = meta.project.as_str();
    projects::require(conn, project)?;
    let path = deliverables::split_version(meta.path.trim()).0;
    if !deliverables::exists(conn, project, &path)? {
        return Err(Error::not_found("Deliverable not found"));
    }
    if meta.number < 1 {
        return Err(Error::invalid("A deliverable's number starts at 1"));
    }
    let goal = meta.goal.trim();
    if goal.is_empty() {
        return Err(Error::invalid("Say what the deliverable is for: its goal"));
    }
    let text = |t: &Option<String>| t.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    let mmm = text(&meta.mmm);
    if let Some(level) = &mmm
        && provenance::level(level).is_none()
    {
        return Err(Error::invalid("That isn't one of the five AI-use labels"));
    }
    conn.execute(
        "INSERT OR REPLACE INTO deliverable_facts (project_slug, path, number, goal, mmm, answers, template) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![project, path, meta.number, goal, mmm, text(&meta.answers), text(&meta.template)],
    )?;
    conn.execute("UPDATE projects SET next_deliverable = MAX(next_deliverable, ?2) WHERE slug = ?1", params![project, meta.number + 1])?;
    get(conn, project, &path)?.ok_or_else(|| Error::not_found("Deliverable not found"))
}

/// A deliverable deleted: its facts go (its number stays given).
pub fn forget(conn: &Connection, project: &str, path: &str) -> Result<()> {
    conn.execute("DELETE FROM deliverable_facts WHERE project_slug = ?1 AND path = ?2", [project, path])?;
    Ok(())
}

// ---- the vault --------------------------------------------------------------------------------------

pub fn export(conn: &Connection, project: &str, meta: &mut Mapping) -> Result<()> {
    let next: i64 = conn.query_row("SELECT next_deliverable FROM projects WHERE slug = ?1", [project], |r| r.get(0))?;
    if next > 0 {
        frontmatter::set(meta, "nextDeliverable", next);
    }
    let facts = query_all(conn, &format!("SELECT {COLUMNS} FROM deliverable_facts WHERE project_slug = ?1 ORDER BY number, path"), [project], from_row)?;
    if facts.is_empty() {
        return Ok(());
    }
    let items = facts
        .into_iter()
        .map(|f| {
            let mut item = Mapping::new();
            frontmatter::set(&mut item, "path", f.path);
            frontmatter::set(&mut item, "number", f.number);
            frontmatter::set(&mut item, "goal", f.goal);
            frontmatter::set_opt(&mut item, "mmm", f.mmm.as_deref());
            frontmatter::set_opt(&mut item, "answers", f.answers.as_deref());
            frontmatter::set_opt(&mut item, "template", f.template.as_deref());
            Value::Mapping(item)
        })
        .collect();
    frontmatter::set(meta, "deliverables", Value::Sequence(items));
    Ok(())
}

/// Reads the facts leniently: one without a path or a goal is skipped, a missing number is taken from the file's name
/// (or the next free one), a label that isn't one of the five is no label; of two for the same path the first wins.
pub fn import(conn: &Connection, project: &str, meta: &Mapping) -> Result<()> {
    let next = meta.text("nextDeliverable").and_then(|n| n.parse::<i64>().ok()).unwrap_or(0).max(0);
    conn.execute("UPDATE projects SET next_deliverable = ?2 WHERE slug = ?1", params![project, next])?;
    for item in meta.maps("deliverables") {
        let (Some(path), Some(goal)) = (item.text("path"), item.text("goal")) else { continue };
        let path = deliverables::split_version(path.trim()).0;
        let number = match item.text("number").and_then(|n| n.parse::<i64>().ok()).filter(|n| *n >= 1).or_else(|| number_in(&path)) {
            Some(n) => n,
            None => conn.query_row(
                "SELECT MAX(COALESCE(MAX(number), 0) + 1, ?2) FROM deliverable_facts WHERE project_slug = ?1",
                params![project, next.max(1)],
                |r| r.get(0),
            )?,
        };
        let mmm = item.text("mmm").and_then(|l| provenance::level(&l).map(str::to_string));
        conn.execute(
            "INSERT OR IGNORE INTO deliverable_facts (project_slug, path, number, goal, mmm, answers, template) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![project, path, number, goal.trim(), mmm, item.text("answers"), item.text("template")],
        )?;
    }
    Ok(())
}

/// Once the project's files are in: facts whose file is gone follow a file renamed outside Ledger when its new name
/// starts with the same number and it has no facts of its own.
pub fn reattach(conn: &Connection, project: &str) -> Result<()> {
    let live: Vec<String> = query_all(conn, "SELECT path FROM deliverables WHERE project_slug = ?1", [project], |r| r.get::<_, String>(0))?
        .into_iter()
        .filter(|p| deliverables::split_version(p).1.is_none())
        .collect();
    for fact in query_all(conn, &format!("SELECT {COLUMNS} FROM deliverable_facts WHERE project_slug = ?1"), [project], from_row)? {
        if live.contains(&fact.path) {
            continue;
        }
        let candidates: Vec<&String> = live.iter().filter(|p| number_in(p) == Some(fact.number)).collect();
        let [renamed] = candidates.as_slice() else { continue };
        let taken = query_opt(conn, "SELECT 1 FROM deliverable_facts WHERE project_slug = ?1 AND path = ?2", [project, renamed.as_str()], |_| Ok(()))?;
        if taken.is_none() {
            conn.execute("UPDATE deliverable_facts SET path = ?3 WHERE project_slug = ?1 AND path = ?2", params![project, fact.path, renamed])?;
        }
    }
    Ok(())
}
