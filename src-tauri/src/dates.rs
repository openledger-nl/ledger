//! Dates and times, the one way Ledger handles them on this side (the front end's twin is `src/lib/format.ts`):
//!
//! - a **moment** (when something was made, published, changed) is stored as ISO 8601 in UTC with milliseconds,
//!   `2026-11-20T10:19:32.123Z` ([`crate::ids::now`]);
//! - a **calendar day** (a meeting note, a semester's start, a project's end) is stored as `2026-11-20` ([`parse_day`],
//!   [`day_key`]);
//! - everything shown to people is in **Amsterdam** time (Central European time, summer time included), whatever the
//!   computer's own time zone, and written the European way: `20-11-2026` ([`numeric_day`]), or in full on a
//!   document, `20 November 2026` ([`long_day`]). Times of day are shown by the front end, as `14:05`.
//!
//! Amsterdam's offset follows the EU rule: an hour ahead of UTC, two from the last Sunday of March to the last Sunday
//! of October, switching at 01:00 UTC. Computed here rather than read from a time-zone database, which would carry
//! every zone of the world for this one.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc, Weekday};

/// How a calendar day is stored, in files and in the database.
pub const DAY_FORMAT: &str = "%Y-%m-%d";

const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// The last Sunday of a month.
fn last_sunday(year: i32, month: u32) -> NaiveDate {
    let first_of_next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, month + 1, 1) };
    let mut day = first_of_next.unwrap_or_default() - Duration::days(1);
    while day.weekday() != Weekday::Sun {
        day -= Duration::days(1);
    }
    day
}

/// How far Amsterdam's clock is ahead of UTC at a moment: one hour, or two in summer time.
pub fn amsterdam_offset(at: DateTime<Utc>) -> Duration {
    let year = at.year();
    let switch = |month| Utc.from_utc_datetime(&last_sunday(year, month).and_hms_opt(1, 0, 0).unwrap_or_default());
    if at >= switch(3) && at < switch(10) { Duration::hours(2) } else { Duration::hours(1) }
}

/// A moment as Amsterdam's wall clock reads it.
pub fn in_amsterdam(at: DateTime<Utc>) -> NaiveDateTime {
    at.naive_utc() + amsterdam_offset(at)
}

/// A stored moment (`2026-11-20T10:19:32.123Z`, or any RFC 3339 time); `None` for text that isn't one.
pub fn parse_moment(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text.trim()).ok().map(|d| d.with_timezone(&Utc))
}

/// A stored calendar day (`2026-11-20`); `None` for anything else.
pub fn parse_day(text: &str) -> Option<NaiveDate> {
    let text = text.trim();
    if text.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(text, DAY_FORMAT).ok()
}

/// Whether text is a stored calendar day.
pub fn is_day(text: &str) -> bool {
    parse_day(text).is_some()
}

/// A day as it is stored: `2026-11-20`.
pub fn day_key(day: NaiveDate) -> String {
    day.format(DAY_FORMAT).to_string()
}

/// Today in Amsterdam.
pub fn today() -> NaiveDate {
    in_amsterdam(Utc::now()).date()
}

/// The calendar day a stored moment fell on in Amsterdam; `None` for text that isn't a moment.
pub fn day_of(moment: &str) -> Option<NaiveDate> {
    parse_moment(moment).map(|at| in_amsterdam(at).date())
}

/// `20-11-2026`: the one way a day is written in the app (no leading zeros).
pub fn numeric_day(day: NaiveDate) -> String {
    format!("{}-{}-{}", day.day(), day.month(), day.year())
}

/// `20 November 2026`: a day written in full, as on a document's cover.
pub fn long_day(day: NaiveDate) -> String {
    format!("{} {} {}", day.day(), MONTHS[day.month0() as usize], day.year())
}

/// A day from what a person typed: stored (`2026-11-20`) or European (`20-11-2026`, `20-11-26` is not accepted).
pub fn parse_typed_day(text: &str) -> Option<NaiveDate> {
    parse_day(text).or_else(|| NaiveDate::parse_from_str(text.trim(), "%d-%m-%Y").ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        parse_moment(text).unwrap()
    }

    #[test]
    fn amsterdam_is_an_hour_ahead_in_winter_and_two_in_summer() {
        assert_eq!(amsterdam_offset(at("2026-01-15T12:00:00Z")), Duration::hours(1));
        assert_eq!(amsterdam_offset(at("2026-07-15T12:00:00Z")), Duration::hours(2));
        // Summer time starts on 29 March 2026 at 01:00 UTC and ends on 25 October 2026 at 01:00 UTC.
        assert_eq!(amsterdam_offset(at("2026-03-29T00:59:59Z")), Duration::hours(1));
        assert_eq!(amsterdam_offset(at("2026-03-29T01:00:00Z")), Duration::hours(2));
        assert_eq!(amsterdam_offset(at("2026-10-25T00:59:59Z")), Duration::hours(2));
        assert_eq!(amsterdam_offset(at("2026-10-25T01:00:00Z")), Duration::hours(1));
    }

    #[test]
    fn a_moment_falls_on_the_day_it_was_in_amsterdam() {
        // Half past midnight in Amsterdam is still the day before in UTC.
        assert_eq!(day_of("2026-11-19T23:30:00.000Z").map(day_key).as_deref(), Some("2026-11-20"));
        assert_eq!(day_of("2026-07-19T22:30:00.000Z").map(day_key).as_deref(), Some("2026-07-20"));
        assert_eq!(day_of("not a moment"), None);
    }

    #[test]
    fn days_and_times_are_written_the_european_way() {
        let day = parse_day("2026-02-02").unwrap();
        assert_eq!(numeric_day(day), "2-2-2026");
        assert_eq!(long_day(day), "2 February 2026");
        assert_eq!(in_amsterdam(at("2026-11-20T13:05:00.000Z")).format("%H:%M").to_string(), "14:05");
        assert_eq!(parse_typed_day("20-11-2026").map(day_key).as_deref(), Some("2026-11-20"));
        assert!(!is_day("2026-2-2") && !is_day("2026-13-01") && is_day("2026-12-31"));
    }
}
