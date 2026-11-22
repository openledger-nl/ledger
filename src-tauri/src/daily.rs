//! Meeting notes, kept as daily notes: one file a day, `meetings/<YYYY>/<MM>/<YYYY-MM-DD>.md` at the root of the vault,
//! written the way Obsidian writes daily notes (date format `YYYY/MM/YYYY-MM-DD`), so the same note opens in both.
//!
//! ```text
//! ---
//! date: 2026-11-09
//! tags: [project/thrym-player]                    the meetings' projects, plus any plain tags of your own
//! meetings:                                       each meeting's time and the projects it belongs to, by its title
//! - title: Coach meeting
//!   time: '10:00'
//!   projects: [thrym-player]
//! ---
//!
//! ## Coach meeting                                a meeting: its title
//! With: [[people/frank]], [[people/sam]]          who was there: links to their files in shared/people
//!
//! What was said, decided, to do… [[pricing-brief]] links to the project's deliverables and threads.
//! ```
//!
//! Reading is lenient: a note Obsidian made, without frontmatter, reads in; a heading the frontmatter doesn't list, or
//! without a `With:` line, is a meeting all the same (with no time and no project). Of two meetings with the same title
//! the first listed goes with the first heading. Whatever comes before the first `## ` heading, and any frontmatter of
//! your own, is kept as it is and written back. Someone named in a `With:` line who isn't in the vault yet is added to
//! it when the note is saved, so every name there leads to a file.

use std::collections::HashSet;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

use crate::{
    db::{query_all, query_opt},
    error::{Error, Result},
    frontmatter::{self, MetaExt},
    ids,
};

/// The folder of the meeting notes, at the vault's root, in lower case like the vault's other folders.
pub const FOLDER: &str = "meetings";


#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MeetingSection {
    /// "10:00"; none when the heading has no time.
    pub time: Option<String>,
    pub title: String,
    /// The projects it belongs to, by their tag (`thrym-player` for `#project/thrym-player`).
    pub projects: Vec<String>,
    /// Who was there, by their id in shared/people (`frank`).
    pub people: Vec<String>,
    /// Your notes, as written.
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyNote {
    pub date: String,
    pub id: String,
    pub meetings: Vec<MeetingSection>,
    pub updated_at: String,
}

/// A day's note as the screen saves it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DailyDraft {
    pub date: String,
    pub meetings: Vec<MeetingSection>,
}

/// A note as it was read, before it is stored: the meetings and what else the file holds.
pub struct ParsedDaily {
    pub meetings: Vec<MeetingSection>,
    /// The text before the first meeting, kept as it is.
    pub preamble: String,
    /// Frontmatter keys Ledger doesn't write itself, and plain tags (without a `/`) of your own.
    pub extra: Mapping,
    pub own_tags: Vec<String>,
}

pub fn is_day(text: &str) -> bool {
    crate::dates::is_day(text)
}

/// Where the note of a day is kept: in a folder per year and per month, `meetings/2026/11/2026-11-20.md`, as
/// Obsidian's Daily notes plugin writes it with the date format `YYYY/MM/YYYY-MM-DD` and the folder `meetings`.
pub fn file(date: &str) -> String {
    match (date.get(..4), date.get(5..7)) {
        (Some(year), Some(month)) if is_day(date) => format!("{FOLDER}/{year}/{month}/{date}.md"),
        _ => format!("{FOLDER}/{date}.md"),
    }
}

/// The people a `With:` line links to: `[[people/frank]]` (or `[[people/frank|Frank]]`), by id.
fn people_in(line: &str) -> Vec<String> {
    let mut out = vec![];
    for (_, rest) in line.match_indices("[[").map(|(at, _)| line.split_at(at + 2)) {
        let Some(inner) = rest.split("]]").next() else { continue };
        let target = inner.split('|').next().unwrap_or(inner).trim();
        if let Some(id) = target.strip_prefix(crate::links::PEOPLE).map(str::trim).filter(|id| !id.is_empty() && !id.contains('[')) {
            out.push(id.to_lowercase());
        }
    }
    out
}

