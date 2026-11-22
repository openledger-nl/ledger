//! What a project is, beyond its name: its kind, its goal, when it ends and the AI-use label its work is meant to get.
//! Kept in the frontmatter of the project's `project.md`:
//!
//! ```text
//! kind: study                  study, work or hobby
//! goal: Give the bakery a rota tool their staff can use on a phone.
//! end: semester                semester, a calendar day (2027-01-31), or left out for no end
//! semester: NJ26               the semester a study project runs in
//! intendedAiUse: cyborg        the label the work is meant to get
//! mainQuestion: How can …?     a study project's main question
//! client: bakery-bos           a work project's client, by its id (see `clients`)
//! ```
//!
//! A project without a kind and a goal has no facts yet: the screens then ask for them.
//!
//! Who is part of it is listed there too, whether or not it has facts: people of the vault (see `people`), by their id,
//! each with the role they have in this project:
//!
//! ```text
//! people:
//! - person: frank-de-vries
//!   role: coach
//! - person: sanne
//!   role: teammate
//! ```
//!
//! A person goes by the tag `people/<id>` (`people/frank-de-vries`) in every project: how meetings and feedback name
//! them.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Mapping;

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::frontmatter::{self, MetaExt};
use crate::{ids, projects, provenance};

pub const KINDS: [&str; 3] = ["study", "work", "hobby"];
pub const ROLES: [&str; 6] = ["coach", "expert", "teammate", "stakeholder", "colleague", "other"];

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
    /// A work project's client, by its id (see `clients`).
    pub client: Option<String>,
    /// That client's name (filled in by Ledger).
    pub client_name: Option<String>,
    /// How daily notes and feedback tag the project: `project/<tag>`. Its slug.
    pub tag: String,
}

const COLUMNS: &str = "slug, kind, goal, project_end, semester, intended_mmm, main_question, client, (SELECT name FROM clients c WHERE c.id = projects.client)";

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
        client_name: r.get(8)?,
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
        return Err(Error::invalid("A project is a study, work or hobby project"));
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
        client_name: None,
        tag: meta.slug.clone(),
    })
}

/// Sets the project's facts (all of them: what is left out is cleared).
pub fn save(conn: &Connection, slug: &str, meta: &ProjectMeta) -> Result<ProjectMeta> {
    projects::require(conn, slug)?;
    let m = clean(&ProjectMeta { slug: slug.to_string(), ..meta.clone() })?;
    if let Some(client) = &m.client
        && !crate::clients::exists(conn, client)?
    {
        return Err(Error::invalid(format!("There is no client {client} in this vault: add it under Clients first")));
    }
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
    frontmatter::set_opt(meta, "intendedAiUse", m.intended_mmm.as_deref());
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
    let intended = meta.text("intendedAiUse").and_then(|l| provenance::level(l.trim()).map(str::to_string));
    // A client that isn't in the vault (any more) is no client.
    let client = match meta.text("client").map(|c| c.trim().to_string()).filter(|c| !c.is_empty()) {
        Some(id) if crate::clients::exists(conn, &id)? => Some(id),
        _ => None,
    };
    conn.execute(
        "UPDATE projects SET kind = ?2, goal = ?3, project_end = ?4, semester = ?5, intended_mmm = ?6, main_question = ?7, client = ?8 WHERE slug = ?1",
        params![slug, kind, goal, end, meta.text("semester"), intended, meta.text("mainQuestion"), client],
    )?;
    Ok(())
}

// ---- its people -------------------------------------------------------------------------------------

/// Someone of the vault as part of one project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectPerson {
    /// The project they are part of.
    pub project: String,
    /// Their id in the vault (see `people`).
    pub person: String,
    /// coach, expert, teammate, stakeholder, colleague or other: their role in this project.
    pub role: String,
    /// Their name, from the vault (filled in by Ledger).
    pub name: String,
    /// Their Portflow id, when they are someone in Portflow (filled in by Ledger).
    pub portflow_user: Option<i64>,
    /// `people/<id>`, as meetings and feedback tag them (filled in by Ledger).
    pub tag: String,
}

