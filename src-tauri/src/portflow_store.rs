//! What Portflow holds, kept in the database (and so, through the vault, in files): the learning outcomes, the people
//! the portfolio is shared with, the linked collections, the evidence items with their versions, and the comments as
//! feedback. The sync (`portflow_sync.rs`) fetches; this applies what it fetched, and answers the screens.
//!
//! Applying is idempotent: what is stored is a function of what Portflow said, so a round that finds nothing new
//! changes no row and writes no file (and so gives the cloud storage nothing to upload).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::portflow_api::{Collection, CollectionGoal, Comment, Evidence, Goal, GoalSet, Share, SubGoal, Version};
use crate::portflow_html;
use crate::{ids, markdown, projects, threads};

// ---- outcomes -----------------------------------------------------------------------------------------

/// Replaces the outcomes mirror with what Portflow has. One that is gone is kept, marked removed.
pub fn store_outcomes(conn: &Connection, goals: &[(Goal, Vec<SubGoal>)], sets: &[GoalSet]) -> Result<()> {
    let set_of: HashMap<i64, &GoalSet> = sets.iter().flat_map(|s| s.goal_ids.iter().map(move |g| (*g, s))).collect();
    let now = ids::now();
    let mut seen = BTreeSet::new();
    for (goal, subs) in goals {
        seen.insert(goal.id);
        let (set_id, set_name) = match (&goal.goal_set, set_of.get(&goal.id)) {
            (Some(set), _) => (Some(set.id), set.name.clone()),
            (None, Some(set)) => (Some(set.id), set.name.clone()),
            (None, None) => (None, String::new()),
        };
        let explanation = portflow_html::to_markdown(goal.explanation.as_deref().or(goal.description.as_deref()).unwrap_or_default());
        let import_id = goal.last_import.as_ref().and_then(|i| i.id);
        let row = (goal.name.clone(), goal.nickname.clone().unwrap_or_default(), goal.position, set_id, set_name, goal.status.clone().unwrap_or_default(), explanation, import_id);
        type Row = (String, String, i64, Option<i64>, String, String, String, Option<i64>);
        let current = query_opt(conn, "SELECT name, nickname, position, goal_set_id, goal_set, status, explanation, import_id, removed FROM outcomes WHERE id = ?1", [goal.id], |r| {
            let row: Row = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?);
            Ok((row, r.get::<_, bool>(8)?))
        })?;
        if current.as_ref() != Some(&(row.clone(), false)) {
            conn.execute(
                "INSERT INTO outcomes (id, name, nickname, position, goal_set_id, goal_set, status, explanation, import_id, removed, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)
                 ON CONFLICT(id) DO UPDATE SET name = ?2, nickname = ?3, position = ?4, goal_set_id = ?5, goal_set = ?6, status = ?7,
                     explanation = ?8, import_id = ?9, removed = 0, synced_at = ?10",
                params![goal.id, row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, now],
            )?;
        }
        type Sub = (i64, String, String, i64, String, i64);
        let wanted: Vec<Sub> = subs
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, s.name.clone(), s.nickname.clone().unwrap_or_default(), i as i64, portflow_html::to_markdown(s.description.as_deref().unwrap_or_default()), s.evidence_count))
            .collect();
        let have: Vec<Sub> = query_all(
            conn,
            "SELECT id, name, nickname, position, description, evidence_count FROM sub_outcomes WHERE outcome_id = ?1 ORDER BY position, id",
            [goal.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )?;
        if have != wanted {
            conn.execute("DELETE FROM sub_outcomes WHERE outcome_id = ?1", [goal.id])?;
            for (id, name, nickname, position, description, evidence_count) in wanted {
                conn.execute(
                    "INSERT OR REPLACE INTO sub_outcomes (id, outcome_id, name, nickname, position, description, evidence_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![id, goal.id, name, nickname, position, description, evidence_count],
                )?;
            }
        }
    }
    let known: Vec<i64> = query_all(conn, "SELECT id FROM outcomes WHERE removed = 0", [], |r| r.get(0))?;
    for id in known.into_iter().filter(|id| !seen.contains(id)) {
        conn.execute("UPDATE outcomes SET removed = 1, synced_at = ?2 WHERE id = ?1", params![id, now])?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub project: String,
    pub project_name: String,
    pub slug: String,
    pub name: String,
    pub thread: Option<String>,
}

pub(crate) fn evidence_for(conn: &Connection) -> Result<HashMap<i64, Vec<EvidenceRef>>> {
    let rows = query_all(
        conn,
        "SELECT o.outcome_id, e.project_slug, p.name, e.slug, e.name, e.thread_slug
         FROM evidence_outcomes o JOIN evidence e ON e.project_slug = o.project_slug AND e.slug = o.evidence_slug
         JOIN projects p ON p.slug = e.project_slug ORDER BY p.name, e.name",
        [],
        |r| Ok((r.get::<_, i64>(0)?, EvidenceRef { project: r.get(1)?, project_name: r.get(2)?, slug: r.get(3)?, name: r.get(4)?, thread: r.get(5)? })),
    )?;
    let mut out: HashMap<i64, Vec<EvidenceRef>> = HashMap::new();
    for (id, item) in rows {
        out.entry(id).or_default().push(item);
    }
    Ok(out)
}

