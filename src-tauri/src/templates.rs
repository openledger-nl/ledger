//! Templates shape deliverables: a name, a "use when…" hint, guiding questions to answer first, the skeleton of the
//! deliverable itself, the kinds of project it is for and who its deliverables are written for (its readers). A
//! vault-wide template (`shared/templates/prompts/<id>.md`, scope "") is available in every project of its kinds; a project's own
//! (`projects/<p>/templates/prompts/<id>.md`) is available there, and replaces a vault-wide one with the same id.
//!
//! A new vault starts with one template, a basic project plan (`starter`), written to
//! `shared/templates/prompts/project-plan.md` when the vault is made. From then on it is the vault's like any other:
//! changed, it stays changed, and deleted, it is gone.
//!
//! The readers are told to Claude in a prompt's base, and written into a new deliverable as a `%% Readers: … %%` note
//! (only seen in source mode), with a visible `**Readers:**` line as well when the template says so.

use rusqlite::{Connection, params};
use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::frontmatter::{self, MetaExt};
use crate::models::{TemplateDraft, TemplateReader, TemplateView};
use crate::{ids, project_facts, projects};

const SELECT: &str = "SELECT scope, id, name, \"trigger\", body, kinds, readers, readers_shown, created_at, updated_at FROM templates";

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TemplateView> {
    let kinds: String = r.get(5)?;
    let readers: String = r.get(6)?;
    Ok(TemplateView {
        scope: r.get(0)?,
        id: r.get(1)?,
        name: r.get(2)?,
        trigger: r.get(3)?,
        body: r.get(4)?,
        kinds: kinds.split(',').filter(|k| !k.is_empty()).map(str::to_string).collect(),
        readers: serde_json::from_str(&readers).unwrap_or_default(),
        readers_shown: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
        questions: vec![],
        overridden: false,
    })
}

fn with_questions(conn: &Connection, mut templates: Vec<TemplateView>) -> Result<Vec<TemplateView>> {
    for t in &mut templates {
        t.questions = questions_of(conn, &t.scope, &t.id)?;
    }
    Ok(templates)
}

pub fn questions_of(conn: &Connection, scope: &str, id: &str) -> Result<Vec<String>> {
    query_all(conn, "SELECT question FROM template_questions WHERE scope = ?1 AND template_id = ?2 ORDER BY position", [scope, id], |r| r.get(0))
}

/// The templates of exactly this scope ("" for the vault-wide ones), by name.
pub fn list(conn: &Connection, scope: &str) -> Result<Vec<TemplateView>> {
    let rows = query_all(conn, &format!("{SELECT} WHERE scope = ?1 ORDER BY name COLLATE NOCASE, id"), [scope], from_row)?;
    with_questions(conn, rows)
}

/// Whether a vault-wide template is offered in a project of this kind (a project without a kind gets them all).
fn serves(template: &TemplateView, kind: Option<&str>) -> bool {
    match kind {
        Some(kind) => template.kinds.is_empty() || template.kinds.iter().any(|k| k == kind),
        None => true,
    }
}

/// What the project's templates screen shows: its own, then the vault-wide ones for its kind of project (marked if the
/// project replaced them).
pub fn for_project(conn: &Connection, project: &str) -> Result<Vec<TemplateView>> {
    let kind = project_facts::get(conn, project)?.map(|m| m.kind);
    let own = list(conn, project)?;
    let mut inherited: Vec<TemplateView> = list(conn, "")?.into_iter().filter(|t| serves(t, kind.as_deref())).collect();
    for t in &mut inherited {
        t.overridden = own.iter().any(|o| o.id == t.id);
    }
    Ok(own.into_iter().chain(inherited).collect())
}

/// What a prompt or a new deliverable in the project can start from: its own templates and the vault-wide ones for its
/// kind that it hasn't replaced.
pub fn effective(conn: &Connection, project: &str) -> Result<Vec<TemplateView>> {
    let mut all: Vec<TemplateView> = for_project(conn, project)?.into_iter().filter(|t| !t.overridden).collect();
    all.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(all)
}

