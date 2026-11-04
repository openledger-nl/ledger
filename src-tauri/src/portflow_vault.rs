//! Portflow's files in the vault, read into the database and written back out (called from `vault.rs`).
//!
//! ```text
//! portflow/outcomes/<id>-<slug>.md          a learning outcome and its sub-goals, mirrored from Portflow
//! portflow/people.md                        the people the portfolio is shared with, mirrored from Portflow
//! portflow/sections.md                      the portfolio's sections (its semesters), mirrored from Portflow
//! projects/<p>/portflow/<semester>.md       a linked collection: name, subtitle, outcomes, and its description
//! projects/<p>/evidence/<slug>.md           an evidence item: its files, thread, outcomes, and Portflow's versions
//!                                           (by file name; a version that came from Ledger names its deliverable)
//! ```
//!
//! These are only read while Portflow is switched on (see `vault::import`); switched off, the tables stay empty, so
//! nothing here is written or deleted either and the files sit untouched in the vault.
//!
//! Two things are kept whether Portflow is on or not, because they live in files Ledger always writes: a project's
//! `portflowCollections` (in `vault.rs`) and who gave a piece of feedback ([`FeedbackExtra`]).

use std::collections::{BTreeMap, HashMap};

use rusqlite::{Connection, params};
use serde_yaml_ng::{Mapping, Value};

use crate::db::query_all;
use crate::error::Result;
use crate::frontmatter::{self, MetaExt};
use crate::ids;
use crate::vault::Source;

pub const MIRROR_NOTE: &str = "Mirrored from Portflow: edits here are overwritten by the next sync.";

pub fn outcome_file(id: i64, name: &str) -> String {
    format!("portflow/outcomes/{id}-{}.md", ids::slugify_as(name, "outcome"))
}

pub const PEOPLE_FILE: &str = "portflow/people.md";
pub const SECTIONS_FILE: &str = "portflow/sections.md";

pub fn collection_file(project: &str, semester: &str) -> String {
    format!("projects/{project}/portflow/{semester}.md")
}

pub fn evidence_file(project: &str, slug: &str) -> String {
    format!("projects/{project}/evidence/{slug}.md")
}

fn int(meta: &Mapping, key: &str) -> Option<i64> {
    match meta.get(key)? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn ints(meta: &Mapping, key: &str) -> Vec<i64> {
    meta.list(key).iter().filter_map(|s| s.trim().parse().ok()).collect()
}

fn set_int(meta: &mut Mapping, key: &str, value: Option<i64>) {
    if let Some(value) = value {
        frontmatter::set(meta, key, Value::Number(value.into()));
    }
}

fn set_ints(meta: &mut Mapping, key: &str, values: &[i64]) {
    if !values.is_empty() {
        frontmatter::set(meta, key, Value::Sequence(values.iter().map(|v| Value::Number((*v).into())).collect()));
    }
}

// ---- who gave feedback (always kept) ------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedbackExtra {
    pub from: Option<String>,
    pub from_user: Option<i64>,
    pub from_role: Option<String>,
    pub given: Option<String>,
    pub portflow_comment: Option<i64>,
    pub reply_to: Option<String>,
    pub comment_type: Option<String>,
    pub evidence: Option<String>,
    pub evidence_version: Option<i64>,
}

impl FeedbackExtra {
    pub fn read(meta: &Mapping) -> FeedbackExtra {
        FeedbackExtra {
            from: meta.text("from"),
            from_user: int(meta, "fromPortflowUser"),
            from_role: meta.text("fromRole"),
            given: meta.text("given"),
            portflow_comment: int(meta, "portflowComment"),
            reply_to: meta.text("replyTo"),
            comment_type: meta.text("commentType"),
            evidence: meta.text("evidence"),
            evidence_version: int(meta, "evidenceVersion"),
        }
    }

