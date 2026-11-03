//! Semesters and their sprints. A semester is set by hand (a name, a first and a last day; usually half a year),
//! with its vacations, and cut into sprints: three weeks each unless chosen otherwise, and each sprint can have a
//! length of its own. Sprints are counted in weeks of work: the clock stops during a vacation, so a sprint that
//! runs into one carries on after it, and one that would start in it starts after it. Less than a week of work
//! left at the end joins the last sprint. Sprints and vacations stay inside the semester, in order, without
//! overlapping their own kind. Semesters don't overlap, so a day belongs to one semester at most. Kept as
//! `semesters/<id>.md`.
//!
//! Dates are calendar days, inclusive at both ends: no time zone is involved. They are stored as "2026-09-01", the
//! form YAML and other tools expect; a hand-written file may also use the European "01-09-2026". Messages show the
//! European form.

use chrono::{Days, NaiveDate};
use rusqlite::{Connection, params};

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::ids;
use crate::models::{SemesterDraft, SemesterView, Sprint, Vacation};

const DEFAULT_WEEKS: i64 = 3;
const FORMAT: &str = "%Y-%m-%d";
const EUROPEAN: &str = "%d-%m-%Y";

/// A draft that passed every check, with its sprints filled in.
pub struct Checked {
    pub name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub sprint_weeks: i64,
    pub notes: String,
    pub sprints: Vec<Sprint>,
    pub vacations: Vec<Vacation>,
}

fn day(text: &str, what: &str) -> Result<NaiveDate> {
    let text = text.trim();
    NaiveDate::parse_from_str(text, FORMAT)
        .or_else(|_| NaiveDate::parse_from_str(text, EUROPEAN))
        .map_err(|_| Error::invalid(format!("{what} is not a date (use DD-MM-YYYY)")))
}

/// "01-09-2026", for messages.
fn european(stored: &str) -> String {
    NaiveDate::parse_from_str(stored, FORMAT).map_or_else(|_| stored.to_string(), |d| d.format(EUROPEAN).to_string())
}

fn text(date: NaiveDate) -> String {
    date.format(FORMAT).to_string()
}

fn after(date: NaiveDate, days: u64) -> NaiveDate {
    date.checked_add_days(Days::new(days)).unwrap_or(date)
}

/// Sprints of `weeks` weeks of work from `start` to `end`, around the `vacations` (first and last days); a leftover
/// of less than a week of work joins the last sprint. The frontend's `planSprints` cuts the same way.
pub fn plan(start: NaiveDate, end: NaiveDate, weeks: i64, vacations: &[(NaiveDate, NaiveDate)]) -> Vec<Sprint> {
    let off = |d: NaiveDate| vacations.iter().any(|(a, b)| d >= *a && d <= *b);
    let need = weeks.max(1) * 7;
    let mut sprints: Vec<Sprint> = vec![];
    let mut day = start;
    loop {
        while day <= end && off(day) {
            day = after(day, 1);
        }
        if day > end {
            break;
        }
        let from = day;
        let (mut worked, mut until) = (0, day);
        while day <= end && worked < need {
            if !off(day) {
                worked += 1;
                until = day;
            }
            day = after(day, 1);
        }
        match sprints.last_mut() {
            Some(last) if worked < 7 => last.end = text(until),
            _ => sprints.push(Sprint { name: format!("Sprint {}", sprints.len() + 1), start: text(from), end: text(until), goal: String::new() }),
        }
        day = after(until, 1);
    }
    sprints
}

