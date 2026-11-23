//! What a `[[link]]` written in the vault's markdown can point at: every project's threads, deliverables and feedback,
//! the days of the meeting notes (`[[2026-11-22]]`, the day's note in meetings/, as Obsidian links a daily note) and the
//! vault's people (`[[people/frank]]`, their file in shared/people). Links are kept as typed, in the files; this is the
//! list the editor completes them from and the page looks them up in (`src/lib/links.ts` holds the rules for matching
//! a typed target to one of these), and what keeps them working when a thread or deliverable is renamed
//! ([`follow_rename`], which reads them by the same rules).
//!
//! A link is read from the project it is written in: a name there needs nothing more, and another project's things
//! start with that project's slug (`[[foo-bar/pricing-brief]]`).

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::Path;

use rusqlite::{Connection, params};

use crate::db::query_all;
use crate::deliverables;
use crate::error::Result;
use crate::models::LinkTarget;

/// The longest a piece of feedback is called, in characters.
const MAX_TITLE: usize = 60;

/// What a link to a person starts with.
pub const PEOPLE: &str = "people/";

/// Everything in the vault a link can point at: each project's threads (newest first), deliverables (by name) and
/// feedback (newest first), then the days of the meeting notes (newest first), then the people (by name).
pub fn list(conn: &Connection) -> Result<Vec<LinkTarget>> {
    let mut found = Vec::new();
    for project in crate::projects::list(conn)? {
        found.extend(of_project(conn, &project.slug)?);
    }
    for note in crate::daily::list(conn, None, None)? {
        let titles: Vec<&str> = note.meetings.iter().map(|m| m.title.as_str()).collect();
        found.push(LinkTarget { kind: "meeting".into(), project: String::new(), id: note.date.clone(), title: titles.join(" | "), detail: note.date, open: String::new() });
    }
    for person in crate::people::list(conn)? {
        found.push(LinkTarget { kind: "person".into(), project: String::new(), id: person.id, title: person.name, detail: person.organisation, open: String::new() });
    }
    Ok(found)
}

/// Everything [`list`] gives, and then every file in each project's `attachments/` folder (`[[brief.pdf]]`), what the
/// editor completes from and the page looks links up in. They come last, so they never take a name from a thread, a
/// deliverable or a piece of feedback. Renaming leaves them out: nothing renames an attachment.
pub fn list_with_attachments(conn: &Connection, root: &Path) -> Result<Vec<LinkTarget>> {
    let mut found = list(conn)?;
    for project in crate::projects::list(conn)? {
        for (file, _, _) in crate::uploads::folder_files(root, &project.slug) {
            let (folder, name) = file.rsplit_once('/').map(|(d, n)| (d.to_string(), n.to_string())).unwrap_or_default();
            found.push(LinkTarget { kind: "attachment".into(), project: project.slug.clone(), id: file.clone(), title: name, detail: folder, open: file });
        }
    }
    Ok(found)
}

/// A project's threads, deliverables and feedback.
fn of_project(conn: &Connection, project: &str) -> Result<Vec<LinkTarget>> {
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

// ---- reading a link, as links.ts does --------------------------------------------------------------

const PROJECT_KINDS: [&str; 3] = ["thread", "deliverable", "feedback"];

/// How targets are compared: no difference made for case or for runs of white space.
fn fold(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn is_markdown(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

fn without_extension(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((stem, ext)) if is_markdown(path) && !ext.contains('/') => stem,
        _ => path,
    }
}

/// Whether text can stand between the brackets of a link as it is.
fn usable(text: &str) -> bool {
    !text.trim().is_empty() && !text.contains(['[', ']', '|', '\\', '\n'])
}

fn same(a: &LinkTarget, b: &LinkTarget) -> bool {
    a.kind == b.kind && a.project == b.project && a.id == b.id
}

/// The names of one project's things: the first target to claim a name keeps it.
#[derive(Default)]
struct Names<'a> {
    all: HashMap<String, &'a LinkTarget>,
    by_kind: HashMap<&'static str, HashMap<String, &'a LinkTarget>>,
}

impl<'a> Names<'a> {
    fn add(&mut self, kind: &'static str, t: &'a LinkTarget) {
        let names = match kind {
            "thread" => vec![fold(&t.title), fold(&t.id)],
            "deliverable" => {
                let file = deliverables::file_name(&t.id);
                vec![fold(&t.id), fold(without_extension(&t.id)), fold(file), fold(without_extension(file))]
            }
            _ => vec![fold(&t.id)],
        };
        for name in names {
            self.all.entry(name.clone()).or_insert(t);
            self.by_kind.entry(kind).or_default().entry(name).or_insert(t);
        }
    }

    /// `feedback:` and the other kinds restrict a name to that kind.
    fn find(&self, text: &str) -> Option<&'a LinkTarget> {
        let named = PROJECT_KINDS.iter().find_map(|kind| {
            let rest = text.strip_prefix(kind)?.strip_prefix(':')?;
            self.by_kind.get(kind).and_then(|names| names.get(&fold(rest))).copied()
        });
        // A title may itself start like a kind ("Feedback: what next"): if nothing of that kind is meant, the whole text is the name.
        named.or_else(|| self.all.get(text).copied())
    }
}