/// The name of an outcome or sub-goal, by id.
fn outcome_names(conn: &Connection) -> Result<HashMap<i64, String>> {
    let mut names: HashMap<i64, String> = query_all(conn, "SELECT id, name FROM outcomes", [], |r| Ok((r.get(0)?, r.get(1)?)))?.into_iter().collect();
    names.extend(query_all(conn, "SELECT id, name FROM sub_outcomes", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?);
    Ok(names)
}

// ---- people -------------------------------------------------------------------------------------------

/// Replaces the people with access with what Portflow has: teachers first, then by name.
pub fn store_people(conn: &Connection, shares: &[Share]) -> Result<()> {
    let mut people: BTreeMap<i64, (String, String, String, Option<String>)> = BTreeMap::new();
    for share in shares {
        let person = &share.invitee;
        if person.id == 0 || person.name.trim().is_empty() {
            continue;
        }
        let entry = people.entry(person.id).or_insert_with(|| {
            (person.name.trim().to_string(), person.current_role.clone().unwrap_or_default(), share.share_type.clone().unwrap_or_default(), share.expiration_date.clone())
        });
        // Shared more than once: the widest share and the latest end count.
        if share.share_type.as_deref() == Some("portfolio") {
            entry.2 = "portfolio".into();
        }
        if let Some(until) = &share.expiration_date
            && entry.3.as_ref().is_some_and(|current| until > current)
        {
            entry.3 = Some(until.clone());
        }
    }
    let mut ordered: Vec<(i64, (String, String, String, Option<String>))> = people.into_iter().collect();
    ordered.sort_by(|a, b| ((a.1.1 != "teacher"), a.1.0.to_lowercase()).cmp(&((b.1.1 != "teacher"), b.1.0.to_lowercase())));
    let wanted: Vec<(i64, String, String, String, Option<String>, i64)> =
        ordered.into_iter().enumerate().map(|(i, (id, (name, role, share, until)))| (id, name, role, share, until, i as i64)).collect();
    let have: Vec<(i64, String, String, String, Option<String>, i64)> =
        query_all(conn, "SELECT id, name, role, share_type, expires, position FROM portflow_people ORDER BY position", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })?;
    if have != wanted {
        conn.execute("DELETE FROM portflow_people", [])?;
        for (id, name, role, share, until, position) in wanted {
            conn.execute(
                "INSERT INTO portflow_people (id, name, role, share_type, expires, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, name, role, share, until, position],
            )?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonView {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub shared: String,
    pub until: Option<String>,
    /// Whether the share still runs: only those are offered for new feedback.
    pub active: bool,
}

pub fn people(conn: &Connection) -> Result<Vec<PersonView>> {
    let today = crate::dates::day_key(crate::dates::today());
    query_all(conn, "SELECT id, name, role, share_type, expires FROM portflow_people ORDER BY position", [], |r| {
        let until: Option<String> = r.get(4)?;
        Ok(PersonView {
            id: r.get(0)?,
            name: r.get(1)?,
            role: r.get(2)?,
            shared: r.get(3)?,
            active: until.as_deref().is_none_or(|u| u.get(..10).unwrap_or(u) >= today.as_str()),
            until,
        })
    })
}

// ---- collections --------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionLink {
    pub project: String,
    pub semester: String,
    pub collection: i64,
}

pub fn collection_links(conn: &Connection) -> Result<Vec<CollectionLink>> {
    query_all(conn, "SELECT project_slug, semester_id, collection_id FROM project_collections ORDER BY project_slug, semester_id", [], |r| {
        Ok(CollectionLink { project: r.get(0)?, semester: r.get(1)?, collection: r.get(2)? })
    })
}

/// The collection whose description a project's reading guide is: the one linked to the project for the semester that
/// covers today, or else for the latest semester that has started, or else for the first one still to come. One per
/// project; a past semester's collection keeps the description it was handed in with. Only a study project (its facts
/// say `kind: study`) has one: a hobby or company project, or one without facts, is never kept in step, even if linked.
#[derive(Debug, Clone, PartialEq)]
pub struct GuideTarget {
    pub project: String,
    /// The project's name: a guide that is still the starter one Ledger wrote counts as empty.
    pub name: String,
    pub collection: i64,
    /// The guide, as markdown (the project's README.md).
    pub readme: String,
    /// The project's evidence items, for the mentions in the guide.
    pub evidence: Vec<EvidenceMention>,
}

/// An evidence item of the project as something a reading guide can mention (`@Analysis`), with the thread it is
/// placed in, if any: a mention leads there, and a `[[link]]` to that thread becomes a mention of the item in Portflow.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceMention {
    /// Portflow's id of the item (`evidence.portflow_id`).
    pub portflow_id: i64,
    pub name: String,
    pub thread: Option<String>,
    pub thread_title: Option<String>,
    /// The deliverable the item is (its first file, the live path under deliverables/), once it is in Ledger.
    pub deliverable: Option<String>,
}

/// The project's evidence items that are in Portflow, by name.
pub fn evidence_mentions(conn: &Connection, project: &str) -> Result<Vec<EvidenceMention>> {
    query_all(
        conn,
        "SELECT e.portflow_id, e.name, t.slug, t.title, e.slug
         FROM evidence e
         LEFT JOIN threads t ON t.project_slug = e.project_slug AND t.slug = e.thread_slug
         WHERE e.project_slug = ?1 AND e.portflow_id IS NOT NULL ORDER BY e.name COLLATE NOCASE",
        [project],
        |r| Ok((EvidenceMention { portflow_id: r.get(0)?, name: r.get(1)?, thread: r.get(2)?, thread_title: r.get(3)?, deliverable: None }, r.get::<_, String>(4)?)),
    )?
    .into_iter()
    .map(|(mut mention, slug)| {
        mention.deliverable = item_deliverable(conn, project, &slug)?;
        Ok(mention)
    })
    .collect()
}

pub fn guide_targets(conn: &Connection, today: &str) -> Result<Vec<GuideTarget>> {
    type Row = (String, String, i64, String, String, String);
    let rows: Vec<Row> = query_all(
        conn,
        "SELECT l.project_slug, p.name, l.collection_id, p.readme, COALESCE(s.start_date, ''), COALESCE(s.end_date, '')
         FROM project_collections l JOIN projects p ON p.slug = l.project_slug LEFT JOIN semesters s ON s.id = l.semester_id
         WHERE COALESCE(p.kind, '') IN ('study', '')",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
    )?;
    // Lower ranks first: covering today (latest start first), started (latest first), to come (soonest first).
    let rank = |start: &str, end: &str| -> (u8, String) {
        if start <= today && today <= end {
            (0, invert(start))
        } else if start <= today {
            (1, invert(start))
        } else {
            (2, start.to_string())
        }
    };
    let mut best: BTreeMap<String, ((u8, String), GuideTarget)> = BTreeMap::new();
    for (project, name, collection, readme, start, end) in rows {
        let key = rank(&start, &end);
        if best.get(&project).is_some_and(|(k, _)| *k <= key) {
            continue;
        }
        best.insert(project.clone(), (key, GuideTarget { project, name, collection, readme, evidence: vec![] }));
    }
    let mut targets: Vec<GuideTarget> = best.into_values().map(|(_, target)| target).collect();
    for target in &mut targets {
        target.evidence = evidence_mentions(conn, &target.project)?;
    }
    Ok(targets)
}

/// A day ("2026-09-01") turned so that later days sort first.
fn invert(day: &str) -> String {
    day.chars().map(|c| c.to_digit(10).map_or(c, |d| char::from_digit(9 - d, 10).unwrap_or(c))).collect()
}

/// Links a project to a collection for one semester, or (no collection) unlinks it. One collection per project per
/// semester, and a collection belongs to one project.
pub fn link_collection(conn: &Connection, project: &str, semester: &str, collection: Option<i64>) -> Result<()> {
    projects::require(conn, project)?;
    let semester = semester.trim();
    if semester.is_empty() {
        return Err(Error::invalid("Pick the semester the collection is for"));
    }
    match collection {
        None => {
            conn.execute("DELETE FROM project_collections WHERE project_slug = ?1 AND semester_id = ?2", [project, semester])?;
            conn.execute("DELETE FROM collection_mirrors WHERE project_slug = ?1 AND semester_id = ?2", [project, semester])?;
        }
        Some(collection) => {
            let other: Option<String> = conn
                .query_row("SELECT project_slug FROM project_collections WHERE collection_id = ?1 AND project_slug <> ?2", params![collection, project], |r| r.get(0))
                .optional()?;
            if let Some(other) = other {
                return Err(Error::invalid(format!("That collection is already linked to the project \"{other}\"")));
            }
            conn.execute(
                "INSERT INTO project_collections (project_slug, semester_id, collection_id) VALUES (?1, ?2, ?3)
                 ON CONFLICT(project_slug, semester_id) DO UPDATE SET collection_id = ?3",
                params![project, semester, collection],
            )?;
            conn.execute(
                "DELETE FROM collection_mirrors WHERE project_slug = ?1 AND semester_id = ?2 AND collection_id <> ?3",
                params![project, semester, collection],
            )?;
        }
    }
    Ok(())
}

/// Stores a linked collection as Portflow shows it, with its outcomes. Not its description: that is the project's
/// reading guide, one text under two names (see `portflow_sync::sync_guide`), so it isn't kept a second time here.
pub fn store_collection(conn: &Connection, link: &CollectionLink, collection: &Collection, section: &str, goals: &[CollectionGoal]) -> Result<()> {
    let row = (
        collection.id,
        collection.name.clone(),
        collection.subtitle.clone().unwrap_or_default(),
        section.to_string(),
        collection.color.clone().unwrap_or_default(),
    );
    let current = query_opt(
        conn,
        "SELECT collection_id, name, subtitle, section, color FROM collection_mirrors WHERE project_slug = ?1 AND semester_id = ?2",
        [&link.project, &link.semester],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?)),
    )?;
    if current.as_ref() != Some(&row) {
        conn.execute(
            "INSERT OR REPLACE INTO collection_mirrors (project_slug, semester_id, collection_id, name, subtitle, section, color, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![link.project, link.semester, row.0, row.1, row.2, row.3, row.4, ids::now()],
        )?;
    }
    let mut ordered: Vec<&CollectionGoal> = goals.iter().collect();
    ordered.sort_by_key(|g| (g.position, g.goal.id));
    let wanted: Vec<i64> = ordered.iter().map(|g| g.goal.id).collect();
    let have: Vec<i64> = query_all(
        conn,
        "SELECT outcome_id FROM collection_outcomes WHERE project_slug = ?1 AND semester_id = ?2 ORDER BY position",
        [&link.project, &link.semester],
        |r| r.get(0),
    )?;
    if have != wanted {
        conn.execute("DELETE FROM collection_outcomes WHERE project_slug = ?1 AND semester_id = ?2", [&link.project, &link.semester])?;
        for (position, id) in wanted.iter().enumerate() {
            conn.execute(
                "INSERT OR IGNORE INTO collection_outcomes (project_slug, semester_id, outcome_id, position) VALUES (?1, ?2, ?3, ?4)",
                params![link.project, link.semester, id, position as i64],
            )?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionView {
    pub semester: String,
    pub semester_name: Option<String>,
    pub collection: i64,
    /// Empty until the first sync after linking.
    pub name: String,
    pub subtitle: String,
    pub section: String,
    pub color: String,
    pub outcomes: Vec<OutcomeRef>,
}

/// The project's linked collections, the current semester's first.
pub fn project_collections(conn: &Connection, project: &str) -> Result<Vec<CollectionView>> {
    let names = outcome_names(conn)?;
    let rows = query_all(
        conn,
        "SELECT l.semester_id, s.name, l.collection_id, m.name, m.subtitle, m.section, m.color, s.start_date
         FROM project_collections l
         LEFT JOIN semesters s ON s.id = l.semester_id
         LEFT JOIN collection_mirrors m ON m.project_slug = l.project_slug AND m.semester_id = l.semester_id AND m.collection_id = l.collection_id
         WHERE l.project_slug = ?1 ORDER BY s.start_date DESC, l.semester_id DESC",
        [project],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        },
    )?;
    rows.into_iter()
        .map(|(semester, semester_name, collection, name, subtitle, section, color)| {
            let outcomes = query_all(
                conn,
                "SELECT outcome_id FROM collection_outcomes WHERE project_slug = ?1 AND semester_id = ?2 ORDER BY position",
                [project, semester.as_str()],
                |r| r.get::<_, i64>(0),
            )?
            .into_iter()
            .map(|id| OutcomeRef { id, name: names.get(&id).cloned().unwrap_or_else(|| format!("#{id}")) })
            .collect();
            Ok(CollectionView {
                semester,
                semester_name,
                collection,
                name: name.unwrap_or_default(),
                subtitle: subtitle.unwrap_or_default(),
                section: section.unwrap_or_default(),
                color: color.unwrap_or_default(),
                outcomes,
            })
        })
        .collect()
}

/// What importing a collection made.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub project: String,
    pub project_name: String,
    pub evidence: usize,
    pub threads: usize,
}

