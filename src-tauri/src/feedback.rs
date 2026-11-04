//! Feedback is written in a thread and lands on a deliverable (or one of its published versions).

use rusqlite::{Connection, Row, params};

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::models::{FeedbackFrom, FeedbackView};
use crate::{deliverables, ids, markdown, threads};

const COLUMNS: &str = "id, thread_slug, deliverable_path, deliverable_version, status, created_at, updated_at, body,
    from_name, from_user, from_role, given, portflow_comment, reply_to, comment_type, evidence, evidence_version";

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
        from: r.get(8)?,
        from_user: r.get(9)?,
        from_role: r.get(10)?,
        given: r.get(11)?,
        portflow_comment: r.get(12)?,
        reply_to: r.get(13)?,
        comment_type: r.get(14)?,
        evidence: r.get(15)?,
        evidence_version: r.get(16)?,
    })
}

/// Oldest first.
pub fn list_for_thread(conn: &Connection, project: &str, thread: &str) -> Result<Vec<FeedbackView>> {
    query_all(
        conn,
        &format!("SELECT {COLUMNS} FROM feedback WHERE project_slug = ?1 AND thread_slug = ?2 ORDER BY created_at, rowid"),
        [project, thread],
        from_row,
    )
}

/// All feedback on a deliverable and on its published versions, from every thread, oldest first.
pub fn list_for_deliverable(conn: &Connection, project: &str, base_path: &str) -> Result<Vec<FeedbackView>> {
    let all = query_all(
        conn,
        &format!("SELECT {COLUMNS} FROM feedback WHERE project_slug = ?1 ORDER BY created_at, rowid"),
        [project],
        from_row,
    )?;
    Ok(all.into_iter().filter(|f| deliverables::split_version(&f.deliverable_path).0 == base_path).collect())
}

fn get(conn: &Connection, project: &str, thread: &str, id: &str) -> Result<FeedbackView> {
    query_opt(
        conn,
        &format!("SELECT {COLUMNS} FROM feedback WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3"),
        [project, thread, id],
        from_row,
    )?
    .ok_or_else(|| Error::not_found("Feedback not found"))
}

/// Feedback on `deliverable_path`, which must exist (live, or a published "<name>@v1.0.<ext>").
#[cfg_attr(not(test), allow(dead_code))]
pub fn add(conn: &Connection, project: &str, thread: &str, deliverable_path: &str, body: &str) -> Result<FeedbackView> {
    add_from(conn, project, thread, deliverable_path, body, &FeedbackFrom::default())
}

pub const GIVEN: [&str; 4] = ["in person", "portflow", "email", "other"];

/// Feedback, saying who gave it and how.
pub fn add_from(conn: &Connection, project: &str, thread: &str, deliverable_path: &str, body: &str, from: &FeedbackFrom) -> Result<FeedbackView> {
    threads::require(conn, project, thread)?;
    let body = body.trim();
    if body.is_empty() {
        return Err(Error::invalid("Write the feedback first"));
    }
    if !deliverables::exists(conn, project, deliverable_path)? {
        return Err(Error::not_found("Deliverable not found"));
    }
    let version = deliverables::split_version(deliverable_path).1;

    let ids::Stamp { id, at } = ids::new_stamp();
    conn.execute(
        "INSERT INTO feedback (id, project_slug, thread_slug, deliverable_path, deliverable_version, created_at, updated_at, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
        params![id, project, thread, deliverable_path, version, at, body],
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
