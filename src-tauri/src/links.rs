//! What a `[[link]]` written in a project's markdown can point at: the project's threads, deliverables and feedback.
//! Links are kept as typed, in the files; this is only the list the editor completes them from and the page looks
//! them up in (`src/lib/links.ts` holds the rules for matching a typed target to one of these).

use rusqlite::Connection;

use crate::db::query_all;
use crate::error::Result;
use crate::models::LinkTarget;
use crate::deliverables;

/// The longest a piece of feedback is called, in characters.
const MAX_TITLE: usize = 60;

/// Threads first, newest first, then the deliverables by name, then the feedback, newest first.
pub fn list(conn: &Connection, project: &str) -> Result<Vec<LinkTarget>> {
    let mut found = Vec::new();

    for (slug, title) in query_all::<(String, String), _>(
        conn,
        "SELECT slug, title FROM threads WHERE project_slug = ?1 ORDER BY created_at DESC, rowid DESC",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )? {
        found.push(LinkTarget { kind: "thread".into(), id: slug, title, detail: String::new(), open: String::new() });
    }

    for d in deliverables::list(conn, project)? {
        // A deliverable whose live file is gone opens on its newest version.
        let open = match d.versions.last() {
            Some(version) if !d.exists => deliverables::snapshot_path(&d.path, version),
            _ => d.path.clone(),
        };
        let folder = d.path.rsplit_once('/').map(|(dir, _)| dir.to_string()).unwrap_or_default();
        found.push(LinkTarget { kind: "deliverable".into(), id: d.path, title: d.name, detail: folder, open });
    }

    for (id, path, status, body) in query_all::<(String, String, String, String), _>(
        conn,
        "SELECT id, deliverable_path, status, body FROM feedback WHERE project_slug = ?1 ORDER BY created_at DESC, rowid DESC",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )? {
        let on = deliverables::file_name(&deliverables::split_version(&path).0).to_string();
        let detail = if status == "resolved" { format!("on {on}, resolved") } else { format!("on {on}") };
        found.push(LinkTarget { kind: "feedback".into(), id, title: excerpt(&body), detail, open: path });
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
    let cut: String = line.chars().take(MAX_TITLE).collect();
    format!("{}…", cut.trim_end())
}