/// Makes a new project from a Portflow collection: named after it (or `name`), linked to it for `semester`, with an
/// evidence item for each of the collection's evidence and, with `threads`, a thread for each of those, so their
/// feedback has a place to land. The next sync fills in the rest (outcomes, versions, feedback).
pub fn import_collection(conn: &Connection, collection: &Collection, evidence: &[Evidence], semester: &str, name: Option<&str>, threads: bool) -> Result<Imported> {
    let linked: Option<String> =
        conn.query_row("SELECT project_slug FROM project_collections WHERE collection_id = ?1", [collection.id], |r| r.get(0)).optional()?;
    if let Some(project) = linked {
        return Err(Error::invalid(format!("That collection is already in Ledger, as the project \"{project}\"")));
    }
    let name = name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(collection.name.as_str());
    let project = projects::create(conn, name)?;
    link_collection(conn, &project.slug, semester, Some(collection.id))?;
    let link = CollectionLink { project: project.slug.clone(), semester: semester.to_string(), collection: collection.id };
    let mut made_threads = 0;
    for item in evidence {
        let slug = store_evidence(conn, &link, item, &[])?;
        if threads {
            // Everything that could fail is checked above, so an import is never left half done.
            let title = if item.name.trim().is_empty() { "Evidence" } else { item.name.as_str() };
            let thread = threads::create(conn, &project.slug, title)?;
            place_evidence(conn, &project.slug, &slug, Some(&thread.slug))?;
            made_threads += 1;
        }
    }
    Ok(Imported { project: project.slug, project_name: project.name, evidence: evidence.len(), threads: made_threads })
}

// ---- evidence -----------------------------------------------------------------------------------------

/// The evidence item linked to a Portflow evidence item, by its slug in the project.
pub fn evidence_slug_for(conn: &Connection, project: &str, portflow: i64) -> Result<Option<String>> {
    query_opt(conn, "SELECT slug FROM evidence WHERE project_slug = ?1 AND portflow_id = ?2", params![project, portflow], |r| r.get(0))
}

fn free_slug(conn: &Connection, project: &str, name: &str, portflow: Option<i64>) -> Result<String> {
    let base = ids::slugify_as(name, "evidence");
    let taken = |slug: &str| -> Result<bool> {
        Ok(query_opt(conn, "SELECT 1 FROM evidence WHERE project_slug = ?1 AND slug = ?2", [project, slug], |_| Ok(()))?.is_some())
    };
    if !taken(&base)? {
        return Ok(base);
    }
    // Made the same way on every machine, so two machines syncing the same item agree on its file.
    if let Some(id) = portflow {
        let slug = format!("{base}-{id}");
        if !taken(&slug)? {
            return Ok(slug);
        }
    }
    let mut n = 2;
    loop {
        let slug = format!("{base}-{n}");
        if !taken(&slug)? {
            return Ok(slug);
        }
        n += 1;
    }
}

/// Makes or updates the evidence item for a Portflow evidence item in a linked collection. What Portflow owns (its
/// name when Ledger has none, its type, its outcomes, which collection shows it) is taken from Portflow; where it sits
/// in Ledger (the thread, the files) is the student's. Returns the slug.
pub fn store_evidence(conn: &Connection, link: &CollectionLink, evidence: &Evidence, outcomes: &[i64]) -> Result<String> {
    let kind = evidence.latest_version_type.clone().unwrap_or_else(|| "file".into());
    let slug = match evidence_slug_for(conn, &link.project, evidence.id)? {
        Some(slug) => {
            let current: (String, String) =
                conn.query_row("SELECT name, kind FROM evidence WHERE project_slug = ?1 AND slug = ?2", [&link.project, &slug], |r| Ok((r.get(0)?, r.get(1)?)))?;
            if current != (evidence.name.clone(), kind.clone()) {
                conn.execute(
                    "UPDATE evidence SET name = ?3, kind = ?4, updated_at = ?5 WHERE project_slug = ?1 AND slug = ?2",
                    params![link.project, slug, evidence.name, kind, ids::now()],
                )?;
            }
            slug
        }
        None => {
            let slug = free_slug(conn, &link.project, &evidence.name, Some(evidence.id))?;
            let created = evidence.created_at.clone().unwrap_or_else(ids::now);
            conn.execute(
                "INSERT INTO evidence (project_slug, slug, name, thread_slug, portflow_id, kind, description, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5, '', ?6, ?6)",
                params![link.project, slug, evidence.name, evidence.id, kind, created],
            )?;
            slug
        }
    };
    conn.execute(
        "INSERT INTO evidence_collections (project_slug, evidence_slug, semester_id, collection_id) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(project_slug, evidence_slug, semester_id) DO UPDATE SET collection_id = ?4",
        params![link.project, slug, link.semester, link.collection],
    )?;
    let have: BTreeSet<i64> =
        query_all(conn, "SELECT outcome_id FROM evidence_outcomes WHERE project_slug = ?1 AND evidence_slug = ?2", [&link.project, &slug], |r| r.get(0))?.into_iter().collect();
    let wanted: BTreeSet<i64> = outcomes.iter().copied().collect();
    if have != wanted {
        conn.execute("DELETE FROM evidence_outcomes WHERE project_slug = ?1 AND evidence_slug = ?2", [&link.project, &slug])?;
        for id in wanted {
            conn.execute("INSERT INTO evidence_outcomes (project_slug, evidence_slug, outcome_id) VALUES (?1, ?2, ?3)", params![link.project, slug, id])?;
        }
    }
    Ok(slug)
}

