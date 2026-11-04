//! Feedback is written in a thread and lands on a deliverable (or one of its published versions): on the whole
//! document, or on a passage of it.

use rusqlite::{Connection, Row, params};

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::models::{FeedbackFrom, FeedbackView, Quote};
use std::collections::HashMap;

use crate::{deliverables, ids, markdown, provenance, threads};

const COLUMNS: &str = "id, thread_slug, deliverable_path, deliverable_version, status, created_at, updated_at, body, quote, quote_prefix, quote_suffix,
    from_name, from_user, from_role, given, portflow_comment, reply_to, comment_type, evidence, evidence_version";

/// The longest passage that can be commented on, in characters: a paragraph or two, not a whole page.
pub const MAX_QUOTE: usize = 2000;
/// How much of the words before and after a passage are kept: enough to tell two occurrences apart.
const MAX_CONTEXT: usize = 120;

fn from_row(r: &Row<'_>) -> rusqlite::Result<FeedbackView> {
    let body: String = r.get(7)?;
    Ok(FeedbackView {
        id: r.get(0)?,
        thread_slug: r.get(1)?,
        deliverable_path: r.get(2)?,
        deliverable_version: r.get(3)?,
        status: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
        html: markdown::render(&body),
        body,
        quote: r.get(8)?,
        quote_prefix: r.get(9)?,
        quote_suffix: r.get(10)?,
        given_on: None,
        from: r.get(11)?,
        from_user: r.get(12)?,
        from_role: r.get(13)?,
        given: r.get(14)?,
        portflow_comment: r.get(15)?,
        reply_to: r.get(16)?,
        comment_type: r.get(17)?,
        evidence: r.get(18)?,
        evidence_version: r.get(19)?,
    })
}

/// Says which version each piece of feedback was given on. Feedback on the live file was given on what the next version
/// froze, so once one is published the feedback is on that version (publish times and writing times are the same kind of
/// stamp, so they compare as text).
fn fill_given_on(conn: &Connection, project: &str, items: &mut [FeedbackView]) -> Result<()> {
    let mut published: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for f in items.iter_mut() {
        if let Some(version) = &f.deliverable_version {
            f.given_on = Some(version.clone());
            continue;
        }
        if !published.contains_key(&f.deliverable_path) {
            let mut versions: Vec<(String, String)> =
                provenance::resolve(conn, project, &f.deliverable_path)?.into_iter().map(|v| (v.at, v.version)).collect();
            versions.sort();
            published.insert(f.deliverable_path.clone(), versions);
        }
        f.given_on = published[&f.deliverable_path].iter().find(|(at, _)| at.as_str() >= f.created_at.as_str()).map(|(_, version)| version.clone());
    }
    Ok(())
}

/// The feedback with this id in the project.
pub fn find(conn: &Connection, project: &str, id: &str) -> Result<FeedbackView> {
    let mut item = query_opt(conn, &format!("SELECT {COLUMNS} FROM feedback WHERE project_slug = ?1 AND id = ?2"), [project, id], from_row)?
        .ok_or_else(|| Error::not_found("Feedback not found"))?;
    fill_given_on(conn, project, std::slice::from_mut(&mut item))?;
    Ok(item)
}

/// A piece of feedback as a prompt says it: which version it was given on (and where that version is, so it can be read),
/// the thread it was written in, the passage it is about, and what was said.
pub fn block(conn: &Connection, project: &str, item: &FeedbackView) -> Result<String> {
    let live = deliverables::split_version(&item.deliverable_path).0;
    let on = match &item.given_on {
        Some(version) => format!("v{version} (versions/{})", deliverables::snapshot_path(&live, version)),
        None => format!("the live file (deliverables/{live}), as it stands"),
    };
    let thread = query_opt(conn, "SELECT title FROM threads WHERE project_slug = ?1 AND slug = ?2", [project, item.thread_slug.as_str()], |r| r.get::<_, String>(0))?;
    let from = thread.map(|t| format!(", from the thread “{t}”")).unwrap_or_default();
    let passage = item.quote.as_ref().map(|q| format!("\n\n> {q}")).unwrap_or_default();
    Ok(format!("### On {on}{from}{passage}\n\n{}", item.body.trim()))
}