/// Checks a draft on its own (not against other semesters) and fills in what it leaves out.
pub fn check(draft: &SemesterDraft) -> Result<Checked> {
    let name = draft.name.trim();
    if name.is_empty() {
        return Err(Error::invalid("Name is required"));
    }
    let start = day(&draft.start, "The first day")?;
    let end = day(&draft.end, "The last day")?;
    if end < start {
        return Err(Error::invalid("The semester ends before it starts"));
    }
    if (end - start).num_days() > 366 {
        return Err(Error::invalid("A semester is at most a year long"));
    }
    let sprint_weeks = if draft.sprint_weeks == 0 { DEFAULT_WEEKS } else { draft.sprint_weeks };
    if !(1..=12).contains(&sprint_weeks) {
        return Err(Error::invalid("A sprint is 1 to 12 weeks long"));
    }

    let mut vacations: Vec<(NaiveDate, NaiveDate, String)> = vec![];
    for v in &draft.vacations {
        let label = if v.name.trim().is_empty() { "Vacation".to_string() } else { v.name.trim().to_string() };
        let from = day(&v.start, &format!("The first day of {label}"))?;
        let until = day(&v.end, &format!("The last day of {label}"))?;
        if until < from {
            return Err(Error::invalid(format!("{label} ends before it starts")));
        }
        if from < start || until > end {
            return Err(Error::invalid(format!("{label} falls outside the semester")));
        }
        vacations.push((from, until, label));
    }
    vacations.sort_by_key(|v| v.0);
    if let Some(w) = vacations.windows(2).find(|w| w[1].0 <= w[0].1) {
        return Err(Error::invalid(format!("{} and {} overlap", w[0].2, w[1].2)));
    }
    let days_off: Vec<(NaiveDate, NaiveDate)> = vacations.iter().map(|v| (v.0, v.1)).collect();

    let sprints = if draft.sprints.is_empty() {
        plan(start, end, sprint_weeks, &days_off)
    } else {
        let mut out: Vec<Sprint> = vec![];
        let mut previous_end: Option<NaiveDate> = None;
        for (i, sprint) in draft.sprints.iter().enumerate() {
            let label = if sprint.name.trim().is_empty() { format!("Sprint {}", i + 1) } else { sprint.name.trim().to_string() };
            let from = day(&sprint.start, &format!("The first day of {label}"))?;
            let until = day(&sprint.end, &format!("The last day of {label}"))?;
            if until < from {
                return Err(Error::invalid(format!("{label} ends before it starts")));
            }
            if from < start || until > end {
                return Err(Error::invalid(format!("{label} falls outside the semester")));
            }
            if previous_end.is_some_and(|p| from <= p) {
                return Err(Error::invalid(format!("{label} starts before the sprint before it ends")));
            }
            previous_end = Some(until);
            out.push(Sprint { name: label, start: text(from), end: text(until), goal: sprint.goal.trim().to_string() });
        }
        out
    };

    if sprints.is_empty() {
        return Err(Error::invalid("The semester has no days outside its vacations"));
    }
    let vacations = vacations.into_iter().map(|(a, b, name)| Vacation { name, start: text(a), end: text(b) }).collect();
    Ok(Checked { name: name.to_string(), start, end, sprint_weeks, notes: draft.notes.trim().to_string(), sprints, vacations })
}

/// Refuses a semester that shares a day with another one (other than `own`, the one being changed).
fn check_overlap(conn: &Connection, own: Option<&str>, checked: &Checked) -> Result<()> {
    let clash = query_opt(
        conn,
        "SELECT name, start_date, end_date FROM semesters WHERE id != ?1 AND start_date <= ?3 AND end_date >= ?2 ORDER BY start_date LIMIT 1",
        params![own.unwrap_or(""), text(checked.start), text(checked.end)],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
    )?;
    match clash {
        Some((name, from, until)) => Err(Error::invalid(format!(
            "It overlaps “{name}” ({} to {}); semesters can't share days",
            european(&from),
            european(&until)
        ))),
        None => Ok(()),
    }
}

fn write_parts(conn: &Connection, id: &str, checked: &Checked) -> Result<()> {
    conn.execute("DELETE FROM sprints WHERE semester_id = ?1", [id])?;
    for (position, s) in checked.sprints.iter().enumerate() {
        conn.execute(
            "INSERT INTO sprints (semester_id, position, name, start_date, end_date, goal) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, position as i64, s.name, s.start, s.end, s.goal],
        )?;
    }
    conn.execute("DELETE FROM vacations WHERE semester_id = ?1", [id])?;
    for (position, v) in checked.vacations.iter().enumerate() {
        conn.execute(
            "INSERT INTO vacations (semester_id, position, name, start_date, end_date) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, position as i64, v.name, v.start, v.end],
        )?;
    }
    Ok(())
}

