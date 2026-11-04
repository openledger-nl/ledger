//! Semesters and Portflow's sections, and Gamma: the competence matrix of a semester.
//!
//! **Sections are semesters.** Every semester the portfolio gets a new section ("Open Learning FT Semester 3 – Fall 26")
//! with new collections and a new goal set, all from that semester's template, in one import. The section and the goals
//! carry the same import id, which is how a semester's learning outcomes are told apart from another semester's. A Ledger
//! semester is linked to one section (`portflowSection` in `semesters/<id>.md`); a section that is new to Ledger is linked
//! to the semester its import falls in, once, after which the link is the student's to change.
//!
//! **Gamma** is the HBO-i competence matrix, built from Portflow, where the assessment of this semester lives: the
//! activities (Analysis, Advise, Design, Realise, Manage & Control) across the architecture layers (User Interaction,
//! Software, Organisational Processes, Hardware Interfacing, Infrastructure). A
//! sub-goal named "Analysis-U2" is that goal's activity in the User Interaction layer at level 2. Per cell: how many
//! evidence items count for it, and the highest level that has evidence. Goals whose sub-goals have a level but no layer
//! (Professional standard "PS-1", Personal leadership "PL-1") are shown as levels on their own.
//!
//! **Goals.** The student says what they aim for this semester (in Gamma, "Set goals"): a level per layer, which is
//! a whole row ("Infrastructure, level 2"), and a level per level-only goal (PS, PL). The grid stays whole; the rows
//! aimed for are marked, each cell says whether it got there, and a cell's detail shows that level's description only.
//! With the goals comes the semester's overview: its learning plan (an evidence item, delivered in Portflow) and its
//! study guide (a link). All of it is the student's own, kept in `semesters/<id>.md` under `gamma:`.
//!
//! Everything is read from the database (the vault's mirror of Portflow), so opening the screen sends nothing anywhere
//! and takes a few queries: the sync keeps the mirror fresh in the background.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::frontmatter::{self, MetaExt};
use crate::markdown;
use crate::portflow_api::Section;
use crate::portflow_store::EvidenceRef;

// ---- sections -----------------------------------------------------------------------------------------