/// Feedback with this id must be in the project.
pub fn require(conn: &Connection, project: &str, id: &str) -> Result<()> {
    match query_opt(conn, "SELECT 1 FROM feedback WHERE project_slug = ?1 AND id = ?2", [project, id], |r| r.get::<_, i64>(0))? {
        Some(_) => Ok(()),
        None => Err(Error::not_found("Feedback not found")),
    }
}

/// Oldest first.
pub fn list_for_thread(conn: &Connection, project: &str, thread: &str) -> Result<Vec<FeedbackView>> {
    let mut items = query_all(
        conn,
        &format!("SELECT {COLUMNS} FROM feedback WHERE project_slug = ?1 AND thread_slug = ?2 ORDER BY created_at, rowid"),
        [project, thread],
        from_row,
    )?;
    fill_given_on(conn, project, &mut items)?;
    Ok(items)
}

/// All feedback on a deliverable and on its published versions, from every thread, oldest first.
pub fn list_for_deliverable(conn: &Connection, project: &str, base_path: &str) -> Result<Vec<FeedbackView>> {
    let all = query_all(
        conn,
        &format!("SELECT {COLUMNS} FROM feedback WHERE project_slug = ?1 ORDER BY created_at, rowid"),
        [project],
        from_row,
    )?;
    let mut items: Vec<FeedbackView> = all.into_iter().filter(|f| deliverables::split_version(&f.deliverable_path).0 == base_path).collect();
    fill_given_on(conn, project, &mut items)?;
    Ok(items)
}

fn get(conn: &Connection, project: &str, thread: &str, id: &str) -> Result<FeedbackView> {
    let mut item = query_opt(
        conn,
        &format!("SELECT {COLUMNS} FROM feedback WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3"),
        [project, thread, id],
        from_row,
    )?
    .ok_or_else(|| Error::not_found("Feedback not found"))?;
    fill_given_on(conn, project, std::slice::from_mut(&mut item))?;
    Ok(item)
}

/// The passage as it is kept: the text trimmed, the context cut to what is of use; `None` for no passage at all.
fn clean(quote: Option<&Quote>) -> Result<Option<Quote>> {
    let Some(quote) = quote else { return Ok(None) };
    let text = quote.text.trim();
    if text.is_empty() {
        return Err(Error::invalid("Select the text to comment on"));
    }
    if text.chars().count() > MAX_QUOTE {
        return Err(Error::invalid(format!("Select a shorter passage, up to {MAX_QUOTE} characters")));
    }
    let before = quote.prefix.chars().count().saturating_sub(MAX_CONTEXT);
    Ok(Some(Quote {
        text: text.to_string(),
        prefix: quote.prefix.chars().skip(before).collect(),
        suffix: quote.suffix.chars().take(MAX_CONTEXT).collect(),
    }))
}

/// Feedback on `deliverable_path`, which must exist (live, or a published "<name>@v1.0.<ext>"); on the passage `quote`,
/// or on the whole document without one.
pub fn add(conn: &Connection, project: &str, thread: &str, deliverable_path: &str, body: &str, quote: Option<&Quote>) -> Result<FeedbackView> {
    add_with(conn, project, thread, deliverable_path, body, quote, &FeedbackFrom::default())
}

pub const GIVEN: [&str; 4] = ["in person", "portflow", "email", "other"];

/// Feedback on the whole document, saying who gave it and how.
pub fn add_from(conn: &Connection, project: &str, thread: &str, deliverable_path: &str, body: &str, from: &FeedbackFrom) -> Result<FeedbackView> {
    add_with(conn, project, thread, deliverable_path, body, None, from)
}

/// Feedback on the passage `quote` (or the whole document), saying who gave it and how.
pub fn add_with(
    conn: &Connection,
    project: &str,
    thread: &str,
    deliverable_path: &str,
    body: &str,
    quote: Option<&Quote>,
    from: &FeedbackFrom,
) -> Result<FeedbackView> {
    threads::require(conn, project, thread)?;
    let body = body.trim();
    if body.is_empty() {
        return Err(Error::invalid("Write the feedback first"));
    }
    let quote = clean(quote)?;
    if !deliverables::exists(conn, project, deliverable_path)? {
        return Err(Error::not_found("Deliverable not found"));
    }
    let version = deliverables::split_version(deliverable_path).1;

    let ids::Stamp { id, at } = ids::new_stamp();
    conn.execute(
        "INSERT INTO feedback (id, project_slug, thread_slug, deliverable_path, deliverable_version, created_at, updated_at, body,
                               quote, quote_prefix, quote_suffix)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            project,
            thread,
            deliverable_path,
            version,
            at,
            body,
            quote.as_ref().map(|q| q.text.as_str()),
            quote.as_ref().map(|q| q.prefix.as_str()).filter(|s| !s.is_empty()),
            quote.as_ref().map(|q| q.suffix.as_str()).filter(|s| !s.is_empty())
        ],
    )?;
    set_from(conn, &id, from)?;
    get(conn, project, thread, &id)
}

