//! Semester templates: what a new semester can start from, such as NJ26, the autumn semester of 2026. A template is a
//! semester that isn't one yet: its days, vacations and sprints, and the name the semester gets. Nothing is counted in
//! it, and it may overlap a semester or another template. One file each, in the form of a semester's file (see
//! `semesters::file_meta`):
//!
//! ```text
//! shared/templates/semesters/<id>.md
//! ---
//! name: NJ26 - Semester X
//! start: 2026-08-31
//! end: 2027-01-24
//! sprintWeeks: 3
//! sprints: [...]
//! vacations: [...]
//! createdAt: …
//! updatedAt: …
//! ---
//!
//! Notes, which the semester made from it starts with.
//! ```
//!
//! A new vault starts with NJ26 (`seed`); like every template it is the vault's to change or delete.

use rusqlite::{Connection, params};
use serde::Serialize;
use serde_yaml_ng::Mapping;

use crate::{db::{query_all, query_opt}, error::{Error, Result}, frontmatter::{self, MetaExt}, ids, models::{SemesterDraft, Sprint, Vacation}, semesters};

pub const FOLDER: &str = "shared/templates/semesters";

/// A semester template: its id (the file's name) and the semester it makes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemesterTemplate {
    pub id: String,
    #[serde(flatten)]
    pub draft: SemesterDraft,
    pub created_at: String,
    pub updated_at: String,
}

pub fn file(id: &str) -> String {
    format!("{FOLDER}/{id}.md")
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<SemesterTemplate> {
    let draft: String = r.get(1)?;
    Ok(SemesterTemplate { id: r.get(0)?, draft: serde_json::from_str(&draft).unwrap_or_default(), created_at: r.get(2)?, updated_at: r.get(3)? })
}

/// Every template, the earliest semester first.
pub fn list(conn: &Connection) -> Result<Vec<SemesterTemplate>> {
    let mut all = query_all(conn, "SELECT id, draft, created_at, updated_at FROM semester_templates", [], row)?;
    all.sort_by(|a, b| (&a.draft.start, &a.draft.name, &a.id).cmp(&(&b.draft.start, &b.draft.name, &b.id)));
    Ok(all)
}

pub fn get(conn: &Connection, id: &str) -> Result<SemesterTemplate> {
    query_opt(conn, "SELECT id, draft, created_at, updated_at FROM semester_templates WHERE id = ?1", [id], row)?
        .ok_or_else(|| Error::not_found(format!("There is no semester template {id} in this vault")))
}

fn exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(query_opt(conn, "SELECT 1 FROM semester_templates WHERE id = ?1", [id], |r| r.get::<_, i64>(0))?.is_some())
}

fn insert(conn: &Connection, id: &str, draft: &SemesterDraft, created_at: &str, updated_at: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO semester_templates (id, draft, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, serde_json::to_string(draft).unwrap_or_default(), created_at, updated_at],
    )?;
    Ok(())
}

/// Saves a template, checked as a semester is (but not against other semesters): a new one when `id` is None, its id
/// the semester's name as a slug, or a change to the one with that id.
pub fn save(conn: &Connection, id: Option<&str>, draft: &SemesterDraft) -> Result<SemesterTemplate> {
    let draft = semesters::check(draft)?.draft();
    let now = ids::now();
    let id = match id {
        Some(id) => {
            let changed = conn.execute(
                "UPDATE semester_templates SET draft = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, serde_json::to_string(&draft).unwrap_or_default(), now],
            )?;
            if changed == 0 {
                return Err(Error::not_found(format!("There is no semester template {id} in this vault")));
            }
            id.to_string()
        }
        None => {
            let id = ids::numbered(&ids::slugify_as(&draft.name, "semester"), |id| exists(conn, id))?;
            insert(conn, &id, &draft, &now, &now)?;
            id
        }
    };
    get(conn, &id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM semester_templates WHERE id = ?1", [id])? == 0 {
        return Err(Error::not_found(format!("There is no semester template {id} in this vault")));
    }
    Ok(())
}

// ---- the files ----------------------------------------------------------------------------------------

pub fn render(template: &SemesterTemplate) -> String {
    let mut meta = semesters::file_meta(&template.draft);
    frontmatter::set(&mut meta, "createdAt", template.created_at.clone());
    frontmatter::set(&mut meta, "updatedAt", template.updated_at.clone());
    frontmatter::render(&meta, &template.draft.notes)
}

/// Reads a template's file as leniently as a semester's (see `semesters::check_leniently`); one that makes no
/// semester at all is left out.
pub fn import(conn: &Connection, id: &str, meta: &Mapping, notes: &str, stamp: &str) -> Result<()> {
    let Some(checked) = semesters::check_leniently(semesters::draft_from(meta, notes, id)) else { return Ok(()) };
    let (created_at, updated_at) = meta.stamps(stamp);
    insert(conn, id, &checked.draft(), &created_at, &updated_at)
}

// ---- the template a new vault starts with -------------------------------------------------------------

/// NJ26, the autumn semester of 2026, as its calendar has it: a kick-off week, a shorter sprint 0, five sprints of three
/// weeks, a week off in autumn and two at Christmas. It ends with the last sprint: the weeks after it (the end talks and
/// the graduation conference) are events, not sprints. The name keeps its X for whoever makes a semester of it to fill
/// in.
pub fn nj26() -> SemesterDraft {
    let span = |name: &str, start: &str, end: &str| (name.to_string(), start.to_string(), end.to_string());
    let sprints = [
        span("Kick-off", "2026-08-31", "2026-09-06"),
        span("Sprint 0", "2026-09-07", "2026-09-20"),
        span("Sprint 1", "2026-09-21", "2026-10-11"),
        span("Sprint 2", "2026-10-12", "2026-11-08"),
        span("Sprint 3", "2026-11-09", "2026-11-29"),
        span("Sprint 4", "2026-11-30", "2026-12-20"),
        span("Sprint 5", "2027-01-04", "2027-01-24"),
    ];
    let vacations = [span("Autumn break", "2026-10-19", "2026-10-25"), span("Christmas break", "2026-12-21", "2027-01-03")];
    SemesterDraft {
        name: "NJ26 - Semester X".into(),
        start: "2026-08-31".into(),
        end: "2027-01-24".into(),
        sprint_weeks: 3,
        notes: String::new(),
        sprints: sprints.into_iter().map(|(name, start, end)| Sprint { name, start, end, goal: String::new() }).collect(),
        vacations: vacations.into_iter().map(|(name, start, end)| Vacation { name, start, end }).collect(),
    }
}

/// Gives a new vault its semester template, NJ26 (`nj26`).
pub fn seed(conn: &Connection) -> Result<()> {
    let draft = semesters::check(&nj26())?.draft();
    let now = ids::now();
    insert(conn, "nj26", &draft, &now, &now)
}
