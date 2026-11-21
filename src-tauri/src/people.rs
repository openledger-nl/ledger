//! The people the vault knows: coaches, teammates, clients and anyone else. A person is the vault's, not one
//! project's, so the same person is one person in every project they are part of; they show only in the projects they
//! are attached to (see `project_facts`), each time with the role they have there. One file each:
//!
//! ```text
//! shared/people/<id>.md
//! ---
//! name: Frank de Vries
//! email: f.devries@example.nl     optional
//! organisation: Fontys ICT        optional
//! portflowUser: 4711              when they are someone in Portflow
//! createdAt: …
//! updatedAt: …
//! ---
//!
//! Notes about them, in markdown.
//! ```
//!
//! The id is the name as a slug when the person is made (`frank-de-vries`), and stays when they are renamed. A person
//! goes by the tag `people/<id>` (`#people/frank-de-vries`): how meetings and feedback name them.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Mapping;

use crate::{
    db::{query_all, query_opt},
    error::{Error, Result},
    frontmatter::{self, MetaExt},
    ids,
};

pub const FOLDER: &str = "shared/people";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub email: String,
    pub organisation: String,
    /// Their Portflow id, when they are someone in Portflow.
    pub portflow_user: Option<i64>,
    pub notes: String,
    /// The projects they are attached to, with their role in each, in the order of the projects' names.
    pub projects: Vec<Attachment>,
    /// `people/<id>`, as meetings and feedback tag them (filled in by Ledger).
    pub tag: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Someone's place in one project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Attachment {
    pub project: String,
    pub project_name: String,
    pub role: String,
}

/// What can be set on a person.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PersonDraft {
    pub name: String,
    pub email: String,
    pub organisation: String,
    pub portflow_user: Option<i64>,
    pub notes: String,
}

/// The tag a person goes by: `people/frank-de-vries`.
pub fn tag(id: &str) -> String {
    format!("people/{id}")
}

pub fn file(id: &str) -> String {
    format!("{FOLDER}/{id}.md")
}

const COLUMNS: &str = "id, name, email, organisation, portflow_user, notes, created_at, updated_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Person> {
    let id: String = r.get(0)?;
    Ok(Person {
        tag: tag(&id),
        id,
        name: r.get(1)?,
        email: r.get(2)?,
        organisation: r.get(3)?,
        portflow_user: r.get(4)?,
        notes: r.get(5)?,
        projects: vec![],
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

fn with_projects(conn: &Connection, mut person: Person) -> Result<Person> {
    person.projects = query_all(
        conn,
        "SELECT pp.project_slug, p.name, pp.role FROM project_people pp JOIN projects p ON p.slug = pp.project_slug
         WHERE pp.person_id = ?1 ORDER BY p.name COLLATE NOCASE, pp.project_slug",
        [&person.id],
        |r| Ok(Attachment { project: r.get(0)?, project_name: r.get(1)?, role: r.get(2)? }),
    )?;
    Ok(person)
}

/// Everyone in the vault, by name.
pub fn list(conn: &Connection) -> Result<Vec<Person>> {
    let people = query_all(conn, &format!("SELECT {COLUMNS} FROM people ORDER BY name COLLATE NOCASE, id"), [], row)?;
    people.into_iter().map(|p| with_projects(conn, p)).collect()
}

pub fn get(conn: &Connection, id: &str) -> Result<Person> {
    let person = query_opt(conn, &format!("SELECT {COLUMNS} FROM people WHERE id = ?1"), [id], row)?
        .ok_or_else(|| Error::not_found(format!("There is no one called {id} in this vault")))?;
    with_projects(conn, person)
}

pub fn exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(query_opt(conn, "SELECT 1 FROM people WHERE id = ?1", [id], |r| r.get::<_, i64>(0))?.is_some())
}

fn clean(draft: &PersonDraft) -> Result<PersonDraft> {
    let name = draft.name.trim();
    if name.is_empty() {
        return Err(Error::invalid("Give them a name"));
    }
    Ok(PersonDraft {
        name: name.to_string(),
        email: draft.email.trim().to_string(),
        organisation: draft.organisation.trim().to_string(),
        portflow_user: draft.portflow_user,
        notes: draft.notes.trim_end().to_string(),
    })
}

/// A new person. Their id is their name as a slug, with a number after it when that is taken.
pub fn create(conn: &Connection, draft: &PersonDraft) -> Result<Person> {
    let draft = clean(draft)?;
    let base = ids::slugify_as(&draft.name, "someone");
    let mut id = base.clone();
    let mut n = 2;
    while exists(conn, &id)? {
        id = format!("{base}-{n}");
        n += 1;
    }
    let now = ids::now();
    insert(conn, &id, &draft, &now, &now)?;
    get(conn, &id)
}

fn insert(conn: &Connection, id: &str, d: &PersonDraft, created_at: &str, updated_at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO people (id, name, email, organisation, portflow_user, notes, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, d.name, d.email, d.organisation, d.portflow_user, d.notes, created_at, updated_at],
    )?;
    Ok(())
}

/// Changes a person. Their id, and so their tag, stays as it is.
pub fn update(conn: &Connection, id: &str, draft: &PersonDraft) -> Result<Person> {
    let d = clean(draft)?;
    let changed = conn.execute(
        "UPDATE people SET name = ?2, email = ?3, organisation = ?4, portflow_user = ?5, notes = ?6, updated_at = ?7 WHERE id = ?1",
        params![id, d.name, d.email, d.organisation, d.portflow_user, d.notes, ids::now()],
    )?;
    if changed == 0 {
        return Err(Error::not_found(format!("There is no one called {id} in this vault")));
    }
    get(conn, id)
}

/// Takes someone out of the vault and out of every project they were in. Meetings and feedback that name them keep
/// their name as it was written.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let removed = conn.execute("DELETE FROM people WHERE id = ?1", [id])?;
    if removed == 0 {
        return Err(Error::not_found(format!("There is no one called {id} in this vault")));
    }
    Ok(())
}

// ---- the files ----------------------------------------------------------------------------------------

/// The person's file: their frontmatter and their notes.
pub fn render(person: &Person) -> String {
    let mut meta = Mapping::new();
    frontmatter::set(&mut meta, "name", person.name.clone());
    frontmatter::set_opt(&mut meta, "email", Some(&person.email));
    frontmatter::set_opt(&mut meta, "organisation", Some(&person.organisation));
    if let Some(user) = person.portflow_user {
        frontmatter::set(&mut meta, "portflowUser", user);
    }
    frontmatter::set(&mut meta, "createdAt", person.created_at.clone());
    frontmatter::set(&mut meta, "updatedAt", person.updated_at.clone());
    frontmatter::render(&meta, &person.notes)
}

/// Reads a person's file leniently: one without a name is called after its file, and one without frontmatter is all
/// notes.
pub fn import(conn: &Connection, id: &str, meta: &Mapping, notes: &str, stamp: &str) -> Result<()> {
    let draft = PersonDraft {
        name: meta.text("name").map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| id.to_string()),
        email: meta.text("email").unwrap_or_default(),
        organisation: meta.text("organisation").unwrap_or_default(),
        portflow_user: meta.text("portflowUser").and_then(|u| u.trim().parse().ok()),
        notes: notes.trim_end().to_string(),
    };
    let created_at = meta.text("createdAt").unwrap_or_else(|| stamp.to_string());
    let updated_at = meta.text("updatedAt").unwrap_or_else(|| created_at.clone());
    insert(conn, id, &draft, &created_at, &updated_at)
}