/// What the written links mean: the names of every project's things, the meeting days and the people.
struct Index<'a> {
    projects: HashMap<&'a str, Names<'a>>,
    days: HashSet<String>,
    people: HashSet<String>,
}

impl<'a> Index<'a> {
    fn new(targets: &'a [LinkTarget]) -> Index<'a> {
        let mut index = Index { projects: HashMap::new(), days: HashSet::new(), people: HashSet::new() };
        for t in targets {
            match PROJECT_KINDS.iter().copied().find(|k| *k == t.kind) {
                Some(kind) => index.projects.entry(t.project.as_str()).or_default().add(kind, t),
                None if t.kind == "meeting" => {
                    index.days.insert(fold(&t.id));
                }
                None => {
                    index.people.insert(fold(&t.id));
                }
            }
        }
        index
    }

    /// The thread, deliverable or piece of feedback a link written in `home` means; `None` for anything else (a person,
    /// a meeting day, nothing).
    fn resolve(&self, typed: &str, home: Option<&str>) -> Option<&'a LinkTarget> {
        self.lookup(typed, home).or_else(|| {
            // `[[Page#Heading]]`: the part after # is for a place within the page.
            let hash = typed.find('#').filter(|h| *h > 0)?;
            self.lookup(&typed[..hash], home)
        })
    }

    fn lookup(&self, typed: &str, home: Option<&str>) -> Option<&'a LinkTarget> {
        let text = fold(typed);
        if text.starts_with(PEOPLE) || text.starts_with("person:") {
            return None;
        }
        if let Some(own) = home.and_then(|h| self.projects.get(h)).and_then(|names| names.find(&text)) {
            return Some(own);
        }
        if self.days.contains(text.strip_prefix("meeting:").map(fold).as_deref().unwrap_or(text.as_str())) {
            return None;
        }
        // Another project's: its slug, a slash, and the name as that project writes it.
        let (project, rest) = text.split_once('/')?;
        self.projects.get(project)?.find(rest)
    }

    /// What to write for a target in a link written in `home`: the plainest form that comes back to it, else with its
    /// kind spelled out; another project's after its slug.
    fn name_for(&self, t: &LinkTarget, home: Option<&str>) -> String {
        let file = deliverables::file_name(&t.id);
        let mut candidates: Vec<String> = match t.kind.as_str() {
            "thread" => vec![t.title.clone(), t.id.clone()],
            "deliverable" => vec![without_extension(file).to_string(), file.to_string(), without_extension(&t.id).to_string(), t.id.clone()],
            _ => vec![],
        };
        let spelled = format!("{}:{}", t.kind, t.id);
        candidates.push(spelled.clone());
        let prefix = if home == Some(t.project.as_str()) { String::new() } else { format!("{}/", t.project) };
        for name in candidates.iter().filter(|n| usable(n)) {
            let written = format!("{prefix}{}", name.trim());
            if self.resolve(&written, home).is_some_and(|r| same(r, t)) {
                return written;
            }
        }
        format!("{prefix}{spelled}")
    }
}

