//! A prompt is a request for a deliverable, written in a thread. Whatever produces the deliverable (a person, or
//! an assistant reading the vault) leaves the file in the project's deliverables folder; the prompt keeps the
//! list of the deliverables it is about.
//!
//! What a prompt says is two texts. The **base** is filled in for the person, from the template a new deliverable starts
//! from or from the feedback the prompt is to resolve, and can be edited; the **body** is what the person adds in their own
//! words. A prompt has no title to write: it is named from what it asks (see `claude_code::naming`), and until that has
//! happened, from the start of its text.
//!
//! A prompt belongs to the deliverable, not to a version of it: it is linked by the live path, and the versions it
//! produced know it by their own records (see `provenance`). It can also be written to resolve pieces of feedback, which
//! it keeps the ids of.

use std::collections::HashSet;

use rusqlite::{Connection, Row, params};

use crate::claude_code::records;
use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::models::{BaseSpec, DeliverablePrompt, FeedbackView, NewPrompt, PromptDeliverable, PromptView};
use crate::posts::{self, AddOptions};
use crate::{deliverables, feedback, ids, markdown, templates, threads};

const COLUMNS: &str = "id, thread_slug, title, template_id, created_at, updated_at, base, body";

fn from_row(r: &Row<'_>) -> rusqlite::Result<PromptView> {
    let base: String = r.get(6)?;
    let body: String = r.get(7)?;
    Ok(PromptView {
        id: r.get(0)?,
        thread_slug: r.get(1)?,
        title: r.get(2)?,
        template_id: r.get(3)?,
        template_name: None,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        base_html: markdown::render(&base),
        base,
        html: markdown::render(&body),
        body,
        deliverables: vec![],
        feedback: vec![],
        runs: vec![],
    })
}

/// Fills in what lives in other tables: the template's name, the deliverables, the feedback.
fn complete(conn: &Connection, project: &str, mut prompt: PromptView) -> Result<PromptView> {
    prompt.template_name = prompt.template_id.as_deref().and_then(|id| templates::name_of(conn, project, id).ok().flatten());
    let paths: Vec<String> = query_all(conn, "SELECT path FROM prompt_deliverables WHERE prompt_id = ?1 ORDER BY position", [&prompt.id], |r| r.get(0))?;
    for path in paths {
        let exists = deliverables::exists(conn, project, &path)?;
        let versions = deliverables::versions_of(conn, project, &path)?;
        prompt.deliverables.push(PromptDeliverable { path, exists, versions });
    }
    prompt.feedback = query_all(conn, "SELECT feedback_id FROM prompt_feedback WHERE prompt_id = ?1 ORDER BY position", [&prompt.id], |r| r.get(0))?;
    prompt.runs = records::for_prompt(conn, project, &prompt.thread_slug, &prompt.id)?;
    Ok(prompt)
}

/// Oldest first.
pub fn list(conn: &Connection, project: &str, thread: &str) -> Result<Vec<PromptView>> {
    let rows = query_all(
        conn,
        &format!("SELECT {COLUMNS} FROM prompts WHERE project_slug = ?1 AND thread_slug = ?2 ORDER BY created_at, rowid"),
        [project, thread],
        from_row,
    )?;
    rows.into_iter().map(|p| complete(conn, project, p)).collect()
}

pub fn get(conn: &Connection, project: &str, thread: &str, id: &str) -> Result<PromptView> {
    let prompt = query_opt(
        conn,
        &format!("SELECT {COLUMNS} FROM prompts WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3"),
        [project, thread, id],
        from_row,
    )?
    .ok_or_else(|| Error::not_found("Prompt not found"))?;
    complete(conn, project, prompt)
}

/// What a prompt asks, as one text: the base, then what the person added.
pub fn request_text(prompt: &PromptView) -> String {
    [prompt.base.trim(), prompt.body.trim()].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n\n")
}

// ---- writing a prompt -------------------------------------------------------------------------------

/// One line of text: names are the user's, and a stray newline in one must not read as an instruction.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A code fence longer than any run of backticks inside `text`, so quoting a template can't end early.
fn fence_for(text: &str) -> String {
    let (mut longest, mut run) = (0, 0);
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    "`".repeat((longest + 1).max(3))
}

