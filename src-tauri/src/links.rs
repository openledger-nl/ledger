//! What a `[[link]]` written in a project's markdown can point at: the project's threads, deliverables and feedback, and
//! the days its meetings were noted on (`[[2026-11-22]]`, the day's note in meetings/, as Obsidian links a daily note).
//! Links are kept as typed, in the files; this is only the list the editor completes them from and the page looks them
//! up in (`src/lib/links.ts` holds the rules for matching a typed target to one of these). A meeting's notes link to
//! the targets of the meeting's projects ([`list_vault`]).

use rusqlite::Connection;

use crate::db::query_all;
use crate::error::Result;
use crate::models::LinkTarget;
use crate::deliverables;

/// The longest a piece of feedback is called, in characters.
const MAX_TITLE: usize = 60;

/// Threads first, newest first, then the deliverables by name, then the feedback, newest first, then the days of the
/// project's meetings, newest first.
pub fn list(conn: &Connection, project: &str) -> Result<Vec<LinkTarget>> {
    let mut found = Vec::new();
    let target = |kind: &str, id: String, title: String, detail: String, open: String| LinkTarget { kind: kind.into(), project: project.to_string(), id, title, detail, open };

    for (slug, title) in query_all::<(String, String), _>(
        conn,
        "SELECT slug, title FROM threads WHERE project_slug = ?1 ORDER BY created_at DESC, rowid DESC",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )? {
        found.push(target("thread", slug, title, String::new(), String::new()));
    }

    for d in deliverables::list(conn, project)? {
        // A deliverable whose live file is gone opens on its newest version.
        let open = match d.versions.last() {
            Some(version) if !d.exists => deliverables::snapshot_path(&d.path, version),
            _ => d.path.clone(),
        };
        let folder = d.path.rsplit_once('/').map(|(dir, _)| dir.to_string()).unwrap_or_default();
        found.push(target("deliverable", d.path, d.name, folder, open));
    }

    for (id, path, status, body) in query_all::<(String, String, String, String), _>(
        conn,
        "SELECT id, deliverable_path, status, body FROM feedback WHERE project_slug = ?1 ORDER BY created_at DESC, rowid DESC",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )? {
        let on = deliverables::file_name(&deliverables::split_version(&path).0).to_string();
        let detail = if status == "resolved" { format!("on {on}, resolved") } else { format!("on {on}") };
        found.push(target("feedback", id, excerpt(&body), detail, path));
    }

    for note in crate::daily::list(conn, Some(project), None)? {
        let titles: Vec<&str> = note.meetings.iter().filter(|m| m.projects.iter().any(|p| p == project)).map(|m| m.title.as_str()).collect();
        found.push(target("meeting", note.date.clone(), titles.join(" | "), note.date, String::new()));
    }

    Ok(found)
}

/// The targets of every project, for the meeting notes (outside any one project): each says which project it is in.
pub fn list_vault(conn: &Connection) -> Result<Vec<LinkTarget>> {
    let mut found = Vec::new();
    for project in crate::projects::list(conn)? {
        found.extend(list(conn, &project.slug)?);
    }
    Ok(found)
}

/// What a piece of feedback is called in a link: the start of what it says, on one line (its first line with any
/// heading, quote or list mark taken off the front).
fn excerpt(body: &str) -> String {
    let first = body.lines().map(|l| l.trim_start_matches(['#', '>', '-', '*', ' ', '\t']).trim()).find(|l| !l.is_empty()).unwrap_or("");
    let line = first.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= MAX_TITLE {
        return line;
    }
    // Three characters fewer, so the cut with its three dots stays within MAX_TITLE.
    let cut: String = line.chars().take(MAX_TITLE.saturating_sub(3)).collect();
    format!("{}...", cut.trim_end())
}