/// Stores a checked semester under `id` as it is, without looking at other semesters: what reading the vault uses.
pub fn insert(conn: &Connection, id: &str, checked: &Checked, created_at: &str, updated_at: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO semesters (id, name, start_date, end_date, sprint_weeks, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, checked.name, text(checked.start), text(checked.end), checked.sprint_weeks, checked.notes, created_at, updated_at],
    )?;
    write_parts(conn, id, checked)
}

fn fill(conn: &Connection, s: &mut SemesterView) -> Result<()> {
    s.sprints = query_all(conn, "SELECT name, start_date, end_date, goal FROM sprints WHERE semester_id = ?1 ORDER BY position", [&s.id], |r| {
        Ok(Sprint { name: r.get(0)?, start: r.get(1)?, end: r.get(2)?, goal: r.get(3)? })
    })?;
    s.vacations = query_all(conn, "SELECT name, start_date, end_date FROM vacations WHERE semester_id = ?1 ORDER BY position", [&s.id], |r| {
        Ok(Vacation { name: r.get(0)?, start: r.get(1)?, end: r.get(2)? })
    })?;
    Ok(())
}

const SELECT: &str = "SELECT id, name, start_date, end_date, sprint_weeks, notes, created_at, updated_at FROM semesters";

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SemesterView> {
    Ok(SemesterView {
        id: r.get(0)?,
        name: r.get(1)?,
        start: r.get(2)?,
        end: r.get(3)?,
        sprint_weeks: r.get(4)?,
        notes: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
        sprints: vec![],
        vacations: vec![],
    })
}

/// Every semester, the earliest first, with its sprints.
pub fn list(conn: &Connection) -> Result<Vec<SemesterView>> {
    let mut all = query_all(conn, &format!("{SELECT} ORDER BY start_date, id"), [], from_row)?;
    for s in &mut all {
        fill(conn, s)?;
    }
    Ok(all)
}

pub fn get(conn: &Connection, id: &str) -> Result<SemesterView> {
    let mut semester = query_opt(conn, &format!("{SELECT} WHERE id = ?1"), [id], from_row)?.ok_or_else(|| Error::not_found("Semester not found"))?;
    fill(conn, &mut semester)?;
    Ok(semester)
}

pub fn create(conn: &Connection, draft: &SemesterDraft) -> Result<SemesterView> {
    let checked = check(draft)?;
    check_overlap(conn, None, &checked)?;
    let base = ids::slugify_as(&checked.name, "semester");
    let mut id = base.clone();
    let mut n = 2;
    while query_opt(conn, "SELECT 1 FROM semesters WHERE id = ?1", [&id], |r| r.get::<_, i64>(0))?.is_some() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let now = ids::now();
    let tx = conn.unchecked_transaction()?;
    insert(&tx, &id, &checked, &now, &now)?;
    tx.commit()?;
    get(conn, &id)
}

/// Replaces everything about a semester but its id and when it was made.
pub fn update(conn: &Connection, id: &str, draft: &SemesterDraft) -> Result<SemesterView> {
    get(conn, id)?;
    let checked = check(draft)?;
    check_overlap(conn, Some(id), &checked)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE semesters SET name = ?2, start_date = ?3, end_date = ?4, sprint_weeks = ?5, notes = ?6, updated_at = ?7 WHERE id = ?1",
        params![id, checked.name, text(checked.start), text(checked.end), checked.sprint_weeks, checked.notes, ids::now()],
    )?;
    write_parts(&tx, id, &checked)?;
    tx.commit()?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM semesters WHERE id = ?1", [id])? == 0 {
        return Err(Error::not_found("Semester not found"));
    }
    Ok(())
}