/// Says who gave the feedback (an empty name clears it). Feedback pulled from Portflow keeps its author.
pub fn set_from(conn: &Connection, id: &str, from: &FeedbackFrom) -> Result<()> {
    let name = from.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let given = from.given.as_deref().map(str::trim).filter(|g| !g.is_empty());
    if let Some(given) = given
        && !GIVEN.contains(&given)
    {
        return Err(Error::invalid("Feedback is given in person, through Portflow, by email, or otherwise"));
    }
    let role = from.role.as_deref().map(str::trim).filter(|r| !r.is_empty());
    conn.execute(
        "UPDATE feedback SET from_name = ?2, from_user = ?3, from_role = ?4, given = ?5 WHERE id = ?1 AND portflow_comment IS NULL",
        params![id, name, name.and(from.portflow_user), name.and(role), given],
    )?;
    Ok(())
}

/// Changes who gave a piece of feedback.
pub fn update_from(conn: &Connection, project: &str, thread: &str, id: &str, from: &FeedbackFrom) -> Result<FeedbackView> {
    let current = get(conn, project, thread, id)?;
    if current.portflow_comment.is_some() {
        return Err(Error::invalid("This feedback comes from Portflow, which says who gave it"));
    }
    set_from(conn, id, from)?;
    conn.execute("UPDATE feedback SET updated_at = ?2 WHERE id = ?1", params![id, ids::now()])?;
    get(conn, project, thread, id)
}

/// Opens or resolves one piece of feedback (a reply is a piece of its own).
pub fn set_status(conn: &Connection, project: &str, thread: &str, id: &str, status: &str) -> Result<FeedbackView> {
    if status != "open" && status != "resolved" {
        return Err(Error::invalid("Feedback is either open or resolved"));
    }
    let changed = conn.execute(
        "UPDATE feedback SET status = ?4, updated_at = ?5 WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3",
        params![project, thread, id, status, ids::now()],
    )?;
    if changed == 0 {
        return Err(Error::not_found("Feedback not found"));
    }
    get(conn, project, thread, id)
}

/// Opens or resolves a piece of feedback together with every reply to it (an expert's answers in Portflow): "Resolve
/// all" on the card. Each piece can still be changed on its own with [`set_status`].
pub fn set_status_all(conn: &Connection, project: &str, thread: &str, id: &str, status: &str) -> Result<FeedbackView> {
    set_status(conn, project, thread, id, status)?;
    conn.execute(
        "UPDATE feedback SET status = ?3, updated_at = ?4 WHERE project_slug = ?1 AND reply_to = ?2 AND status <> ?3",
        params![project, id, status, ids::now()],
    )?;
    get(conn, project, thread, id)
}

pub fn update(conn: &Connection, project: &str, thread: &str, id: &str, body: &str) -> Result<FeedbackView> {
    if get(conn, project, thread, id)?.portflow_comment.is_some() {
        return Err(Error::invalid("This feedback comes from Portflow: it can't be edited in Ledger"));
    }
    let body = body.trim();
    if body.is_empty() {
        return Err(Error::invalid("Write the feedback first"));
    }
    let changed = conn.execute(
        "UPDATE feedback SET body = ?4, updated_at = ?5 WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3",
        params![project, thread, id, body, ids::now()],
    )?;
    if changed == 0 {
        return Err(Error::not_found("Feedback not found"));
    }
    get(conn, project, thread, id)
}

pub fn delete(conn: &Connection, project: &str, thread: &str, id: &str) -> Result<()> {
    conn.execute("DELETE FROM feedback WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3", [project, thread, id])?;
    Ok(())
}