pub fn get(conn: &Connection, scope: &str, id: &str) -> Result<TemplateView> {
    let row = query_opt(conn, &format!("{SELECT} WHERE scope = ?1 AND id = ?2"), [scope, id], from_row)?
        .ok_or_else(|| Error::not_found("Template not found"))?;
    Ok(with_questions(conn, vec![row])?.remove(0))
}

/// The template a project's prompt means by `id`: its own, else the vault-wide one.
pub fn find(conn: &Connection, project: &str, id: &str) -> Result<TemplateView> {
    get(conn, project, id).or_else(|_| get(conn, "", id))
}

pub fn name_of(conn: &Connection, project: &str, id: &str) -> Result<Option<String>> {
    Ok(find(conn, project, id).ok().map(|t| t.name))
}

/// The kinds as kept: known ones only, each once, in the order of `project_facts::KINDS`. All of them is the same as none.
fn clean_kinds(kinds: &[String]) -> String {
    let chosen: Vec<&str> = project_facts::KINDS.iter().copied().filter(|k| kinds.iter().any(|c| c.trim() == *k)).collect();
    if chosen.len() == project_facts::KINDS.len() { String::new() } else { chosen.join(",") }
}

/// Readers without a `who` are dropped, and the same one is listed once.
fn clean_readers(readers: &[TemplateReader]) -> Vec<TemplateReader> {
    let mut out: Vec<TemplateReader> = vec![];
    for r in readers {
        let who = r.who.trim();
        if who.is_empty() || out.iter().any(|o| o.who.eq_ignore_ascii_case(who)) {
            continue;
        }
        out.push(TemplateReader { who: who.to_string(), needs: one_line(&r.needs), keep_from: one_line(&r.keep_from) });
    }
    out
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn readers_json(readers: &[TemplateReader]) -> String {
    serde_json::to_string(&clean_readers(readers)).unwrap_or_else(|_| "[]".into())
}

pub fn create(conn: &Connection, scope: &str, draft: &TemplateDraft) -> Result<TemplateView> {
    if !scope.is_empty() {
        projects::require(conn, scope)?;
    }
    let name = draft.name.trim();
    if name.is_empty() {
        return Err(Error::invalid("Name is required"));
    }
    let id = unique_id(conn, scope, name)?;
    let tx = conn.unchecked_transaction()?;
    insert(&tx, scope, &id, &TemplateDraft { name: name.to_string(), ..draft.clone() }, &ids::now())?;
    tx.commit()?;
    get(conn, scope, &id)
}

fn insert(conn: &Connection, scope: &str, id: &str, draft: &TemplateDraft, at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO templates (scope, id, name, \"trigger\", body, kinds, readers, readers_shown, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            scope,
            id,
            draft.name.trim(),
            draft.trigger.trim(),
            draft.body.trim_end(),
            clean_kinds(&draft.kinds),
            readers_json(&draft.readers),
            draft.readers_shown,
            at
        ],
    )?;
    write_questions(conn, scope, id, &draft.questions)
}

/// What an existing template says, as a draft to change or copy.
pub fn draft_of(template: &TemplateView) -> TemplateDraft {
    TemplateDraft {
        name: template.name.clone(),
        trigger: template.trigger.clone(),
        questions: template.questions.clone(),
        body: template.body.clone(),
        kinds: template.kinds.clone(),
        readers: template.readers.clone(),
        readers_shown: template.readers_shown,
    }
}

/// Copies a vault-wide template into the project under the same id, so the project can reword it for itself.
pub fn override_global(conn: &Connection, project: &str, id: &str) -> Result<TemplateView> {
    projects::require(conn, project)?;
    let global = get(conn, "", id)?;
    if get(conn, project, id).is_ok() {
        return Err(Error::invalid("This project already has its own version of that template"));
    }
    let tx = conn.unchecked_transaction()?;
    insert(&tx, project, id, &draft_of(&global), &ids::now())?;
    tx.commit()?;
    get(conn, project, id)
}