    pub fn write(&self, meta: &mut Mapping) {
        frontmatter::set_opt(meta, "from", self.from.as_deref());
        set_int(meta, "fromPortflowUser", self.from_user);
        frontmatter::set_opt(meta, "fromRole", self.from_role.as_deref());
        frontmatter::set_opt(meta, "given", self.given.as_deref());
        set_int(meta, "portflowComment", self.portflow_comment);
        frontmatter::set_opt(meta, "replyTo", self.reply_to.as_deref());
        frontmatter::set_opt(meta, "commentType", self.comment_type.as_deref());
        frontmatter::set_opt(meta, "evidence", self.evidence.as_deref());
        set_int(meta, "evidenceVersion", self.evidence_version);
    }

    pub fn is_empty(&self) -> bool {
        *self == FeedbackExtra::default()
    }

    pub fn store(&self, conn: &Connection, id: &str) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }
        // A second file claiming a comment that another already holds keeps its text but not the claim.
        let comment = match self.portflow_comment {
            Some(c) => {
                let taken: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM feedback WHERE portflow_comment = ?1 AND id <> ?2)", params![c, id], |r| r.get(0))?;
                if taken { None } else { Some(c) }
            }
            None => None,
        };
        conn.execute(
            "UPDATE feedback SET from_name = ?2, from_user = ?3, from_role = ?4, given = ?5, portflow_comment = ?6, reply_to = ?7,
                 comment_type = ?8, evidence = ?9, evidence_version = ?10 WHERE id = ?1",
            params![id, self.from, self.from_user, self.from_role, self.given, comment, self.reply_to, self.comment_type, self.evidence, self.evidence_version],
        )?;
        Ok(())
    }
}

pub fn feedback_extras(conn: &Connection) -> Result<HashMap<String, FeedbackExtra>> {
    let rows = query_all(
        conn,
        "SELECT id, from_name, from_user, from_role, given, portflow_comment, reply_to, comment_type, evidence, evidence_version FROM feedback",
        [],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                FeedbackExtra {
                    from: r.get(1)?,
                    from_user: r.get(2)?,
                    from_role: r.get(3)?,
                    given: r.get(4)?,
                    portflow_comment: r.get(5)?,
                    reply_to: r.get(6)?,
                    comment_type: r.get(7)?,
                    evidence: r.get(8)?,
                    evidence_version: r.get(9)?,
                },
            ))
        },
    )?;
    Ok(rows.into_iter().filter(|(_, e)| !e.is_empty()).collect())
}

// ---- export: database → files -------------------------------------------------------------------------

pub fn export(conn: &Connection, files: &mut BTreeMap<String, String>) -> Result<()> {
    export_outcomes(conn, files)?;
    export_people(conn, files)?;
    export_sections(conn, files)?;
    export_collections(conn, files)?;
    export_evidence(conn, files)?;
    Ok(())
}

fn export_outcomes(conn: &Connection, files: &mut BTreeMap<String, String>) -> Result<()> {
    let mut subs: HashMap<i64, Vec<Value>> = HashMap::new();
    for (outcome, id, name, nickname, description, evidence_count) in query_all(
        conn,
        "SELECT outcome_id, id, name, nickname, description, evidence_count FROM sub_outcomes ORDER BY outcome_id, position, id",
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, i64>(5)?)),
    )? {
        let mut item = Mapping::new();
        set_int(&mut item, "id", Some(id));
        frontmatter::set(&mut item, "name", name);
        frontmatter::set_opt(&mut item, "nickname", Some(&nickname));
        set_int(&mut item, "evidenceCount", Some(evidence_count).filter(|n| *n > 0));
        frontmatter::set_opt(&mut item, "description", Some(&description));
        subs.entry(outcome).or_default().push(Value::Mapping(item));
    }

    type Row = (i64, String, String, i64, Option<i64>, String, String, String, bool, Option<i64>);
    for (id, name, nickname, position, set_id, set_name, status, explanation, removed, import_id) in query_all::<Row, _>(
        conn,
        "SELECT id, name, nickname, position, goal_set_id, goal_set, status, explanation, removed, import_id FROM outcomes",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)),
    )? {
        let mut meta = Mapping::new();
        frontmatter::set(&mut meta, "mirror", MIRROR_NOTE);
        set_int(&mut meta, "id", Some(id));
        frontmatter::set(&mut meta, "name", name.clone());
        frontmatter::set_opt(&mut meta, "nickname", Some(&nickname));
        set_int(&mut meta, "position", Some(position));
        set_int(&mut meta, "goalSetId", set_id);
        frontmatter::set_opt(&mut meta, "goalSet", Some(&set_name));
        set_int(&mut meta, "import", import_id);
        frontmatter::set_opt(&mut meta, "status", Some(&status));
        frontmatter::set_flag(&mut meta, "removed", removed);
        if let Some(subs) = subs.remove(&id) {
            frontmatter::set(&mut meta, "subOutcomes", Value::Sequence(subs));
        }
        files.insert(outcome_file(id, &name), frontmatter::render(&meta, &explanation));
    }
    Ok(())
}

