//! A markdown deliverable as a PDF, set with Typst inside Ledger: no other program is needed. The PDF has a cover
//! (title, subtitle, who it is for and by, version and date), the version history from the published versions, a
//! table of contents when the document has a few headings, and then the document, with a header and a footer.
//!
//! Who it names and how it looks (your name, your own side and the client, each with a logo, and the accent colour) is
//! the project's branding ([`branding`]).
//!
//! The PDF of a published version is `exports/<name> v<version>.pdf` in the project's folder, that of the live file
//! `exports/<name> (draft).pdf`; exporting again replaces it. A version's PDF says what that version said, so it can
//! be made again at any time and comes out the same (apart from its date when it is a draft).
//!
//! Exporting is two steps, so the vault is only held while it is read: [`prepare`] gathers the text, the pictures and
//! what is known about the versions, and [`make`] sets it and writes the file.

mod markdown;
mod world;

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;

use crate::db::query_opt;
use crate::error::{Error, Result};
use crate::frontmatter::{self, MetaExt};
use crate::{branding, deliverable_facts, deliverables, images, project_facts, provenance, uploads};

/// The accent a project gets when it has none of its own: the orange of the report template the layout follows.
pub const DEFAULT_ACCENT: &str = "#e07a3f";

/// What the layout is told about the document (`/data.json`, read by `ledger-doc` in ledger.typ).
#[derive(Debug, Serialize)]
struct Data {
    title: String,
    subtitle: Option<String>,
    /// The version the PDF is of; `None` for the live file, a draft.
    version: Option<String>,
    date: String,
    author: Option<String>,
    project: String,
    accent: String,
    lang: String,
    client: Party,
    own: Party,
    history: Vec<HistoryRow>,
    contents: bool,
    /// The AI-use label on the cover, when the project shows it.
    label: Option<Label>,
}

/// An MMM label: its name, its picture in the PDF's folder, and whether it is only the one intended (a draft has no
/// label of its own yet).
#[derive(Debug, Serialize)]
struct Label {
    name: String,
    picture: String,
    intended: bool,
}

/// Someone named on the cover: who the document is for (the client) or who made it (your own side).
#[derive(Debug, Default, Serialize)]
struct Party {
    name: Option<String>,
    /// The logo's name inside the PDF's folder, when there is one.
    logo: Option<String>,
}

#[derive(Debug, Serialize)]
struct HistoryRow {
    version: String,
    date: String,
    by: String,
    note: String,
    label: Option<String>,
}

/// Everything a PDF is made from, gathered from the vault.
pub struct Prepared {
    main: String,
    files: Vec<(String, Vec<u8>)>,
    /// Where it goes, on disk and relative to the project's folder.
    out: PathBuf,
    rel: String,
}