/// Replaces what the template says.
pub fn update(conn: &Connection, scope: &str, id: &str, draft: &TemplateDraft) -> Result<TemplateView> {
    get(conn, scope, id)?; // not found if it does not exist
    let name = draft.name.trim();
    if name.is_empty() {
        return Err(Error::invalid("Name is required"));
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE templates SET name = ?3, \"trigger\" = ?4, body = ?5, kinds = ?6, readers = ?7, readers_shown = ?8, updated_at = ?9
         WHERE scope = ?1 AND id = ?2",
        params![
            scope,
            id,
            name,
            draft.trigger.trim(),
            draft.body.trim_end(),
            clean_kinds(&draft.kinds),
            readers_json(&draft.readers),
            draft.readers_shown,
            ids::now()
        ],
    )?;
    write_questions(&tx, scope, id, &draft.questions)?;
    tx.commit()?;
    get(conn, scope, id)
}

/// Prompts that were started from a template keep what they were given, so a template can always be deleted, the
/// project plan a vault starts with too.
pub fn delete(conn: &Connection, scope: &str, id: &str) -> Result<()> {
    conn.execute("DELETE FROM templates WHERE scope = ?1 AND id = ?2", [scope, id])?;
    Ok(())
}

/// Blank questions are dropped; the order the caller gave is kept.
fn write_questions(conn: &Connection, scope: &str, id: &str, questions: &[String]) -> Result<()> {
    conn.execute("DELETE FROM template_questions WHERE scope = ?1 AND template_id = ?2", [scope, id])?;
    let cleaned = questions.iter().map(|q| q.trim()).filter(|q| !q.is_empty());
    for (position, question) in cleaned.enumerate() {
        conn.execute(
            "INSERT INTO template_questions (scope, template_id, position, question) VALUES (?1, ?2, ?3, ?4)",
            params![scope, id, position as i64, question],
        )?;
    }
    Ok(())
}

fn unique_id(conn: &Connection, scope: &str, name: &str) -> Result<String> {
    let base = ids::slugify_as(name, "template");
    ids::numbered(&base, |id| Ok(query_opt(conn, "SELECT 1 FROM templates WHERE scope = ?1 AND id = ?2", [scope, id], |_| Ok(()))?.is_some()))
}

// ---- in the files -----------------------------------------------------------------------------------

/// The frontmatter a template keeps its kinds and readers in:
///
/// ```text
/// kinds: [study, work]             left out for every kind
/// readers:
/// - who: client                    a preset, or someone in your own words
///   needs: reads the summary first
///   keepFrom: the rate
/// - me                             a reader with nothing more to say about them
/// readersShown: true               a visible Readers line in new deliverables, not only the %% note
/// ```
pub fn export_meta(template: &TemplateView, meta: &mut Mapping) {
    frontmatter::set_list(meta, "kinds", &template.kinds);
    if !template.readers.is_empty() {
        let items = template
            .readers
            .iter()
            .map(|r| {
                if r.needs.is_empty() && r.keep_from.is_empty() {
                    return Value::String(r.who.clone());
                }
                let mut item = Mapping::new();
                frontmatter::set(&mut item, "who", r.who.clone());
                frontmatter::set_opt(&mut item, "needs", Some(&r.needs));
                frontmatter::set_opt(&mut item, "keepFrom", Some(&r.keep_from));
                Value::Mapping(item)
            })
            .collect();
        frontmatter::set(meta, "readers", Value::Sequence(items));
    }
    frontmatter::set_flag(meta, "readersShown", template.readers_shown);
}

/// The kinds, readers and `readersShown` from a template file's frontmatter, as stored.
pub fn import_meta(meta: &Mapping) -> (String, String, bool) {
    let readers = match meta.get("readers") {
        Some(Value::Sequence(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Mapping(m) => Some(TemplateReader {
                    who: m.text("who").unwrap_or_default(),
                    needs: m.text("needs").unwrap_or_default(),
                    keep_from: m.text("keepFrom").unwrap_or_default(),
                }),
                Value::String(who) => Some(TemplateReader { who: who.clone(), ..Default::default() }),
                _ => None,
            })
            .collect(),
        Some(Value::String(who)) => vec![TemplateReader { who: who.clone(), ..Default::default() }],
        _ => vec![],
    };
    (clean_kinds(&meta.list("kinds")), readers_json(&readers), meta.flag("readersShown"))
}

