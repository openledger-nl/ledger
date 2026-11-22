//! The clients the vault knows: the organisations work projects are done for. A client is the vault's, so one client
//! can have many projects; a work project names its client by id in its `project.md` (`client: bakkerij-de-korenaar`,
//! see `project_facts`). One file each:
//!
//! ```text
//! shared/clients/<id>.md
//! ---
//! name: Bakkerij De Korenaar
//! website: https://dekorenaar.nl     optional
//! createdAt: …
//! updatedAt: …
//! ---
//!
//! Notes about them, in markdown.
//! ```
//!
//! The id is the name as a slug when the client is added, and stays when it is renamed. A client is not a person: the
//! people who speak for it are attached to the project as stakeholders (see `people`).

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Mapping;

use crate::{db::{query_all, query_opt}, error::{Error, Result}, frontmatter::{self as fm, MetaExt}, ids};

pub const FOLDER: &str = "shared/clients";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Client {
    pub id: String,
    pub name: String,
    pub website: String,
    pub notes: String,
    /// The projects done for them, by name: `(slug, name)`.
    pub projects: Vec<ClientProject>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientProject {
    pub project: String,
    pub project_name: String,
}

/// What can be set on a client.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientDraft {
    pub name: String,
    pub website: String,
    pub notes: String,
}

pub fn file(id: &str) -> String {
    format!("{FOLDER}/{id}.md")
}

const COLUMNS: &str = "id, name, website, notes, created_at, updated_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Client> {
    Ok(Client { id: r.get(0)?, name: r.get(1)?, website: r.get(2)?, notes: r.get(3)?, projects: vec![], created_at: r.get(4)?, updated_at: r.get(5)? })
}

fn with_projects(conn: &Connection, mut client: Client) -> Result<Client> {
    client.projects = query_all(conn, "SELECT slug, name FROM projects WHERE client = ?1 ORDER BY name COLLATE NOCASE, slug", [&client.id], |r| {
        Ok(ClientProject { project: r.get(0)?, project_name: r.get(1)? })
    })?;
    Ok(client)
}

/// Every client, by name.
pub fn list(conn: &Connection) -> Result<Vec<Client>> {
    let clients = query_all(conn, &format!("SELECT {COLUMNS} FROM clients ORDER BY name COLLATE NOCASE, id"), [], row)?;
    clients.into_iter().map(|c| with_projects(conn, c)).collect()
}

pub fn get(conn: &Connection, id: &str) -> Result<Client> {
    let client = query_opt(conn, &format!("SELECT {COLUMNS} FROM clients WHERE id = ?1"), [id], row)?
        .ok_or_else(|| Error::not_found(format!("There is no client {id} in this vault")))?;
    with_projects(conn, client)
}

pub fn exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(query_opt(conn, "SELECT 1 FROM clients WHERE id = ?1", [id], |r| r.get::<_, i64>(0))?.is_some())
}

fn clean(draft: &ClientDraft) -> Result<ClientDraft> {
    let name = draft.name.trim();
    if name.is_empty() {
        return Err(Error::invalid("Give the client a name"));
    }
    Ok(ClientDraft { name: name.to_string(), website: draft.website.trim().to_string(), notes: draft.notes.trim_end().to_string() })
}

/// A new client. Its id is its name as a slug, with a number after it when that is taken.
pub fn create(conn: &Connection, draft: &ClientDraft) -> Result<Client> {
    let draft = clean(draft)?;
    let now = ids::now();
    let id = ids::numbered(&ids::slugify_as(&draft.name, "client"), |id| exists(conn, id))?;
    insert(conn, &id, &draft, &now, &now)?;
    get(conn, &id)
}

fn insert(conn: &Connection, id: &str, d: &ClientDraft, created_at: &str, updated_at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO clients (id, name, website, notes, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, d.name, d.website, d.notes, created_at, updated_at],
    )?;
    Ok(())
}

/// Changes a client. Its id stays as it is.
pub fn update(conn: &Connection, id: &str, draft: &ClientDraft) -> Result<Client> {
    let d = clean(draft)?;
    let changed =
        conn.execute("UPDATE clients SET name = ?2, website = ?3, notes = ?4, updated_at = ?5 WHERE id = ?1", params![id, d.name, d.website, d.notes, ids::now()])?;
    if changed == 0 {
        return Err(Error::not_found(format!("There is no client {id} in this vault")));
    }
    get(conn, id)
}

/// Takes a client out of the vault; its projects keep going without one.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE projects SET client = NULL WHERE client = ?1", [id])?;
    if conn.execute("DELETE FROM clients WHERE id = ?1", [id])? == 0 {
        return Err(Error::not_found(format!("There is no client {id} in this vault")));
    }
    Ok(())
}

// ---- the files ----------------------------------------------------------------------------------------

pub fn render(client: &Client) -> String {
    let mut meta = Mapping::new();
    fm::set(&mut meta, "name", client.name.clone());
    fm::set_opt(&mut meta, "website", Some(&client.website));
    fm::set(&mut meta, "createdAt", client.created_at.clone());
    fm::set(&mut meta, "updatedAt", client.updated_at.clone());
    fm::render(&meta, &client.notes)
}

/// Reads a client's file leniently: one without a name is called after its file, one without frontmatter is all notes.
pub fn import(conn: &Connection, id: &str, meta: &Mapping, notes: &str, stamp: &str) -> Result<()> {
    let draft = ClientDraft {
        name: meta.name_or(id),
        website: meta.text("website").unwrap_or_default(),
        notes: notes.trim_end().to_string(),
    };
    let (created_at, updated_at) = meta.stamps(stamp);
    insert(conn, id, &draft, &created_at, &updated_at)
}