/// Where the target of each `[[link]]` in `text` is, outside code (fenced blocks and inline code), as links.ts finds
/// them: `[[target]]`, `[[target|shown]]` and, in a table cell, `[[target\|shown]]`.
fn link_targets(text: &str) -> Vec<Range<usize>> {
    let mut found = vec![];
    let mut fence: Option<(u8, usize)> = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let bytes = line.as_bytes();
        let indent = bytes.iter().take_while(|b| **b == b' ' || **b == b'\t').count();
        if let Some(&mark) = bytes.get(indent).filter(|b| **b == b'`' || **b == b'~') {
            let run = bytes[indent..].iter().take_while(|b| **b == mark).count();
            if run >= 3 {
                match fence {
                    None => {
                        fence = Some((mark, run));
                        continue;
                    }
                    Some((open, length)) if open == mark && run >= length => {
                        fence = None;
                        continue;
                    }
                    Some(_) => {}
                }
            }
        }
        if fence.is_some() {
            continue;
        }
        let mut code: Option<usize> = None;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'`' {
                let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
                code = match code {
                    None => Some(run),
                    Some(open) if open == run => None,
                    other => other,
                };
                i += run;
                continue;
            }
            if code.is_none()
                && bytes[i..].starts_with(b"[[")
                && let Some((target, end)) = one_link(bytes, i)
            {
                found.push(start + target.start..start + target.end);
                i = end;
                continue;
            }
            i += 1;
        }
    }
    found
}

/// The link starting at `at` (its `[[`): where its target is, and where the link ends.
fn one_link(bytes: &[u8], at: usize) -> Option<(Range<usize>, usize)> {
    let from = at + 2;
    let mut i = from;
    while i < bytes.len() && !matches!(bytes[i], b'[' | b']' | b'|' | b'\n' | b'\\') {
        i += 1;
    }
    if i == from {
        return None;
    }
    let target = from..i;
    if bytes.get(i) == Some(&b'\\') {
        i += 1;
        if bytes.get(i) != Some(&b'|') {
            return None;
        }
    }
    if bytes.get(i) == Some(&b'|') {
        i += 1;
        while i < bytes.len() && !matches!(bytes[i], b'[' | b']' | b'\n') {
            i += 1;
        }
    }
    bytes[i..].starts_with(b"]]").then_some((target, i + 2))
}

/// `text` (written in `home`) with each link that meant `old` (by `was`) written so it means `new` (by `is`), its
/// words and any `#place` kept; `None` when no link changes.
fn retarget(text: &str, home: Option<&str>, was: &Index, is: &Index, old: &LinkTarget, new: &LinkTarget) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for range in link_targets(text) {
        let typed = text[range.clone()].trim();
        if !was.resolve(typed, home).is_some_and(|t| same(t, old)) || is.resolve(typed, home).is_some_and(|t| same(t, new)) {
            continue;
        }
        let place = if was.lookup(typed, home).is_some_and(|t| same(t, old)) { "" } else { typed.find('#').map_or("", |h| &typed[h..]) };
        out.push_str(&text[last..range.start]);
        out.push_str(&is.name_for(new, home));
        out.push_str(place);
        last = range.end;
    }
    (last > 0).then(|| {
        out.push_str(&text[last..]);
        out
    })
}

/// The markdown that links are written in, by table and column, and where its project is ("NULL" for the vault's own:
/// people, clients and semesters, and the text of a meeting note before its meetings).
const TEXTS: [(&str, &str, &str); 15] = [
    ("projects", "description", "slug"),
    ("projects", "readme", "slug"),
    ("threads", "description", "project_slug"),
    ("posts", "body", "project_slug"),
    ("prompts", "body", "project_slug"),
    ("feedback", "body", "project_slug"),
    ("uploads", "description", "project_slug"),
    ("deliverable_descriptions", "description", "project_slug"),
    ("user_stories", "want", "project_slug"),
    ("user_stories", "reason", "project_slug"),
    ("user_stories", "notes", "project_slug"),
    ("tasks", "body", "project_slug"),
    ("people", "notes", "NULL"),
    ("clients", "notes", "NULL"),
    ("daily_notes", "preamble", "NULL"),
];