/// "9:05" → "09:05"; none for anything that isn't a time of day.
fn clock(text: &str) -> Option<String> {
    let (h, m) = text.split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60 && text.len() <= 5).then(|| format!("{h:02}:{m:02}"))
}

/// What the frontmatter says of a meeting: its title, time and projects.
type Listed = (String, Option<String>, Vec<String>);

/// Reads a daily note: every `## ` heading is a meeting, called by the heading; its time and projects are in the
/// frontmatter's `meetings`, found by that title.
pub fn parse(meta: &Mapping, body: &str) -> ParsedDaily {
    let mut preamble = vec![];
    let mut sections: Vec<(String, Vec<&str>)> = vec![];
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            sections.push((heading.split_whitespace().collect::<Vec<_>>().join(" "), vec![]));
        } else if let Some((_, lines)) = sections.last_mut() {
            lines.push(line);
        } else {
            preamble.push(line);
        }
    }

    // What the frontmatter says of each meeting, each taken by the first heading of its title.
    let mut listed: Vec<Option<Listed>> = meta
        .maps("meetings")
        .into_iter()
        .map(|m| Some((m.text("title").unwrap_or_default().trim().to_string(), m.text("time").and_then(|t| clock(t.trim())), m.list("projects"))))
        .collect();
    let mut meetings = vec![];
    for (title, lines) in sections {
        let found = listed.iter_mut().find(|m| m.as_ref().is_some_and(|(t, ..)| t.eq_ignore_ascii_case(&title))).and_then(Option::take);
        let (time, projects) = found.map(|(_, time, projects)| (time, projects)).unwrap_or_default();
        // A `With:` line counts when it is the first line with text under the heading and links to someone; any other
        // is part of the notes.
        let first = lines.iter().position(|l| !l.trim().is_empty());
        let with = first.filter(|i| lines[*i].trim().to_lowercase().starts_with("with:")).map(|i| (i, people_in(lines[i])));
        let (people, notes) = match with {
            Some((i, people)) if !people.is_empty() => (people, &lines[i + 1..]),
            _ => (vec![], &lines[..]),
        };
        meetings.push(MeetingSection { time, title, projects, people, body: notes.join("\n").trim().to_string() });
    }

    let mut extra = meta.clone();
    for key in ["date", "tags", "meetings"] {
        extra.remove(key);
    }
    let own_tags = meta.list("tags").into_iter().map(|t| t.trim_start_matches('#').to_string()).filter(|t| !t.is_empty() && !t.contains('/')).collect();
    ParsedDaily { meetings, preamble: preamble.join("\n").trim().to_string(), extra, own_tags }
}

/// The tags of the meetings' projects (`project/x`), each once. Who was there is linked, not tagged.
fn gathered(meetings: &[MeetingSection]) -> Vec<String> {
    let mut seen = HashSet::new();
    meetings.iter().flat_map(|m| m.projects.iter().map(|p| format!("project/{p}"))).filter(|t| seen.insert(t.clone())).collect()
}

/// What a meeting's heading says: its title, "Meeting" when it has none.
fn heading_of(m: &MeetingSection) -> String {
    let title = m.title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() { "Meeting".into() } else { title }
}

/// The note's text: Ledger's keys first (each meeting's time and projects among them), then any of your own, the text
/// before the meetings, then a heading each.
pub fn render(date: &str, meetings: &[MeetingSection], preamble: &str, extra: &Mapping, own_tags: &[String]) -> String {
    let mut meta = Mapping::new();
    frontmatter::set(&mut meta, "date", date);
    let mut tags = gathered(meetings);
    tags.extend(own_tags.iter().filter(|t| !tags.contains(t)).cloned().collect::<Vec<_>>());
    meta.insert(Value::from("tags"), Value::Sequence(tags.into_iter().map(Value::from).collect()));
    let listed: Vec<Value> = meetings
        .iter()
        .map(|m| {
            let mut item = Mapping::new();
            frontmatter::set(&mut item, "title", heading_of(m));
            if let Some(time) = &m.time {
                frontmatter::set(&mut item, "time", time.clone());
            }
            if !m.projects.is_empty() {
                item.insert(Value::from("projects"), Value::Sequence(m.projects.iter().cloned().map(Value::from).collect()));
            }
            Value::Mapping(item)
        })
        .collect();
    if !listed.is_empty() {
        meta.insert(Value::from("meetings"), Value::Sequence(listed));
    }
    for (key, value) in extra {
        meta.insert(key.clone(), value.clone());
    }

    let mut parts = vec![];
    if !preamble.trim().is_empty() {
        parts.push(preamble.trim().to_string());
    }
    for m in meetings {
        let mut lines = vec![format!("## {}", heading_of(m))];
        if !m.people.is_empty() {
            lines.push(format!("With: {}", m.people.iter().map(|p| format!("[[{}{p}]]", crate::links::PEOPLE)).collect::<Vec<_>>().join(", ")));
        }
        if !m.body.trim().is_empty() {
            lines.push(String::new());
            lines.push(m.body.trim().to_string());
        }
        parts.push(lines.join("\n"));
    }
    frontmatter::render(&meta, &parts.join("\n\n"))
}