// ---- readers ----------------------------------------------------------------------------------------

/// A reader the editor offers by name: what the deliverable is for to them, as Claude is told it.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub hint: &'static str,
}

/// `people` stands for whoever the project lists under its people, each read as their role's preset.
pub const PRESETS: [Preset; 7] = [
    Preset {
        id: "me",
        name: "Me, later",
        hint: "knows the project, but not what was on their mind today: wants the decisions, why they were made and where things are, kept short",
    },
    Preset {
        id: "people",
        name: "The project's people",
        hint: "whoever the project lists as its coach, teammates and stakeholders",
    },
    Preset {
        id: "client",
        name: "Client",
        hint: "not technical; decides on scope, money and time; reads the summary first, so plain language and no jargon",
    },
    Preset {
        id: "coach",
        name: "Coach or assessor",
        hint: "judges the work against the learning outcomes: wants the reasoning, the sources and the reflection to be visible",
    },
    Preset { id: "team", name: "Teammates", hint: "share the context: need who does what, by when, and what was decided" },
    Preset {
        id: "developer",
        name: "Developer or maintainer",
        hint: "technical: needs exact, testable statements, how it is built and how to run it",
    },
    Preset {
        id: "outsider",
        name: "Outside reviewer",
        hint: "has no inside context and sees nothing confidential: every term explained, every claim backed",
    },
];

fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// The preset a person of the project reads as, by their role.
fn role_preset(role: &str) -> Option<&'static Preset> {
    match role {
        "coach" | "expert" => preset("coach"),
        "teammate" | "colleague" => preset("team"),
        "stakeholder" => preset("client"),
        _ => None,
    }
}

/// The readers as they are said: a name and what to know about them (the preset's hint, what the template adds, and
/// what to keep from them). `people` becomes the project's people, one by one; with none listed it says so.
pub fn resolve(conn: &Connection, project: Option<&str>, readers: &[TemplateReader]) -> Result<Vec<(String, String)>> {
    let mut out = vec![];
    for r in readers {
        let extra = |base: &str| {
            let mut parts: Vec<String> = [base, r.needs.as_str()].iter().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            if !r.keep_from.is_empty() {
                parts.push(format!("keep from them: {}", r.keep_from));
            }
            parts.join("; ")
        };
        if r.who == "people" {
            let people = match project {
                Some(p) => project_facts::people(conn, Some(p))?,
                None => vec![],
            };
            for person in &people {
                if out.iter().any(|(n, _): &(String, String)| n == &person.name) {
                    continue;
                }
                let role = person.role.as_str();
                let hint = role_preset(role).map(|p| format!("{role}: {}", p.hint)).unwrap_or_else(|| role.to_string());
                out.push((person.name.clone(), extra(&hint)));
            }
            if people.is_empty() && project.is_some() {
                out.push(("The project's people".into(), extra("none are listed yet")));
            }
            continue;
        }
        match preset(&r.who) {
            Some(p) => out.push((p.name.to_string(), extra(p.hint))),
            None => out.push((r.who.clone(), extra(""))),
        }
    }
    Ok(out)
}

/// The readers in one line: "Client (not technical; …); Me, later (…)".
pub fn describe(resolved: &[(String, String)]) -> String {
    resolved.iter().map(|(name, about)| if about.is_empty() { name.clone() } else { format!("{name} ({about})") }).collect::<Vec<_>>().join("; ")
}