/// The version numbers of an evidence item that are already recorded, with whether their files are all there.
pub fn known_versions(conn: &Connection, project: &str, evidence: &str) -> Result<HashMap<i64, Option<i64>>> {
    Ok(query_all(
        conn,
        "SELECT number, portflow_version FROM evidence_versions WHERE project_slug = ?1 AND evidence_slug = ?2",
        [project, evidence],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?)),
    )?
    .into_iter()
    .collect())
}

/// The base path (without a version) a Portflow file goes to in the evidence item: a file of the item with the same
/// name when there is one, else a new one in a folder named after the item.
pub fn base_for_file(conn: &Connection, project: &str, evidence: &str, file_name: &str) -> Result<String> {
    let clean = crate::deliverables::file_name(file_name).trim().to_string();
    let (stem, ext) = match clean.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), Some(ext.to_ascii_lowercase())),
        _ => (clean.clone(), None),
    };
    // A name that already carries a version ("plan@v2.0.pdf") is the same file.
    let stem = crate::deliverables::split_version(&format!("{stem}.x")).0.trim_end_matches(".x").to_string();
    let stem = ids::slugify_as(&stem, "file");
    let wanted = ext.as_ref().map_or_else(|| stem.clone(), |ext| format!("{stem}.{ext}"));
    let files: Vec<String> =
        query_all(conn, "SELECT path FROM evidence_files WHERE project_slug = ?1 AND evidence_slug = ?2 ORDER BY position", [project, evidence], |r| r.get(0))?;
    let slugged = |path: &str| {
        let name = crate::deliverables::file_name(path);
        match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => format!("{}.{}", ids::slugify_as(stem, "file"), ext.to_ascii_lowercase()),
            _ => ids::slugify_as(name, "file"),
        }
    };
    if let Some(found) = files.iter().find(|p| slugged(p) == wanted) {
        return Ok(found.clone());
    }
    Ok(format!("{evidence}/{wanted}"))
}

/// A file of a Portflow version, as Ledger records it.
pub struct VersionFileRow {
    /// Portflow's name for it.
    pub name: String,
    pub portflow_file: i64,
    /// The Ledger deliverable it is, when the version came from Ledger (a published "<name>@vN.0.<ext>" of a file of
    /// the evidence item). `None`: uploaded in Portflow, and opened there.
    pub path: Option<String>,
}

/// The Ledger deliverable a Portflow file is, if Ledger has published that major of it: same name, same version.
pub fn ledger_file_for(conn: &Connection, project: &str, evidence: &str, name: &str, number: i64) -> Result<Option<String>> {
    let base = base_for_file(conn, project, evidence, name)?;
    let snapshot = crate::deliverables::snapshot_path(&base, &format!("{number}.0"));
    if crate::deliverables::exists(conn, project, &snapshot)? {
        return Ok(Some(snapshot));
    }
    // The file sits elsewhere in the deliverables folder: a published version with that exact name.
    let wanted = crate::deliverables::file_name(&snapshot).to_ascii_lowercase();
    let found: Option<String> = query_all(conn, "SELECT path FROM deliverables WHERE project_slug = ?1", [project], |r| r.get::<_, String>(0))?
        .into_iter()
        .find(|p| crate::deliverables::file_name(p).to_ascii_lowercase() == wanted);
    Ok(found)
}

/// Records Portflow version N of an evidence item. Its files are linked to Ledger's deliverables where they came from
/// Ledger; the others are only named, and stay in Portflow.
pub fn store_version(conn: &Connection, project: &str, evidence: &str, version: &Version, files: &[VersionFileRow]) -> Result<()> {
    let description = portflow_html::to_markdown(version.description.as_deref().unwrap_or_default());
    conn.execute(
        "INSERT INTO evidence_versions (project_slug, evidence_slug, number, portflow_version, kind, description, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(project_slug, evidence_slug, number) DO UPDATE SET portflow_version = ?4, kind = ?5, description = ?6",
        params![
            project,
            evidence,
            version.version_number,
            version.id,
            version.version_type.clone().unwrap_or_else(|| "file".into()),
            description,
            version.created_at.clone().unwrap_or_else(ids::now)
        ],
    )?;
    // A file already in Ledger (downloaded, or linked) keeps its place when the version is recorded again.
    let had: HashMap<i64, String> = query_all(
        conn,
        "SELECT portflow_file, path FROM evidence_version_files WHERE project_slug = ?1 AND evidence_slug = ?2 AND number = ?3 AND portflow_file IS NOT NULL AND path IS NOT NULL",
        params![project, evidence, version.version_number],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    )?
    .into_iter()
    .collect();
    conn.execute(
        "DELETE FROM evidence_version_files WHERE project_slug = ?1 AND evidence_slug = ?2 AND number = ?3",
        params![project, evidence, version.version_number],
    )?;
    let mut position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM evidence_files WHERE project_slug = ?1 AND evidence_slug = ?2",
        [project, evidence],
        |r| r.get(0),
    )?;
    for file in files {
        conn.execute(
            "INSERT OR REPLACE INTO evidence_version_files (project_slug, evidence_slug, number, name, portflow_file, path) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![project, evidence, version.version_number, file.name, file.portflow_file, file.path.as_ref().or(had.get(&file.portflow_file))],
        )?;
        // A Ledger file that went to Portflow belongs to the evidence item (unless another one has it).
        if let Some(path) = &file.path {
            let base = crate::deliverables::split_version(path).0;
            let owned: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM evidence_files WHERE project_slug = ?1 AND path = ?2)", [project, &base], |r| r.get(0))?;
            if !owned {
                conn.execute("INSERT INTO evidence_files (project_slug, evidence_slug, position, path) VALUES (?1, ?2, ?3, ?4)", params![project, evidence, position, base])?;
                position += 1;
            }
        }
    }
    Ok(())
}

// ---- evidence as deliverables ----------------------------------------------------------------------------
// In a study project linked to Portflow (or one without facts yet: being linked makes it part of a portfolio), every
// evidence item is a deliverable: its files are downloaded once, each Portflow version N becomes the published version
// N.0, and the newest is the live file. A file with a `path` in `evidence_version_files` is in Ledger; one without is
// downloaded on the next round (and on every round after, until that works).

/// The projects whose evidence comes in as deliverables: linked to a collection, and a study project or one without a
/// kind (a hobby or company project keeps its files in Portflow).
pub fn delivering_projects(conn: &Connection) -> Result<BTreeSet<String>> {
    Ok(query_all(
        conn,
        "SELECT DISTINCT l.project_slug FROM project_collections l JOIN projects p ON p.slug = l.project_slug
         WHERE COALESCE(p.kind, '') IN ('study', '')",
        [],
        |r| r.get::<_, String>(0),
    )?
    .into_iter()
    .collect())
}