// ---- the database --------------------------------------------------------------------------------------

fn joined(items: &[String]) -> String {
    items.join("\n")
}

fn split(text: String) -> Vec<String> {
    text.lines().filter(|l| !l.is_empty()).map(str::to_string).collect()
}

/// The id a day's note goes by in the app, from its date: the file holds no id of its own.
fn id_of(date: &str) -> String {
    format!("daily-{date}")
}

/// Stores a day's note as read or saved, replacing what the day had.
pub fn store(conn: &Connection, date: &str, parsed: &ParsedDaily, updated_at: &str) -> Result<()> {
    conn.execute("DELETE FROM daily_notes WHERE date = ?1", [date])?;
    let extra = serde_yaml_ng::to_string(&Value::Mapping(parsed.extra.clone())).unwrap_or_default();
    conn.execute(
        "INSERT INTO daily_notes (date, id, preamble, extra, own_tags, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![date, id_of(date), parsed.preamble, if parsed.extra.is_empty() { String::new() } else { extra }, joined(&parsed.own_tags), updated_at],
    )?;
    for (position, m) in parsed.meetings.iter().enumerate() {
        conn.execute(
            "INSERT INTO daily_meetings (date, position, time, title, projects, people, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![date, position as i64, m.time, m.title, joined(&m.projects), joined(&m.people), m.body],
        )?;
    }
    Ok(())
}

fn meetings_of(conn: &Connection, date: &str) -> Result<Vec<MeetingSection>> {
    query_all(conn, "SELECT time, title, projects, people, body FROM daily_meetings WHERE date = ?1 ORDER BY position", [date], |r| {
        Ok(MeetingSection { time: r.get(0)?, title: r.get(1)?, projects: split(r.get(2)?), people: split(r.get(3)?), body: r.get(4)? })
    })
}