/// What a new deliverable from this template starts with: its skeleton, with the readers noted under its title (the
/// first paragraph, when the skeleton opens with a `# ` heading) or at the top.
pub fn skeleton(conn: &Connection, project: &str, template: &TemplateView) -> Result<String> {
    let body = template.body.trim_end();
    let resolved = resolve(conn, Some(project), &template.readers)?;
    if resolved.is_empty() {
        return Ok(body.to_string());
    }
    let mut note = format!("%% Readers: {} %%", describe(&resolved));
    if template.readers_shown {
        let names: Vec<&str> = resolved.iter().map(|(n, _)| n.as_str()).collect();
        note = format!("**Readers:** {}\n{note}", names.join(", "));
    }
    if !body.starts_with("# ") {
        return Ok(if body.is_empty() { note } else { format!("{note}\n\n{body}") });
    }
    // Under the title and whatever lines follow it straight away (tags, a version line), as one paragraph.
    let lines: Vec<&str> = body.lines().collect();
    let end = lines.iter().skip(1).position(|l| l.trim().is_empty()).map_or(lines.len(), |i| i + 1);
    let (head, rest) = lines.split_at(end);
    let mut out = format!("{}\n{note}", head.join("\n"));
    if !rest.is_empty() {
        out.push('\n');
        out.push_str(&rest.join("\n"));
    }
    Ok(out)
}

/// The skeleton a new deliverable in the project starts from, for the template the project means by `id`.
pub fn skeleton_for(conn: &Connection, project: &str, id: &str) -> Result<String> {
    skeleton(conn, project, &find(conn, project, id)?)
}

// ---- the template a new vault starts with -------------------------------------------------------------

const PLAN_BODY: &str = r#"# Project plan

**Version:** 0.1, concept

%% What: what you are going to do, how, when, with whom and with what. Write it once the goal is clear; keep it to three pages at most.
Why: it is what you, and anyone who joins, fall back on when it isn't clear what comes next. Each heading is a short paragraph or a list; a heading that doesn't apply says "Not applicable." so the reader sees you thought about it. %%

## Goal
%% What: in two or three sentences, what is true at the end that isn't true now, and for whom. Why: every choice below is checked against this.
Example: At the end of January the bakery's twelve staff see their shifts on their phones, and the owner no longer fills in a spreadsheet on Sunday evenings. %%

## Background
%% What: how it goes now, and why that no longer works. Facts, not opinions. Why: without the problem nobody can judge whether the plan solves it. %%

## Scope
%% What: what is in, as a short list. Why: the scope is the line you fall back on when "surely that was part of it too?" comes up. %%

### Out of scope
%% What: what someone might expect but you won't do, each with a word on why. Why: when someone asks why it isn't in, the answer is already here. %%

## Result
%% What: what you hand over, each with when it counts as done ("Done when: ...", something you can check). Why: done is something you check, not something you feel. %%

## Approach and planning
%% What: the steps or sprints in order, each with a date or a week and what it delivers. Name the moments someone else looks at the work (a review, a demo, an assessment). Why: a plan without dates is a wish. %%

## People and roles
%% What: who is involved, what each does and who decides. Why: most delays are someone waiting for a decision nobody knew was theirs. %%

## Risks
%% What: what could go wrong, how likely it is, and what you do about it. Think of time, money, people who drop out and things you depend on. Why: a risk that is written down is one you can still do something about. %%

## Communication
%% What: how and how often you keep each other informed, and where things are written down. Why: it saves the "I didn't know" at the end. %%
"#;

/// The project plan a new vault starts with.
pub fn starter() -> TemplateDraft {
    TemplateDraft {
        name: "Project plan".into(),
        trigger: "a project starts, or its goal has just become clear".into(),
        questions: vec![
            "What should be true at the end that isn't now, and for whom?".into(),
            "What is the deadline, and what happens if it slips?".into(),
            "Who decides when it is done?".into(),
        ],
        body: PLAN_BODY.into(),
        kinds: vec![],
        readers: vec![
            TemplateReader { who: "people".into(), ..Default::default() },
            TemplateReader { who: "me".into(), ..Default::default() },
        ],
        readers_shown: false,
    }
}

/// Gives a new vault its starter template, as a vault-wide template (`project-plan`).
pub fn seed(conn: &Connection) -> Result<()> {
    create(conn, "", &starter()).map(|_| ())
}