fn export_people(conn: &Connection, files: &mut BTreeMap<String, String>) -> Result<()> {
    let people = query_all(conn, "SELECT id, name, role, share_type, expires FROM portflow_people ORDER BY position, name", [], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?))
    })?;
    if people.is_empty() {
        return Ok(());
    }
    let items = people
        .into_iter()
        .map(|(id, name, role, share, expires)| {
            let mut item = Mapping::new();
            set_int(&mut item, "id", Some(id));
            frontmatter::set(&mut item, "name", name);
            frontmatter::set_opt(&mut item, "role", Some(&role));
            frontmatter::set_opt(&mut item, "shared", Some(&share));
            frontmatter::set_opt(&mut item, "until", expires.as_deref());
            Value::Mapping(item)
        })
        .collect();
    let mut meta = Mapping::new();
    frontmatter::set(&mut meta, "mirror", MIRROR_NOTE);
    frontmatter::set(&mut meta, "people", Value::Sequence(items));
    files.insert(PEOPLE_FILE.to_string(), frontmatter::render(&meta, "The people your Portflow portfolio is shared with."));
    Ok(())
}

fn export_sections(conn: &Connection, files: &mut BTreeMap<String, String>) -> Result<()> {
    type Row = (i64, String, i64, Option<i64>, String, Option<String>);
    let sections = query_all::<Row, _>(conn, "SELECT id, name, position, import_id, template, imported_at FROM portflow_sections ORDER BY position, id", [], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
    })?;
    if sections.is_empty() {
        return Ok(());
    }
    let items = sections
        .into_iter()
        .map(|(id, name, position, import_id, template, imported_at)| {
            let mut item = Mapping::new();
            set_int(&mut item, "id", Some(id));
            frontmatter::set(&mut item, "name", name);
            set_int(&mut item, "position", Some(position));
            set_int(&mut item, "import", import_id);
            frontmatter::set_opt(&mut item, "template", Some(&template));
            frontmatter::set_opt(&mut item, "importedAt", imported_at.as_deref());
            Value::Mapping(item)
        })
        .collect();
    let mut meta = Mapping::new();
    frontmatter::set(&mut meta, "mirror", MIRROR_NOTE);
    frontmatter::set(&mut meta, "sections", Value::Sequence(items));
    files.insert(
        SECTIONS_FILE.to_string(),
        frontmatter::render(&meta, "The sections of your Portflow portfolio: one per semester. Which Ledger semester each is, is in that semester's file (`portflowSection`)."),
    );
    Ok(())
}

fn export_collections(conn: &Connection, files: &mut BTreeMap<String, String>) -> Result<()> {
    let mut outcomes: HashMap<(String, String), Vec<i64>> = HashMap::new();
    for (project, semester, outcome) in query_all(conn, "SELECT project_slug, semester_id, outcome_id FROM collection_outcomes ORDER BY position", [], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
    })? {
        outcomes.entry((project, semester)).or_default().push(outcome);
    }
    type Row = (String, String, i64, String, String, String, String, String);
    for (project, semester, collection, name, subtitle, section, color, description) in query_all::<Row, _>(
        conn,
        "SELECT project_slug, semester_id, collection_id, name, subtitle, section, color, description FROM collection_mirrors",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
    )? {
        let mut meta = Mapping::new();
        frontmatter::set(&mut meta, "mirror", MIRROR_NOTE);
        set_int(&mut meta, "collection", Some(collection));
        frontmatter::set(&mut meta, "name", name);
        frontmatter::set_opt(&mut meta, "subtitle", Some(&subtitle));
        frontmatter::set_opt(&mut meta, "section", Some(&section));
        frontmatter::set_opt(&mut meta, "color", Some(&color));
        set_ints(&mut meta, "outcomes", &outcomes.remove(&(project.clone(), semester.clone())).unwrap_or_default());
        files.insert(collection_file(&project, &semester), frontmatter::render(&meta, &description));
    }
    Ok(())
}