/// A file of a recorded version that isn't in Ledger yet.
#[derive(Debug, Clone)]
pub struct OpenFile {
    pub number: i64,
    pub name: String,
    pub portflow_file: i64,
    /// Whether it is the file of its version that is the item (see [`crate::deliverables::main_of`]): that one is the
    /// version of the item's deliverable; the others are its attachments.
    pub first: bool,
}

/// The files of the item's file versions that aren't in Ledger yet, oldest version first.
pub fn open_files(conn: &Connection, project: &str, evidence: &str) -> Result<Vec<OpenFile>> {
    let item: String = conn.query_row("SELECT name FROM evidence WHERE project_slug = ?1 AND slug = ?2", [project, evidence], |r| r.get(0))?;
    let all: Vec<(i64, String)> = query_all(
        conn,
        "SELECT number, name FROM evidence_version_files WHERE project_slug = ?1 AND evidence_slug = ?2 ORDER BY number, rowid",
        [project, evidence],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut by_version: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for (number, name) in all {
        by_version.entry(number).or_default().push(name);
    }
    let main: HashMap<i64, String> = by_version
        .into_iter()
        .map(|(number, names)| {
            let i = crate::deliverables::main_of(&item, &names.iter().map(String::as_str).collect::<Vec<_>>());
            (number, names[i].clone())
        })
        .collect();
    let rows: Vec<(i64, String, Option<i64>)> = query_all(
        conn,
        "SELECT f.number, f.name, f.portflow_file
         FROM evidence_version_files f
         JOIN evidence_versions v ON v.project_slug = f.project_slug AND v.evidence_slug = f.evidence_slug AND v.number = f.number
         WHERE f.project_slug = ?1 AND f.evidence_slug = ?2 AND f.path IS NULL AND v.kind = 'file'
         ORDER BY f.number, f.rowid",
        [project, evidence],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(rows
        .into_iter()
        .filter_map(|(number, name, file)| {
            let first = main.get(&number) == Some(&name);
            file.map(|portflow_file| OpenFile { number, name, portflow_file, first })
        })
        .collect())
}

/// The file name an evidence item's deliverable gets: the item's name, made safe for a file, with the file's extension.
pub fn item_file_name(item: &str, file_name: &str) -> String {
    // An `@` could read as a version ("Plan@v2.0"); a dot at the end would lose the extension.
    let name = crate::uploads::sanitize(&item.replace('@', "_"));
    match crate::deliverables::file_name(file_name).rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => format!("{name}.{}", ext.to_ascii_lowercase()),
        _ => name,
    }
}

/// `path`, or the first of `path-2`, `path-3`… (before the extension) that isn't a deliverable yet.
fn free_path(conn: &Connection, project: &str, path: &str) -> Result<String> {
    let taken = |p: &str| -> Result<bool> { Ok(crate::deliverables::exists(conn, project, p)? || !crate::deliverables::versions_of(conn, project, p)?.is_empty()) };
    if !taken(path)? {
        return Ok(path.to_string());
    }
    let (stem, ext) = match path.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.contains('/') => (stem.to_string(), format!(".{ext}")),
        _ => (path.to_string(), String::new()),
    };
    let mut n = 2;
    loop {
        let candidate = format!("{stem}-{n}{ext}");
        if !taken(&candidate)? {
            return Ok(candidate);
        }
        n += 1;
    }
}

/// The item's files from Portflow, by version, in Portflow's order: (number, Portflow's name, its version in Ledger).
fn version_rows(conn: &Connection, project: &str, evidence: &str) -> Result<BTreeMap<i64, Vec<(String, String)>>> {
    let rows: Vec<(i64, String, String)> = query_all(
        conn,
        "SELECT f.number, f.name, f.path FROM evidence_version_files f
         JOIN evidence_versions v ON v.project_slug = f.project_slug AND v.evidence_slug = f.evidence_slug AND v.number = f.number
         WHERE f.project_slug = ?1 AND f.evidence_slug = ?2 AND v.kind = 'file' AND f.path IS NOT NULL
         ORDER BY f.number, f.rowid",
        [project, evidence],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut out: BTreeMap<i64, Vec<(String, String)>> = BTreeMap::new();
    for (number, name, path) in rows {
        out.entry(number).or_default().push((name, path));
    }
    Ok(out)
}

/// Of a version's files, the one that is the item, by Portflow's names (which a rename in Ledger doesn't change).
fn main_row<'a>(item: &str, files: &'a [(String, String)]) -> &'a (String, String) {
    &files[crate::deliverables::main_of(item, &files.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>())]
}

/// The item's deliverable: the file its newest version in Ledger is a version of (that version's file that is the item,
/// by Portflow's names); with no version in Ledger, the item's file named most like it. `None` while it has none.
pub fn item_deliverable(conn: &Connection, project: &str, evidence: &str) -> Result<Option<String>> {
    let item: String = conn.query_row("SELECT name FROM evidence WHERE project_slug = ?1 AND slug = ?2", [project, evidence], |r| r.get(0))?;
    let paths: Vec<String> =
        query_all(conn, "SELECT path FROM evidence_files WHERE project_slug = ?1 AND evidence_slug = ?2 ORDER BY position", [project, evidence], |r| r.get(0))?;
    if let Some((_, files)) = version_rows(conn, project, evidence)?.into_iter().next_back() {
        let base = crate::deliverables::split_version(&main_row(&item, &files).1).0;
        if paths.contains(&base) {
            return Ok(Some(base));
        }
    }
    if paths.is_empty() {
        return Ok(None);
    }
    Ok(Some(paths[crate::deliverables::main_of(&item, &paths.iter().map(String::as_str).collect::<Vec<_>>())].clone()))
}

/// The extension of a path, lowercased (`""` for none).
fn ext_of(path: &str) -> String {
    crate::deliverables::file_name(path).rsplit_once('.').filter(|(stem, _)| !stem.is_empty()).map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default()
}

/// Where a downloaded file lives (its base path, without a version). An evidence item is one deliverable, named after
/// the item: `deliverables/<item>.<ext>`. The first file of each Portflow version is that version of it, whatever the
/// file was called. A version's other files are its attachments, beside it in `deliverables/<item>/<file>`, and so is a
/// first file of another kind (a .pptx where the deliverable is a .pdf). A name another deliverable has gets a number.
pub fn base_for_download(conn: &Connection, project: &str, evidence: &str, file: &OpenFile) -> Result<String> {
    let main = item_deliverable(conn, project, evidence)?;
    if let Some(main) = main.as_ref().filter(|m| file.first && ext_of(m) == ext_of(&file.name)) {
        let version = format!("{}.0", file.number);
        if !crate::deliverables::versions_of(conn, project, main)?.contains(&version) {
            return Ok(main.clone());
        }
    }
    // Came in before, under the same name.
    let before: Option<String> = query_opt(
        conn,
        "SELECT path FROM evidence_version_files WHERE project_slug = ?1 AND evidence_slug = ?2 AND name = ?3 AND path IS NOT NULL ORDER BY number DESC LIMIT 1",
        params![project, evidence, file.name],
        |r| r.get(0),
    )?;
    if let Some(path) = before {
        return Ok(crate::deliverables::split_version(&path).0);
    }
    // A file of the item already (Ledger handed it in under that name).
    let found = base_for_file(conn, project, evidence, &file.name)?;
    let owned: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM evidence_files WHERE project_slug = ?1 AND evidence_slug = ?2 AND path = ?3)", params![project, evidence, found], |r| r.get(0))?;
    if owned {
        return Ok(found);
    }
    let item: String = conn.query_row("SELECT name FROM evidence WHERE project_slug = ?1 AND slug = ?2", [project, evidence], |r| r.get(0))?;
    match main {
        None if file.first => free_path(conn, project, &item_file_name(&item, &file.name)),
        main => {
            let dir = main.map_or_else(|| crate::uploads::sanitize(&item.replace('@', "_")), |m| crate::deliverables::attachment_dir(&m));
            let name = crate::uploads::sanitize(&crate::deliverables::file_name(&file.name).replace('@', "_"));
            free_path(conn, project, &format!("{dir}/{name}"))
        }
    }
}