/// Gathers what the PDF of the deliverable at `path` (the live file or a published version) is made from.
pub fn prepare(conn: &Connection, root: &Path, project: &str, path: &str) -> Result<Prepared> {
    let doc = deliverables::get(conn, project, path)?;
    let Some(source) = doc.body.as_deref() else {
        return Err(Error::invalid("Only a markdown deliverable can be exported as a PDF"));
    };
    let base = doc.base_path.clone();
    let project_name: String =
        query_opt(conn, "SELECT name FROM projects WHERE slug = ?1", [project], |r| r.get(0))?.unwrap_or_else(|| project.to_string());

    // Frontmatter may name the title, a subtitle and the language; the rest of the file is the document.
    let (meta, text) = match frontmatter::parse(source) {
        Some(parsed) => (parsed.meta, parsed.body),
        None => (Default::default(), source.to_string()),
    };

    // Pictures are looked for beside the live file, as on the page (vaultImages.ts), and never outside the project.
    let project_dir = uploads::project_dir(root, project);
    let dir = format!("deliverables/{}", base.rsplit_once('/').map_or("", |(d, _)| d));
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut picture = |src: &str| -> Option<String> {
        let rel = resolve_src(&dir, src)?;
        let file = project_dir.join(&rel);
        let ext = rel.rsplit_once('.')?.1.to_ascii_lowercase();
        let ext = match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => ext,
            "jxl" => "png".to_string(),
            _ => return None,
        };
        let bytes = images::displayable_file(&file).ok()?;
        let name = format!("/pictures/{}.{ext}", files.len());
        files.push((name.clone(), bytes));
        Some(name)
    };
    let converted = markdown::to_typst(&text, &mut picture);
    let brand = branding::get(conn, project)?;
    let mut logo = |rel: &Option<String>, side: &str| -> Option<String> {
        let rel = rel.as_deref()?;
        let ext = rel.rsplit_once('.')?.1.to_ascii_lowercase();
        let bytes = fs::read(project_dir.join(rel)).ok()?;
        let name = format!("/logos/{side}.{ext}");
        files.push((name.clone(), bytes));
        Some(name)
    };
    let own_logo = logo(&brand.own_logo, "own");
    let client_logo = logo(&brand.client_logo, "client");

    let stem = file_stem(&doc.name);
    let title = meta.text("title").or(converted.title.clone()).unwrap_or_else(|| stem.clone());
    let deliverable_meta = deliverable_facts::get(conn, project, &base)?;
    let goal = deliverable_meta.as_ref().map(|m| m.goal.clone()).filter(|g| !g.trim().is_empty());
    let subtitle = meta.text("subtitle").or_else(|| meta.text("description")).or(goal);

    // The versions up to the one exported (all of them for the live file), oldest first.
    let versions = provenance::resolve(conn, project, &base)?;
    let upto = match &doc.version {
        Some(v) => versions.iter().position(|r| &r.version == v).map_or(versions.len(), |i| i + 1),
        None => versions.len(),
    };
    let history: Vec<HistoryRow> = versions[..upto]
        .iter()
        .map(|r| HistoryRow {
            version: r.version.clone(),
            date: long_date(&r.at),
            by: match (r.author, r.run.is_some()) {
                (provenance::AI, true) => "Claude Code".to_string(),
                (provenance::AI, false) => "With AI".to_string(),
                (provenance::HUMAN, _) => brand.author.clone().unwrap_or_else(|| "Person".to_string()),
                _ => "-".to_string(),
            },
            note: if r.note.trim().is_empty() { "-".to_string() } else { r.note.trim().to_string() },
            label: r.mmm.as_deref().and_then(label_name).map(str::to_string),
        })
        .collect();
    let date = match &doc.version {
        Some(v) => versions.iter().find(|r| &r.version == v).map_or_else(|| long_date(&doc.modified), |r| long_date(&r.at)),
        None => long_date(&chrono::Local::now().date_naive().to_string()),
    };

    // Who it is for: as the branding says, else a company project's client, else the first client among its people.
    let facts = project_facts::get(conn, project)?;
    let kind = facts.as_ref().map(|f| f.kind.clone()).unwrap_or_default();
    let intended_mmm = facts.as_ref().and_then(|f| f.intended_mmm.clone());
    let client_name = brand.client_name.clone().or_else(|| facts.and_then(|f| f.client)).or_else(|| {
        project_facts::people(conn, Some(project)).ok()?.into_iter().find(|p| p.role == "client").map(|p| p.name)
    });

    // The AI-use label, bottom left on the cover: the version's own, else the one meant for this deliverable or for the
    // project. Shown when the project says so, and by default in a study project (where a version must have one).
    let label = if brand.show_label.unwrap_or(kind == "study") {
        let recorded = match &doc.version {
            Some(v) => versions.iter().find(|r| &r.version == v).and_then(|r| r.mmm.clone()),
            None => None,
        };
        let intended = recorded.is_none();
        recorded
            .or_else(|| deliverable_meta.as_ref().and_then(|m| m.mmm.clone()))
            .or(intended_mmm)
            .and_then(|level| provenance::level(&level))
            .map(|level| {
                let picture = format!("/labels/{level}.svg");
                files.push((picture.clone(), label_picture(level).to_vec()));
                Label { name: label_name(level).unwrap_or(level).to_string(), picture, intended }
            })
    } else {
        None
    };

    let data = Data {
        title,
        subtitle,
        version: doc.version.clone(),
        date,
        author: brand.author.clone(),
        project: project_name,
        accent: brand.accent.clone().unwrap_or_else(|| DEFAULT_ACCENT.to_string()),
        lang: meta.text("lang").filter(|l| l.len() == 2 && l.chars().all(|c| c.is_ascii_alphabetic())).unwrap_or_else(|| "en".into()),
        client: Party { name: client_name, logo: client_logo },
        own: Party { name: brand.own_name.clone(), logo: own_logo },
        history,
        contents: converted.headings >= 3,
        label,
    };
    files.push(("/data.json".to_string(), serde_json::to_vec(&data).map_err(|e| Error::export(e.to_string()))?));
    let main = format!("#import \"/ledger.typ\": *\n#show: ledger-doc.with(json(\"/data.json\"))\n\n{}", converted.body);

    let file_name = match &doc.version {
        Some(v) => format!("{} v{v}.pdf", safe_file_name(&stem)),
        None => format!("{} (draft).pdf", safe_file_name(&stem)),
    };
    let rel = format!("exports/{file_name}");
    Ok(Prepared { main, files, out: project_dir.join("exports").join(&file_name), rel })
}