/// Replaces the sections mirror with what Portflow has. Sections Ledger hadn't seen before are linked to the semester
/// their import falls in (when that semester has no section yet); others keep the link they have, or don't have.
pub fn store_sections(conn: &Connection, sections: &[Section]) -> Result<()> {
    let known: HashSet<i64> = query_all(conn, "SELECT id FROM portflow_sections", [], |r| r.get(0))?.into_iter().collect();
    type Row = (String, i64, Option<i64>, String, Option<String>);
    for section in sections {
        let import = section.last_import.as_ref();
        let row: Row = (
            section.name.clone(),
            section.position,
            import.and_then(|i| i.id),
            import.and_then(|i| i.template_name.clone()).unwrap_or_default(),
            import.and_then(|i| i.imported_at.clone()),
        );
        let have = query_opt(conn, "SELECT name, position, import_id, template, imported_at FROM portflow_sections WHERE id = ?1", [section.id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?;
        if have.as_ref() != Some(&row) {
            conn.execute(
                "INSERT INTO portflow_sections (id, name, position, import_id, template, imported_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET name = ?2, position = ?3, import_id = ?4, template = ?5, imported_at = ?6",
                params![section.id, row.0, row.1, row.2, row.3, row.4],
            )?;
        }
    }
    let wanted: HashSet<i64> = sections.iter().map(|s| s.id).collect();
    for gone in known.iter().filter(|id| !wanted.contains(id)) {
        conn.execute("DELETE FROM portflow_sections WHERE id = ?1", [gone])?;
    }
    for section in sections.iter().filter(|s| !known.contains(&s.id)) {
        let linked = query_opt(conn, "SELECT 1 FROM semester_sections WHERE section_id = ?1", [section.id], |r| r.get::<_, i64>(0))?.is_some();
        let Some(day) = section.last_import.as_ref().and_then(|i| i.imported_at.as_deref()).and_then(|d| d.get(..10)) else { continue };
        if linked {
            continue;
        }
        if let Some(semester) = semester_for_day(conn, day)? {
            conn.execute("INSERT INTO semester_sections (semester_id, section_id) VALUES (?1, ?2)", params![semester, section.id])?;
        }
    }
    Ok(())
}

/// The semester without a section that a section imported on `day` belongs to: the one the day falls in, or else the
/// first one starting within a month after it (templates are often imported before the semester starts).
fn semester_for_day(conn: &Connection, day: &str) -> Result<Option<String>> {
    let free = "s.id NOT IN (SELECT semester_id FROM semester_sections)";
    if let Some(id) = query_opt(conn, &format!("SELECT s.id FROM semesters s WHERE {free} AND s.start_date <= ?1 AND s.end_date >= ?1"), [day], |r| r.get(0))? {
        return Ok(Some(id));
    }
    query_opt(
        conn,
        &format!("SELECT s.id FROM semesters s WHERE {free} AND s.start_date > ?1 AND s.start_date <= date(?1, '+31 days') ORDER BY s.start_date LIMIT 1"),
        [day],
        |r| r.get(0),
    )
}

/// Links a semester to a section, or unlinks it (`None`). A section is one semester's at most, so it moves.
pub fn link_section(conn: &Connection, semester: &str, section: Option<i64>) -> Result<()> {
    crate::semesters::get(conn, semester)?;
    conn.execute("DELETE FROM semester_sections WHERE semester_id = ?1", [semester])?;
    if let Some(section) = section {
        conn.execute("DELETE FROM semester_sections WHERE section_id = ?1", [section])?;
        conn.execute("INSERT INTO semester_sections (semester_id, section_id) VALUES (?1, ?2)", params![semester, section])?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionView {
    pub id: i64,
    pub name: String,
    pub template: String,
    pub imported_at: Option<String>,
    /// The semester it is linked to.
    pub semester: Option<String>,
    pub semester_name: Option<String>,
}

/// The portfolio's sections, as last synced, with the semester each is.
pub fn sections(conn: &Connection) -> Result<Vec<SectionView>> {
    query_all(
        conn,
        "SELECT p.id, p.name, p.template, p.imported_at, l.semester_id, s.name FROM portflow_sections p
         LEFT JOIN semester_sections l ON l.section_id = p.id LEFT JOIN semesters s ON s.id = l.semester_id
         ORDER BY p.position, p.id",
        [],
        |r| Ok(SectionView { id: r.get(0)?, name: r.get(1)?, template: r.get(2)?, imported_at: r.get(3)?, semester: r.get(4)?, semester_name: r.get(5)? }),
    )
}

// ---- the semester's goals -----------------------------------------------------------------------------

/// What a semester aims for, and its overview. Keys: a layer's letter ("I"); a level-only goal's nickname or name ("PS").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GammaGoals {
    pub layers: BTreeMap<String, i64>,
    pub goals: BTreeMap<String, i64>,
    /// The learning plan: an evidence item, as "<project>/<evidence slug>".
    pub learning_plan: Option<String>,
    pub study_guide: Option<String>,
}

const MAX_LEVEL: i64 = 9;

pub fn goals_of(conn: &Connection, semester: &str) -> Result<GammaGoals> {
    let mut goals = GammaGoals::default();
    for (kind, key, level) in query_all(conn, "SELECT kind, key, level FROM semester_targets WHERE semester_id = ?1 ORDER BY kind, key", [semester], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
    })? {
        match kind.as_str() {
            "layer" => goals.layers.insert(key, level),
            _ => goals.goals.insert(key, level),
        };
    }
    if let Some((project, evidence, guide)) = query_opt(conn, "SELECT plan_project, plan_evidence, study_guide FROM semester_plans WHERE semester_id = ?1", [semester], |r| {
        Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?))
    })? {
        goals.learning_plan = project.zip(evidence).map(|(p, e)| format!("{p}/{e}"));
        goals.study_guide = guide;
    }
    Ok(goals)
}