fn person_row(r: &rusqlite::Row) -> rusqlite::Result<ProjectPerson> {
    let person: String = r.get(1)?;
    Ok(ProjectPerson { project: r.get(0)?, tag: crate::people::tag(&person), person, role: r.get(2)?, name: r.get(3)?, portflow_user: r.get(4)? })
}

/// The people of one project, or of every project with `None`, in the order they are listed.
pub fn people(conn: &Connection, project: Option<&str>) -> Result<Vec<ProjectPerson>> {
    let sql = "SELECT pp.project_slug, pp.person_id, pp.role, p.name, p.portflow_user FROM project_people pp JOIN people p ON p.id = pp.person_id";
    match project {
        Some(slug) => {
            projects::require(conn, slug)?;
            query_all(conn, &format!("{sql} WHERE pp.project_slug = ?1 ORDER BY pp.position"), [slug], person_row)
        }
        None => query_all(conn, &format!("{sql} ORDER BY pp.project_slug, pp.position"), [], person_row),
    }
}

/// Replaces who is part of the project: people of the vault by their id, each with one of the roles. Someone listed
/// twice is part of it once, with the role they were listed with first.
pub fn save_people(conn: &Connection, slug: &str, people: &[ProjectPerson]) -> Result<Vec<ProjectPerson>> {
    projects::require(conn, slug)?;
    let mut cleaned: Vec<(String, String)> = vec![];
    for p in people {
        let id = p.person.trim();
        if !crate::people::exists(conn, id)? {
            return Err(Error::invalid(format!("There is no one called {id} in this vault: add them under People first")));
        }
        let role = p.role.trim().to_lowercase();
        if !ROLES.contains(&role.as_str()) {
            return Err(Error::invalid("A person is a coach, expert, teammate, stakeholder, colleague or other"));
        }
        if !cleaned.iter().any(|(other, _)| other == id) {
            cleaned.push((id.to_string(), role));
        }
    }
    store_people(conn, slug, &cleaned)?;
    conn.execute("UPDATE projects SET updated_at = ?2 WHERE slug = ?1", params![slug, ids::now()])?;
    self::people(conn, Some(slug))
}

fn store_people(conn: &Connection, slug: &str, people: &[(String, String)]) -> Result<()> {
    conn.execute("DELETE FROM project_people WHERE project_slug = ?1", [slug])?;
    for (position, (person, role)) in people.iter().enumerate() {
        conn.execute(
            "INSERT INTO project_people (project_slug, position, person_id, role) VALUES (?1, ?2, ?3, ?4)",
            params![slug, position as i64, person, role],
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
            frontmatter::set(&mut item, "person", p.person);
            frontmatter::set(&mut item, "role", p.role);
            serde_yaml_ng::Value::Mapping(item)
        })
        .collect();
    frontmatter::set(meta, "people", serde_yaml_ng::Value::Sequence(items));
    Ok(())
}

/// Reads who is part of the project leniently: a role that isn't one of the six reads as "other", and someone who
/// isn't in the vault (any more) or is listed twice is left out.
pub fn import_people(conn: &Connection, slug: &str, meta: &Mapping) -> Result<()> {
    let mut list: Vec<(String, String)> = vec![];
    for item in meta.maps("people") {
        let Some(id) = item.text("person").map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) else { continue };
        if !crate::people::exists(conn, &id)? || list.iter().any(|(other, _)| *other == id) {
            continue;
        }
        let role = item.text("role").map(|r| r.trim().to_lowercase()).filter(|r| ROLES.contains(&r.as_str())).unwrap_or_else(|| "other".into());
        list.push((id, role));
    }
    store_people(conn, slug, &list)
}