fn export_evidence(conn: &Connection, files: &mut BTreeMap<String, String>) -> Result<()> {
    type Key = (String, String);
    let mut file_lists: HashMap<Key, Vec<String>> = HashMap::new();
    for (p, e, path) in query_all(conn, "SELECT project_slug, evidence_slug, path FROM evidence_files ORDER BY position", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))? {
        file_lists.entry((p, e)).or_default().push(path);
    }
    let mut outcome_lists: HashMap<Key, Vec<i64>> = HashMap::new();
    for (p, e, id) in query_all(conn, "SELECT project_slug, evidence_slug, outcome_id FROM evidence_outcomes ORDER BY outcome_id", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))? {
        outcome_lists.entry((p, e)).or_default().push(id);
    }
    let mut collection_maps: HashMap<Key, Mapping> = HashMap::new();
    for (p, e, sem, c) in query_all(conn, "SELECT project_slug, evidence_slug, semester_id, collection_id FROM evidence_collections ORDER BY semester_id", [], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))
    })? {
        collection_maps.entry((p, e)).or_default().insert(Value::String(sem), Value::Number(c.into()));
    }
    let mut version_files: HashMap<(String, String, i64), Vec<Value>> = HashMap::new();
    type FRow = (String, String, i64, String, Option<i64>, Option<String>);
    for (p, e, n, name, portflow_file, path) in query_all::<FRow, _>(
        conn,
        "SELECT project_slug, evidence_slug, number, name, portflow_file, path FROM evidence_version_files ORDER BY name",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
    )? {
        let mut item = Mapping::new();
        frontmatter::set(&mut item, "name", name);
        set_int(&mut item, "portflowFile", portflow_file);
        frontmatter::set_opt(&mut item, "deliverable", path.as_deref());
        version_files.entry((p, e, n)).or_default().push(Value::Mapping(item));
    }
    let mut versions: HashMap<Key, Vec<Value>> = HashMap::new();
    type VRow = (String, String, i64, Option<i64>, String, String, String);
    for (p, e, n, pv, kind, description, created_at) in query_all::<VRow, _>(
        conn,
        "SELECT project_slug, evidence_slug, number, portflow_version, kind, description, created_at FROM evidence_versions ORDER BY number",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
    )? {
        let mut item = Mapping::new();
        set_int(&mut item, "number", Some(n));
        set_int(&mut item, "portflowVersion", pv);
        frontmatter::set(&mut item, "type", kind);
        frontmatter::set(&mut item, "createdAt", created_at);
        if let Some(files) = version_files.remove(&(p.clone(), e.clone(), n)) {
            frontmatter::set(&mut item, "files", Value::Sequence(files));
        }
        frontmatter::set_opt(&mut item, "description", Some(&description));
        versions.entry((p, e)).or_default().push(Value::Mapping(item));
    }

    type Row = (String, String, String, Option<String>, Option<i64>, String, String, String, String);
    for (project, slug, name, thread, portflow, kind, description, created_at, updated_at) in query_all::<Row, _>(
        conn,
        "SELECT project_slug, slug, name, thread_slug, portflow_id, kind, description, created_at, updated_at FROM evidence",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)),
    )? {
        let key = (project.clone(), slug.clone());
        let mut meta = Mapping::new();
        frontmatter::set(&mut meta, "name", name);
        frontmatter::set_opt(&mut meta, "thread", thread.as_deref());
        frontmatter::set_list(&mut meta, "files", &file_lists.remove(&key).unwrap_or_default());
        set_ints(&mut meta, "outcomes", &outcome_lists.remove(&key).unwrap_or_default());
        if let Some(collections) = collection_maps.remove(&key) {
            frontmatter::set(&mut meta, "collections", Value::Mapping(collections));
        }
        set_int(&mut meta, "portflowEvidence", portflow);
        frontmatter::set(&mut meta, "type", kind);
        frontmatter::set(&mut meta, "createdAt", created_at);
        frontmatter::set(&mut meta, "updatedAt", updated_at);
        if let Some(versions) = versions.remove(&key) {
            frontmatter::set(&mut meta, "versions", Value::Sequence(versions));
        }
        files.insert(evidence_file(&project, &slug), frontmatter::render(&meta, &description));
    }
    Ok(())
}