pub fn get(conn: &Connection, date: &str) -> Result<Option<DailyNote>> {
    let Some((id, updated_at)) =
        query_opt(conn, "SELECT id, updated_at FROM daily_notes WHERE date = ?1", [date], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
    else {
        return Ok(None);
    };
    Ok(Some(DailyNote { date: date.to_string(), id, meetings: meetings_of(conn, date)?, updated_at }))
}

/// The days' notes, newest first; with a project or a person, only the days with a meeting of theirs.
pub fn list(conn: &Connection, project: Option<&str>, person: Option<&str>) -> Result<Vec<DailyNote>> {
    let dates = query_all(conn, "SELECT date FROM daily_notes ORDER BY date DESC", [], |r| r.get::<_, String>(0))?;
    let mut out = vec![];
    for date in dates {
        let Some(note) = get(conn, &date)? else { continue };
        let of_project = project.is_none_or(|p| note.meetings.iter().any(|m| m.projects.iter().any(|x| x == p)));
        let of_person = person.is_none_or(|p| note.meetings.iter().any(|m| m.people.iter().any(|x| x == p)));
        if of_project && of_person {
            out.push(note);
        }
    }
    Ok(out)
}

/// A tag as kept: no `#`, no spaces at either end, levels without empty parts.
fn clean_tag(tag: &str) -> Option<String> {
    let tag = tag.trim().trim_start_matches('#');
    let parts: Vec<String> = tag.split('/').map(|p| p.trim().replace(char::is_whitespace, "-")).filter(|p| !p.is_empty()).collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    tags.iter().filter_map(|t| clean_tag(t)).filter(|t| seen.insert(t.clone())).collect()
}

/// Who was at a meeting, by id, each once: an id as typed (`frank`, `people/frank`, `Frank de Vries`) becomes the id of
/// someone in shared/people, who is added to the vault when they aren't in it yet (named after the id), so every link
/// in the `With:` line leads to a file.
fn people_of(conn: &Connection, typed: &[String]) -> Result<Vec<String>> {
    let mut out: Vec<String> = vec![];
    for text in typed {
        let id = ids::slugify_as(text.trim().trim_start_matches('#').trim_start_matches(crate::links::PEOPLE), "");
        if id.is_empty() || out.contains(&id) {
            continue;
        }
        let id = if crate::people::exists(conn, &id)? {
            id
        } else {
            let name = id.split('-').filter(|w| !w.is_empty()).map(capitalised).collect::<Vec<_>>().join(" ");
            crate::people::create(conn, &crate::people::PersonDraft { name, ..Default::default() })?.id
        };
        if !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Saves a day's note: a day has one, so this replaces its meetings. A day without meetings (and nothing else) has no note.
pub fn save(conn: &Connection, draft: &DailyDraft) -> Result<Option<DailyNote>> {
    if !is_day(&draft.date) {
        return Err(Error::invalid("A day is written as 2026-11-09"));
    }
    let meetings: Vec<MeetingSection> = draft
        .meetings
        .iter()
        .map(|m| -> Result<MeetingSection> {
            Ok(MeetingSection {
                time: m.time.as_deref().and_then(|t| clock(t.trim())),
                // A title is one line: a heading can't hold more.
                title: m.title.split_whitespace().collect::<Vec<_>>().join(" "),
                projects: clean_tags(&m.projects).into_iter().map(|p| p.trim_start_matches("project/").to_string()).collect(),
                people: people_of(conn, &m.people)?,
                // A line starting with `## ` would start another meeting when the note is read back.
                body: m.body.lines().map(|l| if l.starts_with("## ") { format!("#{l}") } else { l.to_string() }).collect::<Vec<_>>().join("\n").trim().to_string(),
            })
        })
        .collect::<Result<_>>()?;
    // What the file holds besides the meetings stays as it was.
    let kept = query_opt(conn, "SELECT preamble, extra, own_tags FROM daily_notes WHERE date = ?1", [&draft.date], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?;
    let (preamble, extra, own_tags) = kept.unwrap_or_default();
    let extra = match serde_yaml_ng::from_str::<Value>(&extra) {
        Ok(Value::Mapping(map)) => map,
        _ => Mapping::new(),
    };
    // A day whose last meeting is taken out has no note, unless the note holds more than meetings (text of your own,
    // frontmatter of your own): then that stays.
    if meetings.is_empty() && preamble.trim().is_empty() && extra.is_empty() {
        delete(conn, &draft.date)?;
        return Ok(None);
    }
    let parsed = ParsedDaily { meetings, preamble, extra, own_tags: split(own_tags) };
    store(conn, &draft.date, &parsed, &ids::now())?;
    get(conn, &draft.date)
}

pub fn delete(conn: &Connection, date: &str) -> Result<()> {
    conn.execute("DELETE FROM daily_meetings WHERE date = ?1", [date])?;
    conn.execute("DELETE FROM daily_notes WHERE date = ?1", [date])?;
    Ok(())
}

/// Every day's note as its file, for the vault.
pub fn export(conn: &Connection) -> Result<Vec<(String, String)>> {
    let rows = query_all(conn, "SELECT date, preamble, extra, own_tags FROM daily_notes ORDER BY date", [], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
    })?;
    let mut out = vec![];
    for (date, preamble, extra, own_tags) in rows {
        let extra = match serde_yaml_ng::from_str::<Value>(&extra) {
            Ok(Value::Mapping(map)) => map,
            _ => Mapping::new(),
        };
        let meetings = meetings_of(conn, &date)?;
        out.push((file(&date), render(&date, &meetings, &preamble, &extra, &split(own_tags))));
    }
    Ok(out)
}