/// Whether a deliverable came from Portflow (Ledger didn't make it): a version of it was recorded as downloaded.
fn came_from_portflow(conn: &Connection, project: &str, base: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM version_records WHERE project_slug = ?1 AND deliverable_path = ?2
                       AND (note LIKE 'From Portflow, version %' OR note LIKE 'Version % in Portflow'))",
        [project, base],
        |r| r.get(0),
    )?)
}

/// The renames that make the item's files look like one deliverable named after it, in the order to make them: each
/// attachment from Portflow into `<item>/` under Portflow's name for it, then the deliverable to `<item>.<ext>` (both
/// only for files that came from Portflow; Ledger's own keep their names, and are still shown by the item's).
/// Each is `(from, to, whether it is the deliverable itself)`.
fn renames_to_settle(conn: &Connection, project: &str, evidence: &str) -> Result<Vec<(String, String, bool)>> {
    let Some(main) = item_deliverable(conn, project, evidence)? else { return Ok(vec![]) };
    let item: String = conn.query_row("SELECT name FROM evidence WHERE project_slug = ?1 AND slug = ?2", [project, evidence], |r| r.get(0))?;
    let main_from_portflow = came_from_portflow(conn, project, &main)?;
    let wanted = if main_from_portflow { item_file_name(&item, &main) } else { main.clone() };
    let dir = crate::deliverables::attachment_dir(&wanted);
    // Portflow's name for each of the item's files: the newest version it is in.
    let mut names: HashMap<String, String> = HashMap::new();
    for (_, files) in version_rows(conn, project, evidence)? {
        for (name, path) in files {
            names.insert(crate::deliverables::split_version(&path).0, name);
        }
    }
    let mut out = vec![];
    let others: Vec<String> =
        query_all(conn, "SELECT path FROM evidence_files WHERE project_slug = ?1 AND evidence_slug = ?2 ORDER BY position", [project, evidence], |r| r.get(0))?;
    for path in others.into_iter().filter(|p| *p != main) {
        if path.starts_with(&format!("{dir}/")) || !came_from_portflow(conn, project, &path)? {
            continue;
        }
        let name = names.get(&path).map_or_else(|| crate::deliverables::file_name(&path).to_string(), Clone::clone);
        out.push((path, format!("{dir}/{}", crate::uploads::sanitize(&crate::deliverables::file_name(&name).replace('@', "_"))), false));
    }
    if main != wanted {
        out.push((main, wanted, true));
    }
    Ok(out)
}

/// Whether [`settle_name`] has anything to do for the item.
pub fn needs_settling(conn: &Connection, project: &str, evidence: &str) -> Result<bool> {
    Ok(!renames_to_settle(conn, project, evidence)?.is_empty())
}

/// Keeps an item one deliverable, named after the item: its attachments from Portflow go into `<item>/`; and a
/// deliverable that came from Portflow is renamed — with its versions — to `<item>.<ext>` when the item was renamed in
/// Portflow or came in under another name. A name that is taken is left as it is. Returns the renames made.
pub fn settle_name(conn: &Connection, root: &std::path::Path, project: &str, evidence: &str) -> Result<Vec<(String, String)>> {
    let mut done = vec![];
    for (from, to, is_main) in renames_to_settle(conn, project, evidence)? {
        let taken = |p: &str| -> Result<bool> { Ok(crate::deliverables::exists(conn, project, p)? || !crate::deliverables::versions_of(conn, project, p)?.is_empty()) };
        // An attachment gets a number when its name is taken; the deliverable waits for its name.
        let to = if taken(&to)? {
            if is_main {
                continue;
            }
            free_path(conn, project, &to)?
        } else {
            to
        };
        crate::deliverables::rename_base(conn, root, project, &from, &to)?;
        done.push((from, to));
    }
    Ok(done)
}

/// What became of the live file when a downloaded version was stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Live {
    /// Not the newest version: the live file isn't its business.
    Untouched,
    /// The live file is now this version.
    Written,
    /// The live file was changed in Ledger since it last matched a version: it stays, and the version is beside it.
    Kept,
}

/// Stores a downloaded file as published version `N.0` of `base` (with the record of where it came from), links the
/// Portflow file to it, makes `base` one of the item's files, and, for the newest version, makes it the live file too —
/// unless the live file was changed in Ledger since it last matched a published version.
#[allow(clippy::too_many_arguments, reason = "what a version is made from and where it is written: a struct for one call would only name them again")]
pub fn store_download(
    conn: &Connection,
    root: &std::path::Path,
    project: &str,
    evidence: &str,
    file: &OpenFile,
    base: &str,
    bytes: &[u8],
    newest: bool,
) -> Result<Live> {
    use crate::deliverables::{file_of, is_markdown, snapshot_path, upsert, versions_of};
    let version = format!("{}.0", file.number);
    let snapshot = snapshot_path(base, &version);
    let at = ids::now();
    let markdown = is_markdown(base);
    let text = markdown.then(|| String::from_utf8_lossy(bytes).into_owned());
    let sha256 = crate::provenance::sha256_hex(text.as_deref().map_or(bytes, str::as_bytes));
    // What the live file was last published as, before this version: to tell whether it was changed in Ledger since.
    let previous = versions_of(conn, project, base)?.into_iter().last();
    let previous_sha: Option<String> = match &previous {
        Some(v) => query_opt(
            conn,
            "SELECT sha256 FROM version_records WHERE project_slug = ?1 AND deliverable_path = ?2 AND version = ?3",
            params![project, base, v],
            |r| r.get::<_, Option<String>>(0),
        )?
        .flatten(),
        None => None,
    };

    let write = |path: &str| -> Result<()> {
        match &text {
            Some(text) => upsert(conn, project, path, text.len() as i64, &at, Some(text)),
            None => {
                let to = crate::deliverables::rel_of(project, path);
                crate::store::at(root).write(&to, bytes).map_err(|e| Error::io(&file_of(root, project, path), e))?;
                upsert(conn, project, path, bytes.len() as i64, &at, None)
            }
        }
    };
    write(&snapshot)?;
    let thread: Option<String> =
        query_opt(conn, "SELECT thread_slug FROM evidence WHERE project_slug = ?1 AND slug = ?2", [project, evidence], |r| r.get::<_, Option<String>>(0))?.flatten();
    let origin = crate::provenance::Origin {
        cause: crate::provenance::Cause::Manual,
        thread,
        ai: false,
        prompt: None,
        run: None,
        mmm: None,
        note: format!("From Portflow, version {}", file.number),
    };
    crate::provenance::insert_record(conn, project, base, &version, &origin, &sha256, &at)?;
    conn.execute(
        "UPDATE evidence_version_files SET path = ?5 WHERE project_slug = ?1 AND evidence_slug = ?2 AND number = ?3 AND name = ?4",
        params![project, evidence, file.number, file.name, snapshot],
    )?;
    let owned: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM evidence_files WHERE project_slug = ?1 AND path = ?2)", [project, base], |r| r.get(0))?;
    if !owned {
        let position: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM evidence_files WHERE project_slug = ?1 AND evidence_slug = ?2",
            [project, evidence],
            |r| r.get(0),
        )?;
        conn.execute("INSERT INTO evidence_files (project_slug, evidence_slug, position, path) VALUES (?1, ?2, ?3, ?4)", params![project, evidence, position, base])?;
    }

    if !newest {
        return Ok(Live::Untouched);
    }
    // The live file as it is now: missing, the same as the last published version, or changed in Ledger since.
    let live_sha: Option<String> = if crate::deliverables::exists(conn, project, base)? {
        let body: Option<String> = query_opt(conn, "SELECT body FROM deliverables WHERE project_slug = ?1 AND path = ?2", [project, base], |r| r.get(0))?.flatten();
        Some(match body {
            Some(body) => crate::provenance::sha256_hex(body.as_bytes()),
            None => {
                let from = crate::deliverables::rel_of(project, base);
                crate::provenance::sha256_hex(&crate::store::at(root).read(&from).map_err(|e| Error::io(&file_of(root, project, base), e))?)
            }
        })
    } else {
        None
    };
    match live_sha {
        Some(current) if current == sha256 => Ok(Live::Written),
        Some(current) if previous_sha.as_deref() != Some(current.as_str()) => Ok(Live::Kept),
        _ => {
            write(base)?;
            Ok(Live::Written)
        }
    }
}