/// Keeps the links to a thread or deliverable working after it was renamed: every link in the vault that meant it
/// (by `before`, the targets as they were) is written so it means it now, under its new name (`to`, its id now; the
/// same as `from` for a thread, whose slug stays). Published versions are left as they were published.
pub fn follow_rename(conn: &Connection, before: &[LinkTarget], kind: &str, project: &str, from: &str, to: &str) -> Result<()> {
    let Some(old) = before.iter().find(|t| t.kind == kind && t.project == project && t.id == from) else { return Ok(()) };
    let now = list(conn)?;
    let Some(new) = now.iter().find(|t| t.kind == kind && t.project == project && t.id == to) else { return Ok(()) };
    let (was, is) = (Index::new(before), Index::new(&now));

    let mut texts: Vec<(&str, &str, i64, String, Option<String>)> = vec![];
    for (table, column, home) in TEXTS {
        let sql = format!("SELECT rowid, {column}, {home} FROM {table} WHERE {column} LIKE '%[[%'");
        for (rowid, text, home) in query_all(conn, &sql, [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)))? {
            texts.push((table, column, rowid, text, home));
        }
    }
    // The live markdown deliverables, not their versions.
    for (rowid, path, text, home) in query_all(conn, "SELECT rowid, path, body, project_slug FROM deliverables WHERE body LIKE '%[[%'", [], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
    })? {
        if deliverables::split_version(&path).1.is_none() {
            texts.push(("deliverables", "body", rowid, text, Some(home)));
        }
    }
    // A meeting is written in its project when it has only one.
    for (rowid, projects, text) in query_all(conn, "SELECT rowid, projects, body FROM daily_meetings WHERE body LIKE '%[[%'", [], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })? {
        let listed: Vec<&str> = projects.lines().filter(|p| !p.is_empty()).collect();
        let home = if let [only] = listed.as_slice() { Some(only.to_string()) } else { None };
        texts.push(("daily_meetings", "body", rowid, text, home));
    }

    for (table, column, rowid, text, home) in texts {
        if let Some(changed) = retarget(&text, home.as_deref(), &was, &is, old, new) {
            conn.execute(&format!("UPDATE {table} SET {column} = ?2 WHERE rowid = ?1"), params![rowid, changed])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(kind: &str, project: &str, id: &str, title: &str) -> LinkTarget {
        LinkTarget { kind: kind.into(), project: project.into(), id: id.into(), title: title.into(), detail: String::new(), open: String::new() }
    }

    #[test]
    fn links_are_found_outside_code_only() {
        let text = "See [[a]] and [[b|the b]].\n`[[not]]` here\n```\n[[nor]]\n```\n| [[c\\|cell]] |\n[[]] [[d";
        let found: Vec<&str> = link_targets(text).into_iter().map(|r| &text[r]).collect();
        assert_eq!(found, ["a", "b", "c"]);
    }

    #[test]
    fn a_rename_rewrites_what_meant_the_old_name_from_where_it_is_written() {
        let before = vec![target("thread", "acme", "call", "Kick-off call"), target("deliverable", "acme", "plan.md", "plan.md"), target("deliverable", "bolt", "plan.md", "plan.md")];
        let after = vec![target("thread", "acme", "call", "Kick-off call"), target("deliverable", "acme", "budget.md", "budget.md"), target("deliverable", "bolt", "plan.md", "plan.md")];
        let (was, is) = (Index::new(&before), Index::new(&after));
        let (old, new) = (&before[1], &after[1]);
        let text = "[[plan]], [[Plan.md|the plan]], [[plan#Goal]], [[bolt/plan]], [[Kick-off call]], `[[plan]]`";
        assert_eq!(retarget(text, Some("acme"), &was, &is, old, new).unwrap(), "[[budget]], [[budget|the plan]], [[budget#Goal]], [[bolt/plan]], [[Kick-off call]], `[[plan]]`");
        // From another project, the link names acme; there `[[plan]]` is bolt's own.
        assert_eq!(retarget("[[acme/plan]] and [[plan]]", Some("bolt"), &was, &is, old, new).unwrap(), "[[acme/budget]] and [[plan]]");
        assert_eq!(retarget("[[plan]]", None, &was, &is, old, new), None, "outside a project, a plain name is nobody's");
    }
}