/// The base of a prompt for a new deliverable that starts from this template: its name and its structure.
fn template_text(conn: &Connection, project: &str, id: &str) -> Result<String> {
    let template = templates::find(conn, project, id).map_err(|_| Error::invalid("Template not found"))?;
    let intro = format!("Create the deliverable from the template “{}”", one_line(&template.name));
    let body = template.body.trim_end();
    if body.trim().is_empty() {
        return Ok(format!("{intro}."));
    }
    let fence = fence_for(body);
    Ok(format!("{intro}. Use this structure:\n\n{fence}markdown\n{body}\n{fence}"))
}

/// Pieces of feedback as a prompt says them: each with the version it was given on, the passage, and what was said. With
/// `intro`, the text that opens a prompt that is to resolve them; without, what adds them to one that is written.
fn feedback_text(conn: &Connection, project: &str, ids: &[String], intro: bool) -> Result<String> {
    let mut blocks = vec![];
    let mut name = String::new();
    for id in ids {
        let item: FeedbackView = feedback::find(conn, project, id)?;
        if name.is_empty() {
            let live = deliverables::split_version(&item.deliverable_path).0;
            name = live.rsplit('/').next().unwrap_or(&live).to_string();
        }
        blocks.push(feedback::block(conn, project, &item)?);
    }
    let blocks = blocks.join("\n\n");
    Ok(if intro {
        format!(
            "Please resolve the following feedback on {name}. Each piece says which version it was given on: read that version to see what was meant, and make the changes in the live file.\n\n{blocks}"
        )
    } else {
        format!("More feedback to resolve:\n\n{blocks}")
    })
}

/// What a new prompt's base starts as: the feedback it is to resolve, else the template's structure, else nothing.
pub fn suggest_base(conn: &Connection, project: &str, spec: &BaseSpec) -> Result<String> {
    if !spec.feedback.is_empty() {
        return feedback_text(conn, project, &spec.feedback, true);
    }
    match spec.template_id.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(id) => template_text(conn, project, id),
        None => Ok(String::new()),
    }
}

/// Writes the request. All of it is checked before anything is written, so no prompt is left half made: the deliverable it
/// asks changes to (given as any of its paths; it is linked as the live file) must exist, or the name of a new one to start
/// for it is used (left blank, whatever produces the prompt names the file), and the feedback must be there. Started from a
/// template, the template's guiding questions are added to the thread as posts to be answered, a one-time snapshot.
pub fn create(conn: &Connection, project: &str, thread: &str, draft: &NewPrompt) -> Result<PromptView> {
    threads::require(conn, project, thread)?;
    let (base, body) = (draft.base.trim(), draft.body.trim());
    if base.is_empty() && body.is_empty() {
        return Err(Error::invalid("Say what you want delivered"));
    }
    let template = match draft.template_id.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(id) => Some(templates::find(conn, project, id).map_err(|_| Error::invalid("Template not found"))?),
        None => None,
    };
    let existing = draft.deliverable.as_deref().map(str::trim).filter(|d| !d.is_empty()).map(|d| deliverables::split_version(d).0);
    let new_name = draft.new_deliverable.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if let Some(path) = &existing
        && !deliverables::exists(conn, project, path)?
    {
        return Err(Error::not_found("Deliverable not found"));
    }
    if existing.is_some() && new_name.is_some() {
        return Err(Error::invalid("A prompt asks for changes to a deliverable that exists, or starts a new one, not both"));
    }
    for id in &draft.feedback {
        feedback::require(conn, project, id)?;
    }

    let tx = conn.unchecked_transaction()?;
    if let Some(template) = &template {
        for question in &template.questions {
            posts::add(
                &tx,
                project,
                thread,
                &format!("**Guiding question ({}):** {question}", template.name),
                AddOptions { guiding_question: true, ..Default::default() },
            )?;
        }
    }
    let ids::Stamp { id, at } = ids::new_stamp();
    let title = display_title("", if body.is_empty() { base } else { body });
    tx.execute(
        "INSERT INTO prompts (id, project_slug, thread_slug, title, template_id, created_at, updated_at, base, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8)",
        params![id, project, thread, title, template.as_ref().map(|t| t.id.as_str()), at, base, body],
    )?;
    tx.commit()?;

    if let Some(path) = &existing {
        link_deliverable(conn, project, thread, &id, path)?;
    }
    if let Some(name) = new_name {
        // The file starts from the template's structure, when the prompt has one.
        let skeleton = template.as_ref().map(|t| t.body.clone()).unwrap_or_default();
        let created = deliverables::create(conn, project, name, &skeleton)?;
        link_deliverable(conn, project, thread, &id, &created.path)?;
    }
    link_feedback(conn, &id, &draft.feedback)?;
    get(conn, project, thread, &id)
}