// ---- import: files → database -------------------------------------------------------------------------

pub(crate) fn import(conn: &Connection, source: &Source, projects: &[String]) -> Result<()> {
    import_outcomes(conn, source)?;
    import_people(conn, source)?;
    import_sections(conn, source)?;
    for project in projects {
        import_collections(conn, source, project)?;
        import_evidence(conn, source, project)?;
    }
    Ok(())
}

fn import_outcomes(conn: &Connection, source: &Source) -> Result<()> {
    for stem in source.stems("portflow/outcomes") {
        let rel = format!("portflow/outcomes/{stem}.md");
        let Some((meta, explanation)) = source.parsed(&rel) else { continue };
        let Some(id) = int(&meta, "id").or_else(|| stem.split('-').next().and_then(|n| n.parse().ok())) else { continue };
        let name = meta.text("name").unwrap_or_else(|| stem.clone());
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO outcomes (id, name, nickname, position, goal_set_id, goal_set, status, explanation, removed, synced_at, import_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                name,
                meta.text("nickname").unwrap_or_default(),
                int(&meta, "position").unwrap_or(0),
                int(&meta, "goalSetId"),
                meta.text("goalSet").unwrap_or_default(),
                meta.text("status").unwrap_or_default(),
                explanation,
                meta.flag("removed"),
                source.stamp(&rel),
                int(&meta, "import"),
            ],
        )?;
        if inserted == 0 {
            continue;
        }
        for (position, sub) in meta.maps("subOutcomes").into_iter().enumerate() {
            let Some(sub_id) = int(sub, "id") else { continue };
            conn.execute(
                "INSERT OR IGNORE INTO sub_outcomes (id, outcome_id, name, nickname, position, description, evidence_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    sub_id,
                    id,
                    sub.text("name").unwrap_or_else(|| sub_id.to_string()),
                    sub.text("nickname").unwrap_or_default(),
                    position as i64,
                    sub.text("description").unwrap_or_default(),
                    int(sub, "evidenceCount").unwrap_or(0)
                ],
            )?;
        }
    }
    Ok(())
}

fn import_people(conn: &Connection, source: &Source) -> Result<()> {
    let Some((meta, _)) = source.parsed(PEOPLE_FILE) else { return Ok(()) };
    for (position, person) in meta.maps("people").into_iter().enumerate() {
        let (Some(id), Some(name)) = (int(person, "id"), person.text("name")) else { continue };
        conn.execute(
            "INSERT OR IGNORE INTO portflow_people (id, name, role, share_type, expires, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, person.text("role").unwrap_or_default(), person.text("shared").unwrap_or_default(), person.text("until"), position as i64],
        )?;
    }
    Ok(())
}

fn import_sections(conn: &Connection, source: &Source) -> Result<()> {
    let Some((meta, _)) = source.parsed(SECTIONS_FILE) else { return Ok(()) };
    for section in meta.maps("sections") {
        let (Some(id), Some(name)) = (int(section, "id"), section.text("name")) else { continue };
        conn.execute(
            "INSERT OR IGNORE INTO portflow_sections (id, name, position, import_id, template, imported_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, int(section, "position").unwrap_or(0), int(section, "import"), section.text("template").unwrap_or_default(), section.text("importedAt")],
        )?;
    }
    Ok(())
}