/// Checks goals and keeps what they say, leaving out what says nothing (a level of 0, an empty link).
fn checked(goals: &GammaGoals) -> Result<GammaGoals> {
    let level = |what: &str, l: i64| {
        if (1..=MAX_LEVEL).contains(&l) { Ok(l) } else { Err(Error::invalid(format!("The level for {what} must be 1 to {MAX_LEVEL}"))) }
    };
    let mut out = GammaGoals::default();
    for (key, l) in &goals.layers {
        let key = key.trim().to_uppercase();
        if !LAYERS.iter().any(|(k, _)| *k == key) {
            return Err(Error::invalid(format!("There is no layer “{key}”")));
        }
        out.layers.insert(key.clone(), level(&key, *l)?);
    }
    for (key, l) in &goals.goals {
        let key = key.trim();
        if !key.is_empty() {
            out.goals.insert(key.to_string(), level(key, *l)?);
        }
    }
    out.learning_plan = match goals.learning_plan.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        None => None,
        Some(plan) => match plan.split_once('/') {
            Some((p, e)) if !p.is_empty() && !e.is_empty() && !e.contains('/') => Some(plan.to_string()),
            _ => return Err(Error::invalid("A learning plan is an evidence item: <project>/<evidence>")),
        },
    };
    out.study_guide = match goals.study_guide.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
        None => None,
        Some(url) if url.starts_with("https://") || url.starts_with("http://") => Some(url.to_string()),
        Some(_) => return Err(Error::invalid("The study guide is a web address (https://…)")),
    };
    Ok(out)
}

fn store_goals(conn: &Connection, semester: &str, goals: &GammaGoals) -> Result<()> {
    conn.execute("DELETE FROM semester_targets WHERE semester_id = ?1", [semester])?;
    conn.execute("DELETE FROM semester_plans WHERE semester_id = ?1", [semester])?;
    for (kind, map) in [("layer", &goals.layers), ("goal", &goals.goals)] {
        for (key, level) in map {
            conn.execute("INSERT INTO semester_targets (semester_id, kind, key, level) VALUES (?1, ?2, ?3, ?4)", params![semester, kind, key, level])?;
        }
    }
    let (project, evidence) = goals.learning_plan.as_deref().and_then(|p| p.split_once('/')).unzip();
    if project.is_some() || goals.study_guide.is_some() {
        conn.execute(
            "INSERT INTO semester_plans (semester_id, plan_project, plan_evidence, study_guide) VALUES (?1, ?2, ?3, ?4)",
            params![semester, project, evidence, goals.study_guide],
        )?;
    }
    Ok(())
}

/// Replaces a semester's goals.
pub fn set_goals(conn: &Connection, semester: &str, goals: &GammaGoals) -> Result<()> {
    crate::semesters::get(conn, semester)?;
    store_goals(conn, semester, &checked(goals)?)
}

/// The `gamma:` block of a semester's file, or `None` when it has no goals.
pub fn export_goals(conn: &Connection, semester: &str) -> Result<Option<Mapping>> {
    let goals = goals_of(conn, semester)?;
    if goals == GammaGoals::default() {
        return Ok(None);
    }
    let levels = |map: &BTreeMap<String, i64>| Value::Mapping(map.iter().map(|(k, l)| (Value::String(k.clone()), Value::Number((*l).into()))).collect());
    let mut meta = Mapping::new();
    if !goals.layers.is_empty() {
        frontmatter::set(&mut meta, "layers", levels(&goals.layers));
    }
    if !goals.goals.is_empty() {
        frontmatter::set(&mut meta, "goals", levels(&goals.goals));
    }
    frontmatter::set_opt(&mut meta, "learningPlan", goals.learning_plan.as_deref());
    frontmatter::set_opt(&mut meta, "studyGuide", goals.study_guide.as_deref());
    Ok(Some(meta))
}