/// Links files of recorded versions that Ledger has published since (they were only in Portflow before).
pub fn relink_versions(conn: &Connection, project: &str, evidence: &str) -> Result<()> {
    let open: Vec<(i64, String)> = query_all(
        conn,
        "SELECT number, name FROM evidence_version_files WHERE project_slug = ?1 AND evidence_slug = ?2 AND path IS NULL",
        [project, evidence],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    for (number, name) in open {
        if let Some(path) = ledger_file_for(conn, project, evidence, &name, number)? {
            conn.execute(
                "UPDATE evidence_version_files SET path = ?5 WHERE project_slug = ?1 AND evidence_slug = ?2 AND number = ?3 AND name = ?4",
                params![project, evidence, number, name, path],
            )?;
        }
    }
    Ok(())
}

/// Puts an evidence item in a thread (or takes it out of one). Its feedback lands there from the next sync on.
pub fn place_evidence(conn: &Connection, project: &str, evidence: &str, thread: Option<&str>) -> Result<()> {
    if let Some(thread) = thread {
        threads::require(conn, project, thread)?;
    }
    let changed = conn.execute(
        "UPDATE evidence SET thread_slug = ?3, updated_at = ?4 WHERE project_slug = ?1 AND slug = ?2",
        params![project, evidence, thread, ids::now()],
    )?;
    if changed == 0 {
        return Err(Error::not_found("Evidence item not found"));
    }
    // Feedback already pulled in moves with it.
    if let Some(thread) = thread {
        conn.execute("UPDATE feedback SET thread_slug = ?3 WHERE project_slug = ?1 AND evidence = ?2 AND portflow_comment IS NOT NULL", params![project, evidence, thread])?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceFileView {
    pub path: String,
    pub name: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionFileView {
    pub name: String,
    /// The Ledger deliverable, when it came from Ledger; otherwise it is opened in Portflow.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceVersionView {
    pub number: i64,
    pub portflow_version: Option<i64>,
    pub kind: String,
    pub created_at: String,
    pub description_html: String,
    pub files: Vec<VersionFileView>,
    /// Every file is in Ledger.
    pub in_ledger: bool,
    /// This version's page in Portflow.
    pub portflow_url: Option<String>,
}

/// The page of an evidence item (at a version, or its latest) in Portflow's own web app.
pub fn portflow_url(collection: Option<i64>, evidence: i64, version: Option<i64>) -> String {
    let version = version.map_or_else(|| "latest".to_string(), |v| v.to_string());
    match collection {
        Some(c) => format!("{}/collections/{c}/evidence/{evidence}/versions/{version}", crate::portflow_api::WEB_APP),
        None => format!("{}/evidence/{evidence}/versions/{version}", crate::portflow_api::WEB_APP),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceView {
    pub project: String,
    pub slug: String,
    pub name: String,
    pub thread: Option<String>,
    pub thread_title: Option<String>,
    pub portflow_id: Option<i64>,
    pub kind: String,
    /// The description as Ledger keeps it (markdown), and its What? / Goal? / Why it contributes? when it has that shape.
    pub description: String,
    pub description_parts: Option<crate::portflow_evidence_text::Parts>,
    pub description_html: String,
    pub files: Vec<EvidenceFileView>,
    pub outcomes: Vec<OutcomeRef>,
    pub versions: Vec<EvidenceVersionView>,
    pub feedback_count: i64,
    pub open_feedback: i64,
    /// The item's page in Portflow (its latest version).
    pub portflow_url: Option<String>,
}

fn evidence_views(conn: &Connection, project: &str, filter: &str, arg: Option<&str>, me: i64) -> Result<Vec<EvidenceView>> {
    let names = outcome_names(conn)?;
    let sql = format!(
        "SELECT e.slug, e.name, e.thread_slug, t.title, e.portflow_id, e.kind, e.description FROM evidence e
         LEFT JOIN threads t ON t.project_slug = e.project_slug AND t.slug = e.thread_slug
         WHERE e.project_slug = ?1 {filter} ORDER BY e.created_at, e.slug"
    );
    type Row = (String, String, Option<String>, Option<String>, Option<i64>, String, String);
    let rows: Vec<Row> = match arg {
        Some(arg) => query_all(conn, &sql, [project, arg], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?,
        None => query_all(conn, &sql, [project], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?,
    };
    rows.into_iter()
        .map(|(slug, name, thread, thread_title, portflow_id, kind, description)| {
            let collection: Option<i64> = conn
                .query_row(
                    "SELECT collection_id FROM evidence_collections WHERE project_slug = ?1 AND evidence_slug = ?2 ORDER BY semester_id DESC LIMIT 1",
                    [project, slug.as_str()],
                    |r| r.get(0),
                )
                .optional()?;
            let files = query_all(
                conn,
                "SELECT f.path, EXISTS(SELECT 1 FROM deliverables d WHERE d.project_slug = f.project_slug AND d.path = f.path)
                 FROM evidence_files f WHERE f.project_slug = ?1 AND f.evidence_slug = ?2 ORDER BY f.position",
                [project, slug.as_str()],
                |r| {
                    let path: String = r.get(0)?;
                    Ok(EvidenceFileView { name: crate::deliverables::file_name(&path).to_string(), exists: r.get(1)?, path })
                },
            )?;
            let outcomes = query_all(conn, "SELECT outcome_id FROM evidence_outcomes WHERE project_slug = ?1 AND evidence_slug = ?2", [project, slug.as_str()], |r| r.get::<_, i64>(0))?
                .into_iter()
                .map(|id| OutcomeRef { id, name: names.get(&id).cloned().unwrap_or_else(|| format!("#{id}")) })
                .collect();
            let versions = query_all(
                conn,
                "SELECT number, portflow_version, kind, created_at, description FROM evidence_versions WHERE project_slug = ?1 AND evidence_slug = ?2 ORDER BY number DESC",
                [project, slug.as_str()],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?)),
            )?
            .into_iter()
            .map(|(number, portflow_version, kind, created_at, description)| {
                let files: Vec<VersionFileView> = query_all(
                    conn,
                    "SELECT name, path FROM evidence_version_files WHERE project_slug = ?1 AND evidence_slug = ?2 AND number = ?3 ORDER BY name",
                    params![project, slug, number],
                    |r| Ok(VersionFileView { name: r.get(0)?, path: r.get(1)? }),
                )?;
                let in_ledger = !files.is_empty() && files.iter().all(|f| f.path.is_some());
                let url = portflow_id.map(|e| portflow_url(collection, e, portflow_version));
                Ok(EvidenceVersionView { number, portflow_version, kind, created_at, description_html: markdown::render(&description), files, in_ledger, portflow_url: url })
            })
            .collect::<Result<Vec<_>>>()?;
            let (feedback_count, open_feedback): (i64, i64) = conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(status = 'open'), 0) FROM feedback WHERE project_slug = ?1 AND evidence = ?2 AND COALESCE(from_user, -1) <> ?3",
                params![project, slug, me],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok(EvidenceView {
                project: project.to_string(),
                slug,
                name,
                thread,
                thread_title,
                portflow_id,
                kind,
                description_html: markdown::render(&description),
                description_parts: crate::portflow_evidence_text::parse(&description),
                description,
                files,
                outcomes,
                versions,
                feedback_count,
                open_feedback,
                portflow_url: portflow_id.map(|e| portflow_url(collection, e, None)),
            })
        })
        .collect()
}

/// `me`: the student's Portflow id; their own comments aren't counted as feedback.
pub fn evidence_in_project(conn: &Connection, project: &str, me: i64) -> Result<Vec<EvidenceView>> {
    evidence_views(conn, project, "", None, me)
}

pub fn evidence_in_thread(conn: &Connection, project: &str, thread: &str, me: i64) -> Result<Vec<EvidenceView>> {
    evidence_views(conn, project, "AND e.thread_slug = ?2", Some(thread), me)
}

// ---- comments → feedback ------------------------------------------------------------------------------

/// The feedback id a Portflow comment gets: the same on every machine, so two machines syncing it agree.
pub fn feedback_id(comment: i64) -> String {
    format!("portflow-{comment}")
}

/// Stores a comment on an evidence item as feedback in the item's thread, open: only the student marks feedback resolved.
/// The student's own comments (their feedback requests and replies) are kept too. Returns whether it was new feedback
/// from someone else.
pub fn store_comment(conn: &Connection, project: &str, evidence: &str, thread: &str, comment: &Comment, me: i64) -> Result<bool> {
    let body = portflow_html::to_markdown(comment.text.as_deref().unwrap_or_default());
    let body = match (body.trim().is_empty(), &comment.feedback_request) {
        (false, _) => body,
        (true, Some(request)) if !request.invitees.is_empty() => {
            format!("Asked for feedback from {}.", request.invitees.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "))
        }
        (true, _) => return Ok(false),
    };
    if comment.hidden {
        return Ok(false);
    }
    let id = feedback_id(comment.id);
    let version = comment.evidence_version.as_ref().map(|v| v.version_number);
    // The deliverable it is on: the first file of that version, when Ledger has it.
    let deliverable: String = match version {
        Some(n) => conn
            .query_row(
                "SELECT path FROM evidence_version_files WHERE project_slug = ?1 AND evidence_slug = ?2 AND number = ?3 AND path IS NOT NULL ORDER BY name LIMIT 1",
                params![project, evidence, n],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default(),
        None => String::new(),
    };
    let deliverable_version = crate::deliverables::split_version(&deliverable).1;
    let created = comment.created_at.clone().unwrap_or_else(ids::now);
    let updated = comment.updated_at.clone().unwrap_or_else(|| created.clone());
    let author = &comment.created_by;
    let role = comment.created_by_role.clone().or_else(|| author.current_role.clone());
    let reply_to = comment.parent_id.map(feedback_id);
    let mine = author.id == me && me != 0;

    let current = query_opt(
        conn,
        "SELECT body, deliverable_path, from_name, from_role, evidence_version, comment_type, reply_to FROM feedback WHERE id = ?1",
        [&id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        },
    )?;
    let wanted = (body.clone(), deliverable.clone(), Some(author.name.clone()), role.clone(), version, comment.comment_type.clone(), reply_to.clone());
    match current {
        Some(current) if current == wanted => Ok(false),
        Some(_) => {
            conn.execute(
                "UPDATE feedback SET body = ?2, deliverable_path = ?3, deliverable_version = ?4, from_name = ?5, from_user = ?6, from_role = ?7,
                     evidence_version = ?8, comment_type = ?9, reply_to = ?10, updated_at = ?11 WHERE id = ?1",
                params![id, body, deliverable, deliverable_version, author.name, author.id, role, version, comment.comment_type, reply_to, updated],
            )?;
            Ok(false)
        }
        None => {
            conn.execute(
                "INSERT INTO feedback (id, project_slug, thread_slug, deliverable_path, deliverable_version, status, created_at, updated_at, body,
                     from_name, from_user, from_role, given, portflow_comment, reply_to, comment_type, evidence, evidence_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'portflow', ?13, ?14, ?15, ?16, ?17)",
                params![
                    id,
                    project,
                    thread,
                    deliverable,
                    deliverable_version,
                    "open",
                    created,
                    updated,
                    body,
                    author.name,
                    author.id,
                    role,
                    comment.id,
                    reply_to,
                    comment.comment_type,
                    evidence,
                    version
                ],
            )?;
            Ok(!mine)
        }
    }
}

/// New feedback from others since a moment, newest first: for the dashboard.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFeedback {
    pub project: String,
    pub project_name: String,
    pub thread: String,
    pub id: String,
    pub from: Option<String>,
    pub from_role: Option<String>,
    pub evidence: Option<String>,
    pub evidence_name: Option<String>,
    pub created_at: String,
    pub excerpt: String,
}

pub fn open_portflow_feedback(conn: &Connection, limit: i64, me: i64) -> Result<Vec<RecentFeedback>> {
    query_all(
        conn,
        "SELECT f.project_slug, p.name, f.thread_slug, f.id, f.from_name, f.from_role, f.evidence, e.name, f.created_at, f.body
         FROM feedback f JOIN projects p ON p.slug = f.project_slug
         LEFT JOIN evidence e ON e.project_slug = f.project_slug AND e.slug = f.evidence
         WHERE f.portflow_comment IS NOT NULL AND f.status = 'open' AND COALESCE(f.from_user, -1) <> ?2 ORDER BY f.created_at DESC LIMIT ?1",
        [limit, me],
        |r| {
            let body: String = r.get(9)?;
            let excerpt: String = body.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(160).collect();
            Ok(RecentFeedback {
                project: r.get(0)?,
                project_name: r.get(1)?,
                thread: r.get(2)?,
                id: r.get(3)?,
                from: r.get(4)?,
                from_role: r.get(5)?,
                evidence: r.get(6)?,
                evidence_name: r.get(7)?,
                created_at: r.get(8)?,
                excerpt,
            })
        },
    )
}

/// Sets an evidence item's description, as Ledger keeps it (`portflow_evidence_text`): the body of its file.
pub fn set_evidence_description(conn: &Connection, project: &str, evidence: &str, text: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE evidence SET description = ?3, updated_at = ?4 WHERE project_slug = ?1 AND slug = ?2 AND description != ?3",
        params![project, evidence, text.trim(), ids::now()],
    )?;
    if changed == 0 && query_opt(conn, "SELECT 1 FROM evidence WHERE project_slug = ?1 AND slug = ?2", [project, evidence], |r| r.get::<_, i64>(0))?.is_none() {
        return Err(Error::not_found("Evidence item not found"));
    }
    Ok(())
}