fn import_collections(conn: &Connection, source: &Source, project: &str) -> Result<()> {
    for semester in source.stems(&format!("projects/{project}/portflow")) {
        let rel = collection_file(project, &semester);
        let Some((meta, description)) = source.parsed(&rel) else { continue };
        let Some(collection) = int(&meta, "collection") else { continue };
        conn.execute(
            "INSERT OR IGNORE INTO collection_mirrors (project_slug, semester_id, collection_id, name, subtitle, section, color, description, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                project,
                semester,
                collection,
                meta.text("name").unwrap_or_else(|| semester.clone()),
                meta.text("subtitle").unwrap_or_default(),
                meta.text("section").unwrap_or_default(),
                meta.text("color").unwrap_or_default(),
                description,
                source.stamp(&rel),
            ],
        )?;
        for (position, outcome) in ints(&meta, "outcomes").into_iter().enumerate() {
            conn.execute(
                "INSERT OR IGNORE INTO collection_outcomes (project_slug, semester_id, outcome_id, position) VALUES (?1, ?2, ?3, ?4)",
                params![project, semester, outcome, position as i64],
            )?;
        }
    }
    Ok(())
}

fn import_evidence(conn: &Connection, source: &Source, project: &str) -> Result<()> {
    for slug in source.stems(&format!("projects/{project}/evidence")) {
        let rel = evidence_file(project, &slug);
        let Some((meta, description)) = source.parsed(&rel) else { continue };
        let created_at = meta.text("createdAt").unwrap_or_else(|| source.stamp(&rel));
        // Two files claiming one Portflow item: the first keeps the link.
        let portflow = int(&meta, "portflowEvidence").filter(|id| {
            !conn.query_row("SELECT EXISTS(SELECT 1 FROM evidence WHERE portflow_id = ?1)", [id], |r| r.get::<_, bool>(0)).unwrap_or(true)
        });
        conn.execute(
            "INSERT OR IGNORE INTO evidence (project_slug, slug, name, thread_slug, portflow_id, kind, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                project,
                slug,
                meta.text("name").unwrap_or_else(|| slug.clone()),
                meta.text("thread"),
                portflow,
                meta.text("type").unwrap_or_else(|| "file".into()),
                description,
                created_at.clone(),
                meta.text("updatedAt").unwrap_or(created_at),
            ],
        )?;
        for (position, path) in meta.list("files").into_iter().enumerate() {
            // A file is in one evidence item at most: a second claim is dropped.
            conn.execute(
                "INSERT OR IGNORE INTO evidence_files (project_slug, evidence_slug, position, path) VALUES (?1, ?2, ?3, ?4)",
                params![project, slug, position as i64, path],
            )?;
        }
        for outcome in ints(&meta, "outcomes") {
            conn.execute("INSERT OR IGNORE INTO evidence_outcomes (project_slug, evidence_slug, outcome_id) VALUES (?1, ?2, ?3)", params![project, slug, outcome])?;
        }
        if let Some(Value::Mapping(collections)) = meta.get("collections") {
            for (semester, collection) in collections {
                let (Some(semester), Some(collection)) = (semester.as_str(), collection.as_i64()) else { continue };
                conn.execute(
                    "INSERT OR IGNORE INTO evidence_collections (project_slug, evidence_slug, semester_id, collection_id) VALUES (?1, ?2, ?3, ?4)",
                    params![project, slug, semester, collection],
                )?;
            }
        }
        for version in meta.maps("versions") {
            let Some(number) = int(version, "number").filter(|n| *n >= 0) else { continue };
            let inserted = conn.execute(
                "INSERT OR IGNORE INTO evidence_versions (project_slug, evidence_slug, number, portflow_version, kind, description, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    project,
                    slug,
                    number,
                    int(version, "portflowVersion"),
                    version.text("type").unwrap_or_else(|| "file".into()),
                    version.text("description").unwrap_or_default(),
                    version.text("createdAt").unwrap_or_else(|| source.stamp(&rel)),
                ],
            )?;
            if inserted == 0 {
                continue;
            }
            for file in version.maps("files") {
                let Some(name) = file.text("name") else { continue };
                conn.execute(
                    "INSERT OR IGNORE INTO evidence_version_files (project_slug, evidence_slug, number, name, portflow_file, path) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![project, slug, number, name, int(file, "portflowFile"), file.text("deliverable")],
                )?;
            }
        }
    }
    Ok(())
}