/// Reads a semester file's `gamma:` block, leniently: what doesn't hold is left out.
pub fn import_goals(conn: &Connection, semester: &str, meta: &Mapping) -> Result<()> {
    let levels = |key: &str| -> BTreeMap<String, i64> {
        let Some(Value::Mapping(map)) = meta.get(key) else { return BTreeMap::new() };
        map.iter()
            .filter_map(|(k, v)| {
                let level = match v {
                    Value::Number(n) => n.as_i64(),
                    Value::String(s) => s.trim().parse().ok(),
                    _ => None,
                }?;
                Some((k.as_str()?.to_string(), level))
            })
            .collect()
    };
    let raw = GammaGoals { layers: levels("layers"), goals: levels("goals"), learning_plan: meta.text("learningPlan"), study_guide: meta.text("studyGuide") };
    // One bad entry shouldn't cost the rest: check each part on its own.
    let mut goals = GammaGoals::default();
    for (key, level) in raw.layers {
        if let Ok(one) = checked(&GammaGoals { layers: BTreeMap::from([(key, level)]), ..GammaGoals::default() }) {
            goals.layers.extend(one.layers);
        }
    }
    for (key, level) in raw.goals {
        if let Ok(one) = checked(&GammaGoals { goals: BTreeMap::from([(key, level)]), ..GammaGoals::default() }) {
            goals.goals.extend(one.goals);
        }
    }
    goals.learning_plan = checked(&GammaGoals { learning_plan: raw.learning_plan, ..GammaGoals::default() }).ok().and_then(|g| g.learning_plan);
    goals.study_guide = checked(&GammaGoals { study_guide: raw.study_guide, ..GammaGoals::default() }).ok().and_then(|g| g.study_guide);
    store_goals(conn, semester, &goals)
}

/// An evidence item to pick as the learning plan.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceChoice {
    /// "<project>/<evidence slug>", as the goals keep it.
    pub id: String,
    pub name: String,
    pub project_name: String,
}

pub fn evidence_choices(conn: &Connection) -> Result<Vec<EvidenceChoice>> {
    query_all(
        conn,
        "SELECT e.project_slug || '/' || e.slug, e.name, p.name FROM evidence e JOIN projects p ON p.slug = e.project_slug ORDER BY p.name, e.name",
        [],
        |r| Ok(EvidenceChoice { id: r.get(0)?, name: r.get(1)?, project_name: r.get(2)? }),
    )
}

/// The learning plan as the overview shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub project: String,
    pub project_name: String,
    pub evidence: String,
    pub name: String,
    pub thread: Option<String>,
    /// Its newest version in Portflow, and when it was delivered there.
    pub version: Option<i64>,
    pub delivered_at: Option<String>,
}

fn plan_view(conn: &Connection, plan: &str) -> Result<Option<PlanView>> {
    let Some((project, evidence)) = plan.split_once('/') else { return Ok(None) };
    let Some((name, project_name, thread)) = query_opt(
        conn,
        "SELECT e.name, p.name, e.thread_slug FROM evidence e JOIN projects p ON p.slug = e.project_slug WHERE e.project_slug = ?1 AND e.slug = ?2",
        [project, evidence],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
    )?
    else {
        return Ok(None);
    };
    let latest = query_opt(
        conn,
        "SELECT number, created_at FROM evidence_versions WHERE project_slug = ?1 AND evidence_slug = ?2 AND portflow_version IS NOT NULL ORDER BY number DESC LIMIT 1",
        [project, evidence],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    )?;
    Ok(Some(PlanView {
        project: project.to_string(),
        project_name,
        evidence: evidence.to_string(),
        name,
        thread,
        version: latest.as_ref().map(|l| l.0),
        delivered_at: latest.map(|l| l.1),
    }))
}

// ---- reading a sub-goal's name --------------------------------------------------------------------------

/// The architecture layers of HBO-i, in the order Fontys shows them: the letter a sub-goal's name uses, and the name.
pub const LAYERS: [(&str, &str); 5] =
    [("U", "User Interaction"), ("S", "Software"), ("O", "Organisational Processes"), ("H", "Hardware Interfacing"), ("I", "Infrastructure")];

/// The activities, in HBO-i's order, by how their goal's name starts.
const ACTIVITIES: [&str; 5] = ["analy", "advi", "design", "reali", "manag"];