/// A test's way of making a prompt with a name of its own.
#[cfg(test)]
pub fn create_titled(conn: &Connection, project: &str, thread: &str, title: &str, body: &str, template_id: Option<&str>) -> Result<PromptView> {
    let made = create(conn, project, thread, &NewPrompt { body: body.to_string(), template_id: template_id.map(str::to_string), ..Default::default() })?;
    if !title.trim().is_empty() {
        set_title(conn, project, thread, &made.id, title)?;
    }
    get(conn, project, thread, &made.id)
}

/// Changes the base and/or what the person added. A prompt must keep saying something.
pub fn update(conn: &Connection, project: &str, thread: &str, id: &str, base: Option<&str>, body: Option<&str>) -> Result<PromptView> {
    let current = get(conn, project, thread, id)?; // not found if it does not exist
    let base = base.map_or(current.base.trim(), str::trim);
    let body = body.map_or(current.body.trim(), str::trim);
    if base.is_empty() && body.is_empty() {
        return Err(Error::invalid("Say what you want delivered"));
    }
    conn.execute("UPDATE prompts SET base = ?2, body = ?3, updated_at = ?4 WHERE id = ?1", params![id, base, body, ids::now()])?;
    get(conn, project, thread, id)
}

/// Names the prompt. Not an edit of it: its "edited" time stays.
pub fn set_title(conn: &Connection, project: &str, thread: &str, id: &str, title: &str) -> Result<()> {
    let title = one_line(title);
    if title.is_empty() {
        return Err(Error::invalid("A name is needed"));
    }
    let changed = conn.execute("UPDATE prompts SET title = ?4 WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3", params![project, thread, id, title])?;
    if changed == 0 {
        return Err(Error::not_found("Prompt not found"));
    }
    Ok(())
}

pub fn delete(conn: &Connection, project: &str, thread: &str, id: &str) -> Result<()> {
    conn.execute("DELETE FROM prompts WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3", [project, thread, id])?;
    Ok(())
}

// ---- what a prompt is about -------------------------------------------------------------------------

/// Records that `path` (a deliverable that exists) is what this prompt is about, as its live path: a prompt isn't tied to
/// one version, so asking for a version's path links the file.
pub fn link_deliverable(conn: &Connection, project: &str, thread: &str, id: &str, path: &str) -> Result<PromptView> {
    get(conn, project, thread, id)?;
    let path = deliverables::split_version(path).0;
    if !deliverables::exists(conn, project, &path)? {
        return Err(Error::not_found("Deliverable not found"));
    }
    conn.execute(
        "INSERT OR IGNORE INTO prompt_deliverables (prompt_id, position, path)
         VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM prompt_deliverables WHERE prompt_id = ?1), ?2)",
        [id, path.as_str()],
    )?;
    get(conn, project, thread, id)
}

pub fn unlink_deliverable(conn: &Connection, project: &str, thread: &str, id: &str, path: &str) -> Result<PromptView> {
    conn.execute("DELETE FROM prompt_deliverables WHERE prompt_id = ?1 AND path = ?2", [id, path])?;
    get(conn, project, thread, id)
}

fn link_feedback(conn: &Connection, id: &str, feedback_ids: &[String]) -> Result<()> {
    for feedback_id in feedback_ids {
        conn.execute(
            "INSERT OR IGNORE INTO prompt_feedback (prompt_id, feedback_id, position)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position) + 1, 0) FROM prompt_feedback WHERE prompt_id = ?1))",
            [id, feedback_id.as_str()],
        )?;
    }
    Ok(())
}