/// Sets the PDF and writes it. Returns where it is, relative to the project's folder.
pub fn make(prepared: Prepared) -> Result<String> {
    let pdf = world::compile(&prepared.main, prepared.files)?;
    if let Some(dir) = prepared.out.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::write(&prepared.out, pdf).map_err(|e| Error::io(&prepared.out, e))?;
    Ok(prepared.rel)
}

/// The project path a picture's address points at from `dir`, or `None` for a web address or one that leaves the
/// project. A leading `/` is the project's own folder. (As `resolveSrc` in vaultImages.ts.)
fn resolve_src(dir: &str, src: &str) -> Option<String> {
    let src = src.trim();
    let lower = src.to_ascii_lowercase();
    if lower.starts_with("//") || lower.starts_with('#') || lower.split_once(':').is_some_and(|(scheme, _)| !scheme.contains('/')) {
        return None;
    }
    let path = src.split(['?', '#']).next().unwrap_or("");
    let path = percent_decode(path);
    let mut parts: Vec<&str> = if path.starts_with('/') { Vec::new() } else { dir.split('/').filter(|p| !p.is_empty()).collect() };
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// `%20` and the like as the characters they stand for; text that isn't valid %-encoding stays as written.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = text.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

/// A file's name without its extension.
fn file_stem(name: &str) -> String {
    let name = deliverables::file_name(name);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => name.to_string(),
    }
}

/// A name that is a valid file name on every system.
fn safe_file_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '-' } else { c }).collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() { "deliverable".to_string() } else { cleaned }
}

/// "2026-11-20T10:19:32Z" (or a plain day) as "20 November 2026"; anything else as it is.
fn long_date(at: &str) -> String {
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let day = at.get(..10).and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    match day {
        Some(d) => format!("{} {} {}", d.day(), MONTHS[d.month0() as usize], d.year()),
        None => at.to_string(),
    }
}

/// The official picture of an MMM level, as the app shows it (src/lib/assets/mmm).
fn label_picture(level: &str) -> &'static [u8] {
    match level {
        "lazy-prompter" => include_bytes!("../../../src/lib/assets/mmm/lazy-prompter.svg"),
        "piggybacker" => include_bytes!("../../../src/lib/assets/mmm/piggybacker.svg"),
        "cyborg" => include_bytes!("../../../src/lib/assets/mmm/cyborg.svg"),
        "handyman" => include_bytes!("../../../src/lib/assets/mmm/handyman.svg"),
        _ => include_bytes!("../../../src/lib/assets/mmm/man-made.svg"),
    }
}

/// An MMM level as the label reads.
fn label_name(level: &str) -> Option<&'static str> {
    Some(match provenance::level(level)? {
        "lazy-prompter" => "Lazy Prompter",
        "piggybacker" => "Piggybacker",
        "cyborg" => "Cyborg",
        "handyman" => "Handyman",
        _ => "Man-Made",
    })
}

#[cfg(test)]
mod tests;