/// What a sub-goal's name says: its layer (one of [`LAYERS`]' letters), if any, and its level. "Analysis-U2" is
/// (Some("U"), 2), "PS-3" is (None, 3); a name with neither is `None`.
pub fn parse_sub_goal(name: &str) -> Option<(Option<&'static str>, i64)> {
    let tail = name.trim().rsplit(['-', ' ', '_']).next()?.trim();
    let digits = tail.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let letters = &tail[..tail.len() - digits.len()];
    let level: i64 = digits.parse().ok().filter(|l| (1..=9).contains(l))?;
    if letters.is_empty() {
        return Some((None, level));
    }
    let layer = LAYERS.iter().find(|(l, _)| l.eq_ignore_ascii_case(letters)).map(|(l, _)| *l)?;
    Some((Some(layer), level))
}

fn activity_rank(name: &str) -> usize {
    let lower = name.trim().to_lowercase();
    ACTIVITIES.iter().position(|a| lower.starts_with(a)).unwrap_or(ACTIVITIES.len())
}

// ---- the matrix -----------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemesterChoice {
    pub id: String,
    pub name: String,
    pub start: String,
    pub end: String,
    pub section: Option<i64>,
    pub section_name: Option<String>,
    pub current: bool,
}

/// One sub-goal: one level of an activity in a layer, or one level of a goal like PS.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelView {
    pub id: i64,
    pub name: String,
    pub level: i64,
    /// Evidence that counts for it: Portflow's count, or the items linked in Ledger when those are more.
    pub evidence_count: i64,
    /// The evidence items in Ledger linked to it (Portflow may count more: those in collections not linked to a project).
    pub evidence: Vec<EvidenceRef>,
    /// Feedback from others on that evidence.
    pub feedback_count: i64,
    pub description_html: String,
    /// The same, as markdown: what a thread or deliverable started from this level quotes.
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    /// The goal (learning outcome) that is this activity.
    pub outcome: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub layer: String,
    pub name: String,
    /// The level aimed for in this layer this semester.
    pub target: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub layer: String,
    pub outcome: i64,
    pub evidence_count: i64,
    pub feedback_count: i64,
    /// The highest level that has evidence.
    pub level: Option<i64>,
    pub levels: Vec<LevelView>,
    /// The level aimed for (its row's), and whether evidence reaches it.
    pub target: Option<i64>,
    pub reached: bool,
}

/// A goal measured in levels only (Professional standard, Personal leadership).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub outcome: i64,
    /// What the goals call it: its nickname, or its name.
    pub key: String,
    pub name: String,
    pub nickname: String,
    pub evidence_count: i64,
    pub level: Option<i64>,
    pub levels: Vec<LevelView>,
    pub target: Option<i64>,
    pub reached: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GammaView {
    pub semesters: Vec<SemesterChoice>,
    /// The semester shown, if there is one.
    pub semester: Option<String>,
    pub section: Option<i64>,
    pub section_name: Option<String>,
    pub goal_set: String,
    /// Whether the outcomes are this semester's (from its section's import). False when the semester has no section, or
    /// its section's goals aren't known yet: then the newest goal set is shown.
    pub scoped: bool,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub cells: Vec<Cell>,
    pub skills: Vec<Skill>,
    /// The highest level any sub-goal has (the legend's length).
    pub max_level: i64,
    pub evidence_count: i64,
    /// Cells with evidence, of all cells that exist in this goal set.
    pub cells_with_evidence: i64,
    pub cells_total: i64,
    /// The semester's goals (empty without a semester), and how many of the cells and goals aimed for are reached.
    pub goals: GammaGoals,
    pub targets_total: i64,
    pub targets_reached: i64,
    /// The learning plan, when it is set and its evidence item is known.
    pub plan: Option<PlanView>,
    /// The projects linked to a collection in this semester: where a thread or deliverable started here goes first.
    pub linked_projects: Vec<String>,
}

fn semester_choices(conn: &Connection, today: &str) -> Result<Vec<SemesterChoice>> {
    query_all(
        conn,
        "SELECT s.id, s.name, s.start_date, s.end_date, l.section_id, p.name FROM semesters s
         LEFT JOIN semester_sections l ON l.semester_id = s.id LEFT JOIN portflow_sections p ON p.id = l.section_id
         ORDER BY s.start_date DESC",
        [],
        |r| {
            let start: String = r.get(2)?;
            let end: String = r.get(3)?;
            let current = start.as_str() <= today && today <= end.as_str();
            Ok(SemesterChoice { id: r.get(0)?, name: r.get(1)?, start, end, section: r.get(4)?, section_name: r.get(5)?, current })
        },
    )
}

