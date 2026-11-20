//! What a project is, beyond its name: its kind, its goal, when it ends and the AI-use label its work is meant to get.
//! Kept in the frontmatter of the project's `project.md`:
//!
//! ```text
//! kind: study                  study, hobby or company
//! goal: Give the bakery a rota tool their staff can use on a phone.
//! end: semester                semester, a calendar day (2027-01-31), or left out for no end
//! semester: NJ26               the semester a study project runs in
//! intendedMmm: cyborg          the label the work is meant to get
//! mainQuestion: How can …?     a study project's main question
//! client: Bakery Bos           a company project's client
//! ```
//!
//! A project without a kind and a goal has no facts yet: the screens then ask for them.
//!
//! Its people are listed there too, whether or not it has facts, each with their role:
//!
//! ```text
//! people:
//! - name: Frank de Vries
//!   role: coach
//! - name: Sanne
//!   role: teammate
//!   portflowUser: 4711        when they are someone in Portflow
//! ```
//!
//! A person goes by a tag, `role/name` (`coach/frank-de-vries`): how meetings and feedback name them.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Mapping;

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::frontmatter::{self, MetaExt};
use crate::{ids, projects, provenance};

pub const KINDS: [&str; 3] = ["study", "hobby", "company"];
pub const ROLES: [&str; 6] = ["coach", "expert", "teammate", "client", "colleague", "other"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectMeta {
    pub slug: String,
    pub kind: String,
    pub goal: String,
    /// "semester", a calendar day, or `None` for no end.
    pub end: Option<String>,
    pub semester: Option<String>,
    pub intended_mmm: Option<String>,
    pub main_question: Option<String>,
    pub client: Option<String>,
    /// How daily notes and feedback tag the project: `project/<tag>`. Its slug.
    pub tag: String,
}

const COLUMNS: &str = "slug, kind, goal, project_end, semester, intended_mmm, main_question, client";

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Option<ProjectMeta>> {
    let slug: String = r.get(0)?;
    let kind: Option<String> = r.get(1)?;
    let goal: Option<String> = r.get(2)?;
    let (Some(kind), Some(goal)) = (kind, goal) else { return Ok(None) };
    Ok(Some(ProjectMeta {
        tag: slug.clone(),
        slug,
        kind,
        goal,
        end: r.get(3)?,
        semester: r.get(4)?,
        intended_mmm: r.get(5)?,
        main_question: r.get(6)?,
        client: r.get(7)?,
    }))
}

/// The project's facts; `None` while it has no kind and goal.
pub fn get(conn: &Connection, slug: &str) -> Result<Option<ProjectMeta>> {
    projects::require(conn, slug)?;
    Ok(query_opt(conn, &format!("SELECT {COLUMNS} FROM projects WHERE slug = ?1"), [slug], from_row)?.flatten())
}

/// The facts of every project that has them.
pub fn list(conn: &Connection) -> Result<Vec<ProjectMeta>> {
    Ok(query_all(conn, &format!("SELECT {COLUMNS} FROM projects ORDER BY slug"), [], from_row)?.into_iter().flatten().collect())
}

fn trimmed(text: &Option<String>) -> Option<String> {
    text.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

/// Checks the facts and puts them in their canonical form: a kind and a goal are required, the end is "semester" or a
/// day, the label is one of the five.
fn clean(meta: &ProjectMeta) -> Result<ProjectMeta> {
    let kind = meta.kind.trim();
    if !KINDS.contains(&kind) {
        return Err(Error::invalid("A project is a study, hobby or company project"));
    }
    let goal = meta.goal.trim();
    if goal.is_empty() {
        return Err(Error::invalid("Say what the project must achieve: its goal"));
    }
    let end = trimmed(&meta.end);
    if let Some(end) = &end
        && end != "semester"
        && !crate::dates::is_day(end)
    {
        return Err(Error::invalid("A project ends with its semester, on a day (2027-01-31), or not at all"));
    }
    let intended = trimmed(&meta.intended_mmm);
    if let Some(level) = &intended
        && provenance::level(level).is_none()
    {
        return Err(Error::invalid("That isn't one of the five AI-use labels"));
    }
    Ok(ProjectMeta {
        slug: meta.slug.clone(),
        kind: kind.to_string(),
        goal: goal.to_string(),
        end,
        semester: trimmed(&meta.semester),
        intended_mmm: intended,
        main_question: trimmed(&meta.main_question),
        client: trimmed(&meta.client),
        tag: meta.slug.clone(),
    })
}

/// Sets the project's facts (all of them: what is left out is cleared).
pub fn save(conn: &Connection, slug: &str, meta: &ProjectMeta) -> Result<ProjectMeta> {
    projects::require(conn, slug)?;
    let m = clean(&ProjectMeta { slug: slug.to_string(), ..meta.clone() })?;
    conn.execute(
        "UPDATE projects SET kind = ?2, goal = ?3, project_end = ?4, semester = ?5, intended_mmm = ?6, main_question = ?7, client = ?8,
                             updated_at = ?9 WHERE slug = ?1",
        params![slug, m.kind, m.goal, m.end, m.semester, m.intended_mmm, m.main_question, m.client, ids::now()],
    )?;
    get(conn, slug)?.ok_or_else(|| Error::not_found("Project not found"))
}

// ---- the vault --------------------------------------------------------------------------------------

/// Writes the facts into the project's frontmatter; nothing for a project that has none.
pub fn export(conn: &Connection, slug: &str, meta: &mut Mapping) -> Result<()> {
    let Some(m) = query_opt(conn, &format!("SELECT {COLUMNS} FROM projects WHERE slug = ?1"), [slug], from_row)?.flatten() else { return Ok(()) };
    frontmatter::set(meta, "kind", m.kind);
    frontmatter::set(meta, "goal", m.goal);
    frontmatter::set_opt(meta, "end", m.end.as_deref());
    frontmatter::set_opt(meta, "semester", m.semester.as_deref());
    frontmatter::set_opt(meta, "intendedMmm", m.intended_mmm.as_deref());
    frontmatter::set_opt(meta, "mainQuestion", m.main_question.as_deref());
    frontmatter::set_opt(meta, "client", m.client.as_deref());
    Ok(())
}

/// Reads the facts from the project's frontmatter, leniently: a kind written in capitals is still a kind, an end that is
/// neither "semester" nor a day is no end, a label that isn't one of the five is no label. Without a known kind and a goal
/// the project has no facts.
pub fn import(conn: &Connection, slug: &str, meta: &Mapping) -> Result<()> {
    let kind = meta.text("kind").map(|k| k.trim().to_lowercase()).filter(|k| KINDS.contains(&k.as_str()));
    let goal = meta.text("goal").map(|g| g.trim().to_string()).filter(|g| !g.is_empty());
    let (Some(kind), Some(goal)) = (kind, goal) else { return Ok(()) };
    let end = meta.text("end").map(|e| e.trim().to_lowercase()).filter(|e| e == "semester" || crate::dates::is_day(e));
    let intended = meta.text("intendedMmm").and_then(|l| provenance::level(l.trim()).map(str::to_string));
    conn.execute(
        "UPDATE projects SET kind = ?2, goal = ?3, project_end = ?4, semester = ?5, intended_mmm = ?6, main_question = ?7, client = ?8 WHERE slug = ?1",
        params![slug, kind, goal, end, meta.text("semester"), intended, meta.text("mainQuestion"), meta.text("client")],
    )?;
    Ok(())
}

// ---- its people -------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectPerson {
    /// The project they are part of.
    pub project: String,
    pub name: String,
    /// coach, expert, teammate, client, colleague or other.
    pub role: String,
    /// Their Portflow id, when they are someone in Portflow.
    pub portflow_user: Option<i64>,
    /// `role/name`, as meetings and feedback tag them (filled in by Ledger).
    pub tag: String,
}

/// The tag a person goes by: `coach/frank-de-vries`.
pub fn person_tag(role: &str, name: &str) -> String {
    format!("{role}/{}", ids::slugify_as(name, "someone"))
}

fn person_row(r: &rusqlite::Row) -> rusqlite::Result<ProjectPerson> {
    let role: String = r.get(2)?;
    let name: String = r.get(1)?;
    Ok(ProjectPerson { project: r.get(0)?, tag: person_tag(&role, &name), name, role, portflow_user: r.get(3)? })
}

/// The people of one project, or of every project with `None`, in the order they are listed.
pub fn people(conn: &Connection, project: Option<&str>) -> Result<Vec<ProjectPerson>> {
    let sql = "SELECT project_slug, name, role, portflow_user FROM project_people";
    match project {
        Some(slug) => {
            projects::require(conn, slug)?;
            query_all(conn, &format!("{sql} WHERE project_slug = ?1 ORDER BY position"), [slug], person_row)
        }
        None => query_all(conn, &format!("{sql} ORDER BY project_slug, position"), [], person_row),
    }
}

/// Replaces the project's people: a name and one of the roles each; the same person in the same role is listed once.
pub fn save_people(conn: &Connection, slug: &str, people: &[ProjectPerson]) -> Result<Vec<ProjectPerson>> {
    projects::require(conn, slug)?;
    let mut cleaned: Vec<(String, String, Option<i64>)> = vec![];
    for p in people {
        let name = p.name.trim();
        if name.is_empty() {
            return Err(Error::invalid("Give everyone a name"));
        }
        let role = p.role.trim().to_lowercase();
        if !ROLES.contains(&role.as_str()) {
            return Err(Error::invalid("A person is a coach, expert, teammate, client, colleague or other"));
        }
        if !cleaned.iter().any(|(n, r, _)| person_tag(r, n) == person_tag(&role, name)) {
            cleaned.push((name.to_string(), role, p.portflow_user));
        }
    }
    store_people(conn, slug, &cleaned)?;
    conn.execute("UPDATE projects SET updated_at = ?2 WHERE slug = ?1", params![slug, ids::now()])?;
    self::people(conn, Some(slug))
}

fn store_people(conn: &Connection, slug: &str, people: &[(String, String, Option<i64>)]) -> Result<()> {
    conn.execute("DELETE FROM project_people WHERE project_slug = ?1", [slug])?;
    for (position, (name, role, user)) in people.iter().enumerate() {
        conn.execute(
            "INSERT INTO project_people (project_slug, position, name, role, portflow_user) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![slug, position as i64, name, role, user],
        )?;
    }
    Ok(())
}

pub fn export_people(conn: &Connection, slug: &str, meta: &mut Mapping) -> Result<()> {
    let list = self::people(conn, Some(slug))?;
    if list.is_empty() {
        return Ok(());
    }
    let items = list
        .into_iter()
        .map(|p| {
            let mut item = Mapping::new();
            frontmatter::set(&mut item, "name", p.name);
            frontmatter::set(&mut item, "role", p.role);
            if let Some(user) = p.portflow_user {
                frontmatter::set(&mut item, "portflowUser", user);
            }
            serde_yaml_ng::Value::Mapping(item)
        })
        .collect();
    frontmatter::set(meta, "people", serde_yaml_ng::Value::Sequence(items));
    Ok(())
}

/// Reads the people leniently: a role that isn't one of the six reads as "other", someone without a name is skipped,
/// and the same person in the same role is kept once.
pub fn import_people(conn: &Connection, slug: &str, meta: &Mapping) -> Result<()> {
    let mut list: Vec<(String, String, Option<i64>)> = vec![];
    for item in meta.maps("people") {
        let Some(name) = item.text("name").map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) else { continue };
        let role = item.text("role").map(|r| r.trim().to_lowercase()).filter(|r| ROLES.contains(&r.as_str())).unwrap_or_else(|| "other".into());
        let user = item.text("portflowUser").and_then(|u| u.parse().ok());
        if !list.iter().any(|(n, r, _)| person_tag(r, n) == person_tag(&role, &name)) {
            list.push((name, role, user));
        }
    }
    store_people(conn, slug, &list)
}