/// Adds pieces of feedback to a prompt that is written: they are linked, and put in its base text with the version each was
/// given on. Those that are in it already are left as they are.
pub fn attach_feedback(conn: &Connection, project: &str, thread: &str, id: &str, feedback_ids: &[String]) -> Result<PromptView> {
    let prompt = get(conn, project, thread, id)?;
    let mut fresh: Vec<String> = vec![];
    for feedback_id in feedback_ids {
        feedback::require(conn, project, feedback_id)?;
        if !prompt.feedback.contains(feedback_id) && !fresh.contains(feedback_id) {
            fresh.push(feedback_id.clone());
        }
    }
    if !fresh.is_empty() {
        let empty = prompt.base.trim().is_empty();
        let text = feedback_text(conn, project, &fresh, empty)?;
        let base = if empty { text } else { format!("{}\n\n{text}", prompt.base.trim_end()) };
        conn.execute("UPDATE prompts SET base = ?2, updated_at = ?3 WHERE id = ?1", params![id, base, ids::now()])?;
        link_feedback(conn, id, &fresh)?;
    }
    get(conn, project, thread, id)
}

pub fn detach_feedback(conn: &Connection, project: &str, thread: &str, id: &str, feedback_id: &str) -> Result<PromptView> {
    conn.execute("DELETE FROM prompt_feedback WHERE prompt_id = ?1 AND feedback_id = ?2", [id, feedback_id])?;
    get(conn, project, thread, id)
}

/// The prompts about a deliverable (given by its live path), oldest first, with everything the screens show of one.
pub fn linked_to(conn: &Connection, project: &str, path: &str) -> Result<Vec<PromptView>> {
    let base = deliverables::split_version(path).0;
    let rows = query_all(
        conn,
        "SELECT p.thread_slug, p.id FROM prompt_deliverables d JOIN prompts p ON p.id = d.prompt_id
         WHERE p.project_slug = ?1 AND d.path = ?2 ORDER BY p.created_at, p.rowid",
        [project, base.as_str()],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )?;
    rows.into_iter().map(|(thread, id)| get(conn, project, &thread, &id)).collect()
}

/// The prompts about a deliverable, by their live path, oldest first.
pub fn producing(conn: &Connection, project: &str, path: &str) -> Result<Vec<DeliverablePrompt>> {
    let linked = query_all(
        conn,
        "SELECT p.thread_slug, t.title, p.id, p.title
         FROM prompt_deliverables d
         JOIN prompts p ON p.id = d.prompt_id
         JOIN threads t ON t.project_slug = p.project_slug AND t.slug = p.thread_slug
         WHERE p.project_slug = ?1 AND d.path = ?2
         ORDER BY p.created_at, p.rowid",
        [project, path],
        |r| Ok(DeliverablePrompt { thread_slug: r.get(0)?, thread_title: r.get(1)?, id: r.get(2)?, title: r.get(3)? }),
    )?;
    let mut seen = HashSet::new();
    Ok(linked.into_iter().filter(|p| seen.insert(p.id.clone())).collect())
}

// ---- names ------------------------------------------------------------------------------------------

/// What a prompt is called on screen: its name, or the start of what it asks when it has none yet.
pub fn display_title(title: &str, body: &str) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    let line = body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Untitled prompt");
    let mut out: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        out = format!("{}…", out.trim_end());
    }
    out
}

/// [`display_title`] of the prompt with this id, or `None` when it is gone.
pub fn display_title_of(conn: &Connection, project: &str, id: &str) -> Result<Option<String>> {
    Ok(locate(conn, project, id)?.map(|(_, title)| title))
}

/// The thread and the name of the prompt with this id, or `None` when it is gone.
pub fn locate(conn: &Connection, project: &str, id: &str) -> Result<Option<(String, String)>> {
    query_opt(conn, "SELECT thread_slug, title, body FROM prompts WHERE project_slug = ?1 AND id = ?2", [project, id], |r| {
        Ok((r.get::<_, String>(0)?, display_title(&r.get::<_, String>(1)?, &r.get::<_, String>(2)?)))
    })
}