/// Feedback from others per outcome or sub-goal id, on the evidence linked to it.
fn feedback_counts(conn: &Connection, me: i64) -> Result<HashMap<i64, i64>> {
    Ok(query_all(
        conn,
        "SELECT o.outcome_id, COUNT(DISTINCT f.id) FROM feedback f
         JOIN evidence_outcomes o ON o.project_slug = f.project_slug AND o.evidence_slug = f.evidence
         WHERE COALESCE(f.from_user, -1) <> ?1 GROUP BY o.outcome_id",
        [me],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?
    .into_iter()
    .collect())
}

/// Gamma, the competence matrix of `semester`, or of the current one (the semester today falls in). `today`: "2026-11-04".
/// `me`: the student's Portflow id, whose own comments don't count as feedback.
pub fn gamma(conn: &Connection, semester: Option<&str>, today: &str, me: i64) -> Result<GammaView> {
    let semesters = semester_choices(conn, today)?;
    let chosen = match semester.filter(|s| !s.is_empty()) {
        Some(id) => Some(semesters.iter().find(|s| s.id == id).ok_or_else(|| Error::not_found("Semester not found"))?),
        None => semesters.iter().find(|s| s.current),
    };
    let section = chosen.and_then(|s| s.section);
    let semester_goals = match chosen {
        Some(s) => goals_of(conn, &s.id)?,
        None => GammaGoals::default(),
    };
    let import: Option<i64> = match section {
        Some(section) => query_opt(conn, "SELECT import_id FROM portflow_sections WHERE id = ?1", [section], |r| r.get::<_, Option<i64>>(0))?.flatten(),
        None => None,
    };

    // The goals of this semester: those that came with its section. Without that, the newest goal set.
    type Goal = (i64, String, String, String);
    let goals_of = |import: i64| -> Result<Vec<Goal>> {
        query_all(conn, "SELECT id, name, nickname, goal_set FROM outcomes WHERE removed = 0 AND import_id = ?1 ORDER BY position, id", [import], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
    };
    let mut scoped = false;
    let mut goals: Vec<Goal> = vec![];
    if let Some(import) = import {
        goals = goals_of(import)?;
        scoped = !goals.is_empty();
    }
    if goals.is_empty() {
        let newest: Option<i64> = conn.query_row("SELECT MAX(import_id) FROM outcomes WHERE removed = 0", [], |r| r.get(0))?;
        goals = match newest {
            Some(import) => goals_of(import)?,
            None => query_all(conn, "SELECT id, name, nickname, goal_set FROM outcomes WHERE removed = 0 ORDER BY position, id", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?,
        };
    }
    let goal_set = goals.iter().map(|g| g.3.clone()).find(|s| !s.is_empty()).unwrap_or_default();

    let mut evidence = crate::portflow_store::evidence_for(conn)?;
    let feedback = feedback_counts(conn, me)?;
    type Sub = (i64, i64, String, String, i64);
    let mut subs: HashMap<i64, Vec<Sub>> = HashMap::new();
    for sub in query_all::<Sub, _>(conn, "SELECT outcome_id, id, name, description, evidence_count FROM sub_outcomes ORDER BY outcome_id, position, id", [], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })? {
        subs.entry(sub.0).or_default().push(sub);
    }

    let mut max_level = 0;
    let mut columns: Vec<(usize, i64, Column)> = vec![];
    let mut cells: HashMap<(String, i64), Vec<LevelView>> = HashMap::new();
    let mut skills = vec![];
    for (position, (id, name, nickname, _)) in goals.iter().enumerate() {
        let mut layered = vec![];
        let mut levelled = vec![];
        for (_, sub_id, sub_name, description, count) in subs.remove(id).unwrap_or_default() {
            let Some((layer, level)) = parse_sub_goal(&sub_name) else { continue };
            max_level = max_level.max(level);
            let linked = evidence.remove(&sub_id).unwrap_or_default();
            let view = LevelView {
                id: sub_id,
                name: sub_name,
                level,
                evidence_count: count.max(linked.len() as i64),
                evidence: linked,
                feedback_count: feedback.get(&sub_id).copied().unwrap_or(0),
                description_html: markdown::render(&description),
                description,
            };
            match layer {
                Some(layer) => layered.push((layer, view)),
                None => levelled.push(view),
            }
        }
        if !layered.is_empty() {
            columns.push((activity_rank(name), position as i64, Column { outcome: *id, name: name.clone() }));
            for (layer, view) in layered {
                cells.entry((layer.to_string(), *id)).or_default().push(view);
            }
        } else {
            levelled.sort_by_key(|l| l.level);
            let evidence_count = levelled.iter().map(|l| l.evidence_count).sum::<i64>().max(evidence.get(id).map_or(0, |e| e.len() as i64));
            let level = levelled.iter().filter(|l| l.evidence_count > 0).map(|l| l.level).max();
            let key = if nickname.trim().is_empty() { name.clone() } else { nickname.clone() };
            let target = semester_goals.goals.get(&key).copied();
            let reached = target.is_some_and(|t| level.is_some_and(|l| l >= t));
            skills.push(Skill { outcome: *id, key, name: name.clone(), nickname: nickname.clone(), evidence_count, level, levels: levelled, target, reached });
        }
    }
    columns.sort_by_key(|c| (c.0, c.1));
    let columns: Vec<Column> = columns.into_iter().map(|c| c.2).collect();

    let used_layers: BTreeSet<String> = cells.keys().map(|(l, _)| l.clone()).collect();
    let rows: Vec<Row> = LAYERS
        .iter()
        .filter(|(l, _)| used_layers.contains(*l))
        .map(|(l, n)| Row { layer: l.to_string(), name: n.to_string(), target: semester_goals.layers.get(*l).copied() })
        .collect();
    let mut out_cells = vec![];
    for row in &rows {
        for column in &columns {
            let Some(mut levels) = cells.remove(&(row.layer.clone(), column.outcome)) else { continue };
            levels.sort_by_key(|l| l.level);
            let level = levels.iter().filter(|l| l.evidence_count > 0).map(|l| l.level).max();
            out_cells.push(Cell {
                layer: row.layer.clone(),
                outcome: column.outcome,
                evidence_count: levels.iter().map(|l| l.evidence_count).sum(),
                feedback_count: levels.iter().map(|l| l.feedback_count).sum(),
                level,
                levels,
                target: row.target,
                reached: row.target.is_some_and(|t| level.is_some_and(|l| l >= t)),
            });
        }
    }
    let cells_total = out_cells.len() as i64;
    let cells_with_evidence = out_cells.iter().filter(|c| c.evidence_count > 0).count() as i64;
    let evidence_count = out_cells.iter().map(|c| c.evidence_count).sum::<i64>() + skills.iter().map(|s| s.evidence_count).sum::<i64>();
    let aimed = out_cells.iter().filter(|c| c.target.is_some()).map(|c| c.reached).chain(skills.iter().filter(|s| s.target.is_some()).map(|s| s.reached));
    let (targets_total, targets_reached) = aimed.fold((0, 0), |(n, r), reached| (n + 1, r + i64::from(reached)));
    let linked_projects: Vec<String> = match chosen {
        Some(s) => query_all(conn, "SELECT project_slug FROM project_collections WHERE semester_id = ?1 ORDER BY project_slug", [&s.id], |r| r.get(0))?,
        None => vec![],
    };
    let plan = match semester_goals.learning_plan.as_deref() {
        Some(plan) => plan_view(conn, plan)?,
        None => None,
    };

    Ok(GammaView {
        semester: chosen.map(|s| s.id.clone()),
        section,
        section_name: chosen.and_then(|s| s.section_name.clone()),
        semesters,
        goal_set,
        scoped,
        columns,
        rows,
        cells: out_cells,
        skills,
        max_level,
        evidence_count,
        cells_with_evidence,
        cells_total,
        goals: semester_goals,
        targets_total,
        targets_reached,
        plan,
        linked_projects,
    })
}
