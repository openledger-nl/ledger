//! Deliverables are the files prompts produce, kept in a project's `deliverables/` folder. They can be anything;
//! markdown ones can be read and edited here. Publishing freezes a copy as "<name>@v1.0.<ext>" in the project's
//! `versions/` folder, in a folder of the deliverable's own named after it, and that snapshot never changes. Beside it
//! is its manifest, how it came to be (see `provenance`). So `deliverables/` holds only the live files, and reads the
//! same in any other tool:
//!
//! ```text
//! deliverables/reports/plan.md                         the live file
//! deliverables/reports/plan.desc.md                    its evidence description (see `deliverable_facts`): no
//!                                                      deliverable of its own
//! versions/reports/plan/plan@v1.2.md                   its version 1.2
//! versions/reports/plan/plan@v1.2.manifest.md          who made 1.2, in which thread, with what AI-use label
//! ```
//!
//! A deliverable can go on in another file (see [`continue_in`]): a PDF that came from Portflow, say, continued in the
//! markdown that is edited here. The two are one deliverable from then on.
//!
//! Paths in this module are relative to the deliverables folder and use `/`; a snapshot's path is named the same way
//! ("reports/plan@v1.2.md") even though its file is under `versions/` ([`in_project`]).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::models::{Bump, Delivered, DeliverableSummary, DeliverableView, Earlier, PublishChoices};
use crate::posts::{self, AddOptions};
use crate::provenance::{self, Origin};
use crate::{feedback, frontmatter, ids, markdown, projects, prompts, store, uploads};

// ---- paths and versions -----------------------------------------------------------------------------

pub fn is_markdown(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

/// "markdown", "image" or "other".
pub fn kind_of(path: &str) -> &'static str {
    if is_markdown(path) {
        "markdown"
    } else if uploads::is_image(path) {
        "image"
    } else {
        "other"
    }
}

pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// "M.m" with both parts plain digits.
pub fn parse_version(version: &str) -> Option<(u32, u32)> {
    let (major, minor) = version.split_once('.')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(major) || !digits(minor) {
        return None;
    }
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// Minor goes 1.0 → 1.1, major goes 1.4 → 2.0; the first publish of anything is 0.1 or 1.0.
pub fn bump_version(current: Option<&str>, bump: Bump) -> String {
    let (major, minor) = current.and_then(parse_version).unwrap_or((0, 0));
    match bump {
        Bump::Major => format!("{}.0", major + 1),
        Bump::Minor => format!("{major}.{}", minor + 1),
    }
}

/// Splits "reports/plan@v1.2.md" into ("reports/plan.md", Some("1.2")); anything else is its own base.
pub fn split_version(path: &str) -> (String, Option<String>) {
    let (dir, file) = path.rsplit_once('/').map_or(("", path), |(d, f)| (d, f));
    let join = |file: String| if dir.is_empty() { file } else { format!("{dir}/{file}") };
    if let Some((base, version)) = file.rsplit_once("@v")
        && !base.is_empty()
        && parse_version(version).is_some()
    {
        return (join(base.to_string()), Some(version.to_string()));
    }
    let (stem, ext) = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, Some(ext)),
        _ => (file, None),
    };
    if let Some((base, version)) = stem.rsplit_once("@v")
        && !base.is_empty()
        && parse_version(version).is_some()
    {
        let file = ext.map_or_else(|| base.to_string(), |ext| format!("{base}.{ext}"));
        return (join(file), Some(version.to_string()));
    }
    (path.to_string(), None)
}

/// "reports/plan.md" + "1.2" → "reports/plan@v1.2.md".
pub fn snapshot_path(base: &str, version: &str) -> String {
    let (dir, file) = base.rsplit_once('/').map_or(("", base), |(d, f)| (d, f));
    let file = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem}@v{version}.{ext}"),
        _ => format!("{file}@v{version}"),
    };
    if dir.is_empty() { file } else { format!("{dir}/{file}") }
}

/// Where the live files are on disk in a vault of plain files, for tests that put files there by hand.
#[cfg(test)]
pub fn deliverables_dir(root: &Path, project: &str) -> PathBuf {
    uploads::project_dir(root, project).join("deliverables")
}

/// Where published versions are kept, next to the records of how each came to be (see [`provenance`]), for tests.
#[cfg(test)]
pub fn versions_dir(root: &Path, project: &str) -> PathBuf {
    uploads::project_dir(root, project).join("versions")
}

/// The file on disk for a deliverable path: a live file under `deliverables/`, a published version under `versions/`.
pub fn file_of(root: &Path, project: &str, path: &str) -> PathBuf {
    root.join(rel_of(project, path))
}

/// The deliverable's file by its path in the vault (see `store`).
pub fn rel_of(project: &str, path: &str) -> String {
    format!("projects/{project}/{}", in_project(path))
}

/// The deliverable's file by its path in its project's folder: a live file under `deliverables/`, a published version
/// under `versions/`, in its deliverable's own folder ("reports/plan@v1.2.md" is `versions/reports/plan/plan@v1.2.md`).
pub fn in_project(path: &str) -> String {
    match split_version(path) {
        (base, Some(_)) => format!("versions/{}/{}", attachment_dir(&base), file_name(path)),
        (_, None) => format!("deliverables/{path}"),
    }
}

/// The version a file under a project's `versions/` folder is, named as the app names it ("reports/plan/plan@v1.2.md"
/// is "reports/plan@v1.2.md"). None for anything else there: a manifest, or a version outside its deliverable's folder.
pub fn version_at(rel: &str) -> Option<String> {
    let (dir, file) = rel.rsplit_once('/')?;
    let (base, version) = split_version(file);
    version?;
    let (parent, folder) = dir.rsplit_once('/').unwrap_or(("", dir));
    // Matched whatever the case: a folder renamed by its case alone keeps its old name on Windows and macOS.
    attachment_dir(&base).eq_ignore_ascii_case(folder).then(|| if parent.is_empty() { file.to_string() } else { format!("{parent}/{file}") })
}

/// Where the manifest of `base`'s `version` is, in its project's folder: beside the version, as "plan@v1.2.manifest.md".
/// `others` are the project's deliverables that have manifests (see [`own_name`]).
pub fn manifest_in_project<'a>(base: &str, version: &str, others: impl IntoIterator<Item = &'a str>) -> String {
    format!("versions/{}/{}@v{version}.manifest.md", attachment_dir(base), own_name(base, others))
}

/// Where the evidence description of `base` is, in its project's folder: beside it, as "plan.desc.md". `others` are the
/// project's live deliverables (see [`own_name`]).
pub fn description_in_project<'a>(base: &str, others: impl IntoIterator<Item = &'a str>) -> String {
    let dir = base.rsplit_once('/').map_or(String::new(), |(dir, _)| format!("{dir}/"));
    format!("deliverables/{dir}{}.desc.md", own_name(base, others))
}

/// An evidence description beside its deliverable ("plan.desc.md"): not a deliverable itself.
pub fn is_description(path: &str) -> bool {
    file_name(path).to_ascii_lowercase().ends_with(".desc.md")
}

/// What a file Ledger keeps about `base` is named after: its name without the extension ("plan"), or with it ("plan.md")
/// when another of `others` has the same name in the same folder ("plan.pdf"), so the two never clash.
fn own_name<'a, 'b>(base: &'a str, others: impl IntoIterator<Item = &'b str>) -> &'a str {
    let stem = attachment_dir(base);
    let shared = others.into_iter().any(|other| other != base && attachment_dir(other) == stem);
    if shared { file_name(base) } else { file_name(&base[..stem.len()]) }
}

// ---- reading ----------------------------------------------------------------------------------------

pub fn exists(conn: &Connection, project: &str, path: &str) -> Result<bool> {
    Ok(query_opt(conn, "SELECT 1 FROM deliverables WHERE project_slug = ?1 AND path = ?2", [project, path], |_| Ok(()))?.is_some())
}

/// How many deliverables the project has (published versions are not counted separately).
pub fn count_live(conn: &Connection, project: &str) -> Result<i64> {
    let paths: Vec<String> = query_all(conn, "SELECT path FROM deliverables WHERE project_slug = ?1", [project], |r| r.get(0))?;
    Ok(paths.iter().filter(|p| split_version(p).1.is_none()).count() as i64)
}

/// Published versions of the deliverable at `base`, oldest first.
pub fn versions_of(conn: &Connection, project: &str, base: &str) -> Result<Vec<String>> {
    let paths: Vec<String> = query_all(conn, "SELECT path FROM deliverables WHERE project_slug = ?1", [project], |r| r.get(0))?;
    let mut versions: Vec<(u32, u32, String)> = paths
        .iter()
        .filter_map(|p| match split_version(p) {
            (b, Some(v)) if b == base => parse_version(&v).map(|(major, minor)| (major, minor, v)),
            _ => None,
        })
        .collect();
    versions.sort();
    Ok(versions.into_iter().map(|(_, _, v)| v).collect())
}

/// The deliverables that are Portflow evidence items. An item is one deliverable: its first file, named after the item;
/// its other files (downloaded into a folder beside it, `Plan.pdf` → `Plan/…`) are its attachments.
#[derive(Default)]
pub struct EvidenceRoles {
    /// Base path of each item's deliverable → the item's name.
    pub names: HashMap<String, String>,
    /// Base path of each item's deliverable → its attachments.
    pub attachments: HashMap<String, Vec<String>>,
    /// Every attachment, to leave out of the list.
    pub folded: std::collections::HashSet<String>,
}

/// The folder an item's attachments sit in: its deliverable's path without the extension.
pub fn attachment_dir(base: &str) -> String {
    match base.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.contains('/') => stem.to_string(),
        _ => base.to_string(),
    }
}

/// Which of an item's files is the item: the one whose name has most of the item's words (`02-workflow-report.pdf` for
/// "Workflow report", not `02-interview.pdf`), the first of those that tie. Named after the item, it always is.
pub fn main_of(item: &str, names: &[&str]) -> usize {
    let words = |text: &str| -> std::collections::HashSet<String> {
        ids::slugify_as(text, "x").split('-').filter(|w| !w.is_empty() && w.parse::<u64>().is_err()).map(str::to_string).collect()
    };
    let wanted = words(item);
    let mut best = (0, 0);
    for (i, name) in names.iter().enumerate() {
        let stem = file_name(name).rsplit_once('.').map_or(file_name(name), |(stem, _)| stem);
        let score = words(stem).intersection(&wanted).count();
        if score > best.1 {
            best = (i, score);
        }
    }
    best.0
}

pub fn evidence_roles(conn: &Connection, project: &str) -> Result<EvidenceRoles> {
    let rows: Vec<(String, String, String)> = query_all(
        conn,
        "SELECT f.evidence_slug, f.path, e.name FROM evidence_files f
         JOIN evidence e ON e.project_slug = f.project_slug AND e.slug = f.evidence_slug
         WHERE f.project_slug = ?1 ORDER BY f.evidence_slug, f.position",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut items: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for (item, path, name) in rows {
        items.entry(item).or_insert_with(|| (name, vec![])).1.push(path);
    }
    let mut roles = EvidenceRoles::default();
    for (_, (name, paths)) in items {
        let main = main_of(&name, &paths.iter().map(String::as_str).collect::<Vec<_>>());
        for (i, path) in paths.iter().enumerate() {
            if i != main {
                roles.attachments.entry(paths[main].clone()).or_default().push(path.clone());
                roles.folded.insert(path.clone());
            }
        }
        roles.names.insert(paths[main].clone(), name);
    }
    // A file continued in another is that other one now: the item's deliverable, or one of its attachments.
    let pairs = continued(conn, project)?;
    for (earlier, later) in &pairs {
        if let Some(name) = roles.names.remove(earlier) {
            let mut files = roles.attachments.remove(earlier).unwrap_or_default();
            files.retain(|f| f != later);
            roles.folded.remove(later);
            roles.folded.insert(earlier.clone());
            if !files.is_empty() {
                roles.attachments.insert(later.clone(), files);
            }
            roles.names.insert(later.clone(), name);
        }
    }
    for (earlier, later) in &pairs {
        for (owner, files) in &mut roles.attachments {
            if let Some(i) = files.iter().position(|f| f == earlier) {
                if owner == later || files.contains(later) {
                    files.remove(i);
                } else {
                    files[i] = later.clone();
                }
                roles.folded.insert(later.clone());
            }
        }
    }
    Ok(roles)
}

// ---- one deliverable, two files ---------------------------------------------------------------------
// A deliverable can go on in another file. A PDF that came from Portflow is continued in a markdown file, so it can be
// edited here: the earlier file keeps its published versions as they are, and the later one is the deliverable from
// then on. It takes the earlier one's place in the list, its evidence item and its feedback, and its version numbers go
// on from the earlier one's (after Portflow's version 6, v6.0, the markdown's first is 6.1 or 7.0). Kept in the
// project's `project.md`:
//
//   continued:
//     Plan.pdf: Plan.md            earlier: later

/// Every file continued in another, as (earlier, later), whose later file is there (live, or published versions only).
pub fn continued(conn: &Connection, project: &str) -> Result<Vec<(String, String)>> {
    let pairs: Vec<(String, String)> =
        query_all(conn, "SELECT earlier, later FROM deliverable_continued WHERE project_slug = ?1 ORDER BY earlier", [project], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if pairs.is_empty() {
        return Ok(pairs);
    }
    let paths: Vec<String> = query_all(conn, "SELECT path FROM deliverables WHERE project_slug = ?1", [project], |r| r.get(0))?;
    let bases: std::collections::HashSet<String> = paths.iter().map(|p| split_version(p).0).collect();
    Ok(pairs.into_iter().filter(|(_, later)| bases.contains(later)).collect())
}

/// The file `earlier` is continued in, if it is (and that file is there).
pub fn later_of(conn: &Connection, project: &str, earlier: &str) -> Result<Option<String>> {
    Ok(continued(conn, project)?.into_iter().find(|(e, _)| e == earlier).map(|(_, later)| later))
}

/// The file `later` continues, if it does.
pub fn earlier_of(conn: &Connection, project: &str, later: &str) -> Result<Option<String>> {
    Ok(continued(conn, project)?.into_iter().find(|(_, l)| l == later).map(|(earlier, _)| earlier))
}

/// Whether either file continues, or is continued in, another already.
fn in_a_pair(conn: &Connection, project: &str, earlier: &str, later: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM deliverable_continued WHERE project_slug = ?1 AND (earlier IN (?2, ?3) OR later IN (?2, ?3)))",
        params![project, earlier, later],
        |r| r.get(0),
    )?)
}

/// Makes `later` (a live file with no versions of its own) the deliverable `earlier` is from now on. The earlier file
/// keeps its versions; its live file goes when it is the same as its newest version (a copy of it), and is refused
/// otherwise, so nothing written in it is hidden. Its facts and evidence description go to `later`, which has none.
pub fn continue_in(conn: &Connection, root: &Path, project: &str, earlier: &str, later: &str) -> Result<()> {
    projects::require(conn, project)?;
    if split_version(earlier).1.is_some() || split_version(later).1.is_some() {
        return Err(Error::invalid("A deliverable is continued, not one of its published versions"));
    }
    if earlier == later {
        return Err(Error::invalid("A file can't continue itself"));
    }
    let versions = versions_of(conn, project, earlier)?;
    let Some(newest) = versions.last() else {
        return Err(Error::invalid(format!("{earlier} has no published versions to go on from")));
    };
    if !exists(conn, project, later)? {
        return Err(Error::not_found(format!("There is no {later} to continue it in")));
    }
    if !versions_of(conn, project, later)?.is_empty() {
        return Err(Error::invalid(format!("{later} has published versions of its own, so it can't continue another file")));
    }
    let taken = in_a_pair(conn, project, earlier, later)?;
    if taken {
        return Err(Error::invalid("One of these files already continues, or is continued in, another"));
    }
    // The earlier live file: a copy of its newest version goes, anything else stays and stops this.
    let mut remove_live = false;
    if exists(conn, project, earlier)? {
        let body: Option<String> = query_opt(conn, "SELECT body FROM deliverables WHERE project_slug = ?1 AND path = ?2", [project, earlier], |r| r.get(0))?.flatten();
        let live_sha = match &body {
            Some(body) => provenance::sha256_hex(body.as_bytes()),
            None => provenance::sha256_hex(&store::at(root).read(&rel_of(project, earlier)).map_err(|e| Error::io(&file_of(root, project, earlier), e))?),
        };
        let newest_sha: Option<String> = query_opt(
            conn,
            "SELECT sha256 FROM version_records WHERE project_slug = ?1 AND deliverable_path = ?2 AND version = ?3",
            params![project, earlier, newest],
            |r| r.get::<_, Option<String>>(0),
        )?
        .flatten();
        if newest_sha.as_deref() != Some(live_sha.as_str()) {
            return Err(Error::invalid(format!(
                "{earlier} was changed since its version {newest}: publish it, or delete the live file, before it goes on in {later}"
            )));
        }
        remove_live = body.is_none();
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM deliverables WHERE project_slug = ?1 AND path = ?2", [project, earlier])?;
    tx.execute("INSERT INTO deliverable_continued (project_slug, earlier, later) VALUES (?1, ?2, ?3)", params![project, earlier, later])?;
    for sql in [
        "UPDATE deliverable_facts SET path = ?3 WHERE project_slug = ?1 AND path = ?2
         AND NOT EXISTS (SELECT 1 FROM deliverable_facts WHERE project_slug = ?1 AND path = ?3)",
        "UPDATE deliverable_descriptions SET path = ?3 WHERE project_slug = ?1 AND path = ?2
         AND NOT EXISTS (SELECT 1 FROM deliverable_descriptions WHERE project_slug = ?1 AND path = ?3)",
    ] {
        tx.execute(sql, params![project, earlier, later])?;
    }
    tx.commit()?;
    if remove_live {
        store::at(root).remove(&rel_of(project, earlier)).map_err(|e| Error::io(&file_of(root, project, earlier), e))?;
    }
    Ok(())
}

/// Continues `earlier` (not markdown) in the markdown file beside it with the same name ("Plan.pdf" in "Plan.md"),
/// written now with the deliverable's name as its heading when it isn't there yet. Returns the markdown's path.
pub fn continue_in_markdown(conn: &Connection, root: &Path, project: &str, earlier: &str) -> Result<String> {
    if is_markdown(earlier) {
        return Err(Error::invalid("This is markdown already: edit it as it is"));
    }
    let later = format!("{}.md", attachment_dir(earlier));
    let created = !exists(conn, project, &later)?;
    if created {
        let (name, _) = evidence_of(conn, project, earlier)?;
        let title = name.unwrap_or_else(|| file_name(&attachment_dir(earlier)).to_string());
        let text = format!("# {title}\n");
        upsert(conn, project, &later, text.len() as i64, &ids::now(), Some(&text))?;
    }
    if let Err(e) = continue_in(conn, root, project, earlier, &later) {
        if created {
            conn.execute("DELETE FROM deliverables WHERE project_slug = ?1 AND path = ?2", [project, &later])?;
        }
        return Err(e);
    }
    Ok(later)
}

/// Takes `later` apart from the file it continues: that one is a deliverable of its own again (with only its versions).
pub fn separate(conn: &Connection, project: &str, later: &str) -> Result<()> {
    let changed = conn.execute("DELETE FROM deliverable_continued WHERE project_slug = ?1 AND later = ?2", [project, later])?;
    if changed == 0 {
        return Err(Error::not_found("This deliverable doesn't continue another file"));
    }
    Ok(())
}

/// What a deliverable has from the file it continues, or which file it is continued in.
/// One timeline: a version of the earlier file (a PDF from Portflow) is a version of the deliverable the later file is,
/// and opens as one; the later file's versions are all of them, its own and the earlier file's, by number.
fn fill_continued(conn: &Connection, project: &str, view: &mut DeliverableView) -> Result<()> {
    if let Some(later) = later_of(conn, project, &view.base_path)? {
        if view.version.is_none() {
            // The earlier file's own page (it has no live file any more): the deliverable is the later one.
            view.later = Some(later);
            return Ok(());
        }
        let whole = get(conn, project, &later)?;
        view.base_path = whole.base_path;
        view.name = whole.name;
        view.evidence = whole.evidence;
        view.attachments = whole.attachments;
        view.versions = whole.versions;
        view.prompts = whole.prompts;
        view.feedback = whole.feedback;
        view.earlier = whole.earlier;
        view.later = None;
        return Ok(());
    }
    let Some(earlier) = earlier_of(conn, project, &view.base_path)? else { return Ok(()) };
    for prompt in prompts::producing(conn, project, &earlier)? {
        if !view.prompts.iter().any(|p| p.id == prompt.id) {
            view.prompts.push(prompt);
        }
    }
    view.feedback.extend(feedback::list_for_deliverable(conn, project, &earlier)?);
    view.feedback.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    // The earlier file's versions that have no text of the later one's at the same number: those open as the earlier file.
    let only_earlier: Vec<String> = versions_of(conn, project, &earlier)?.into_iter().filter(|v| !view.versions.contains(v)).collect();
    view.versions.extend(only_earlier.iter().cloned());
    view.versions.sort_by_key(|v| parse_version(v));
    view.earlier = Some(Earlier { versions: only_earlier, path: earlier });
    Ok(())
}

/// The continued files in the project's `project.md`.
pub fn export_continued(conn: &Connection, project: &str, meta: &mut serde_yaml_ng::Mapping) -> Result<()> {
    let pairs: Vec<(String, String)> =
        query_all(conn, "SELECT earlier, later FROM deliverable_continued WHERE project_slug = ?1 ORDER BY earlier", [project], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if pairs.is_empty() {
        return Ok(());
    }
    let mut map = serde_yaml_ng::Mapping::new();
    for (earlier, later) in pairs {
        map.insert(earlier.into(), later.into());
    }
    frontmatter::set(meta, "continued", serde_yaml_ng::Value::Mapping(map));
    Ok(())
}

/// Reads them leniently: a pair that isn't two names, names a version, or has a file in another pair already is skipped.
/// The files needn't be there yet (they may still be syncing): until the later one is, the earlier one is shown alone.
pub fn import_continued(conn: &Connection, project: &str, meta: &serde_yaml_ng::Mapping) -> Result<()> {
    let Some(serde_yaml_ng::Value::Mapping(map)) = meta.get("continued") else { return Ok(()) };
    for (earlier, later) in map {
        let (Some(earlier), Some(later)) = (earlier.as_str().map(str::trim), later.as_str().map(str::trim)) else { continue };
        if earlier.is_empty() || later.is_empty() || earlier == later || split_version(earlier).1.is_some() || split_version(later).1.is_some() {
            continue;
        }
        let taken = in_a_pair(conn, project, earlier, later)?;
        if !taken {
            conn.execute("INSERT INTO deliverable_continued (project_slug, earlier, later) VALUES (?1, ?2, ?3)", params![project, earlier, later])?;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Group {
    live: Option<(i64, String)>,
    snapshots: Vec<(u32, u32, String, i64, String)>,
}

/// Every deliverable, by name. A deliverable whose live file is gone but which has published versions is still
/// listed (with `exists` false).
pub fn list(conn: &Connection, project: &str) -> Result<Vec<DeliverableSummary>> {
    let rows: Vec<(String, i64, String)> = query_all(
        conn,
        "SELECT path, size, modified FROM deliverables WHERE project_slug = ?1",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for (path, size, modified) in rows {
        let (base, version) = split_version(&path);
        let group = groups.entry(base).or_default();
        match version.as_deref().and_then(|v| parse_version(v).map(|p| (p, v))) {
            Some(((major, minor), v)) => group.snapshots.push((major, minor, v.to_string(), size, modified)),
            None => group.live = Some((size, modified)),
        }
    }

    // The prompts about each deliverable (a prompt is linked to the file, not to its versions).
    let prompt_counts: HashMap<String, i64> = query_all::<(String, i64), _>(
        conn,
        "SELECT d.path, COUNT(DISTINCT d.prompt_id) FROM prompt_deliverables d JOIN prompts p ON p.id = d.prompt_id
         WHERE p.project_slug = ?1 GROUP BY d.path",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?
    .into_iter()
    .collect();
    let mut open_feedback: HashMap<String, i64> = HashMap::new();
    for path in query_all::<String, _>(conn, "SELECT deliverable_path FROM feedback WHERE project_slug = ?1 AND status = 'open'", [project], |r| r.get(0))? {
        *open_feedback.entry(split_version(&path).0).or_default() += 1;
    }

    // The label of the newest version that has one.
    let mut labels: HashMap<String, ((u32, u32), String)> = HashMap::new();
    for (path, version, mmm) in query_all::<(String, String, String), _>(
        conn,
        "SELECT deliverable_path, version, mmm FROM version_records WHERE project_slug = ?1 AND mmm IS NOT NULL",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )? {
        if let Some(number) = parse_version(&version)
            && provenance::level(&mmm).is_some()
            && labels.get(&path).is_none_or(|(known, _)| number > *known)
        {
            labels.insert(path, (number, mmm));
        }
    }

    // When each published version was made, from its record (a file's time changes when it is synced or checked out).
    let made: HashMap<(String, String), String> = query_all::<(String, String, String), _>(
        conn,
        "SELECT deliverable_path, version, created_at FROM version_records WHERE project_slug = ?1",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?
    .into_iter()
    .map(|(path, version, at)| ((path, version), at))
    .collect();
    // The newest version in Portflow of the evidence item each file is in (none while Portflow is off: no evidence then).
    let mut delivered: HashMap<String, Delivered> = HashMap::new();
    for (path, number, at) in query_all::<(String, i64, String), _>(
        conn,
        "SELECT f.path, v.number, v.created_at FROM evidence_files f
         JOIN evidence_versions v ON v.project_slug = f.project_slug AND v.evidence_slug = f.evidence_slug
         WHERE f.project_slug = ?1 AND v.portflow_version IS NOT NULL",
        [project],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )? {
        if delivered.get(&path).is_none_or(|d| number > d.number) {
            delivered.insert(path, Delivered { number, at });
        }
    }

    let mut roles = evidence_roles(conn, project)?;
    // A file continued in another is listed as part of that one: later → (earlier, its group).
    let mut earlier: HashMap<String, (String, Group)> = HashMap::new();
    for (from, to) in continued(conn, project)? {
        if let Some(mut group) = groups.remove(&from) {
            group.snapshots.sort();
            earlier.insert(to, (from, group));
        }
    }
    let mut out: Vec<DeliverableSummary> = groups
        .into_iter()
        .filter(|(base, _)| !roles.folded.contains(base))
        .map(|(base, mut group)| {
            group.snapshots.sort();
            let before = earlier.remove(&base);
            let from = before.as_ref().map(|(path, _)| path.clone()).unwrap_or_default();
            let newest_of = |path: &str, group: &Group| -> Option<String> {
                group.snapshots.last().map(|s| made.get(&(path.to_string(), s.2.clone())).cloned().unwrap_or_else(|| s.4.clone()))
            };
            let latest_at = newest_of(&base, &group).or_else(|| before.as_ref().and_then(|(path, g)| newest_of(path, g)));
            let delivered = match (delivered.remove(&base), delivered.remove(&from)) {
                (Some(a), Some(b)) => Some(if a.number >= b.number { a } else { b }),
                (a, b) => a.or(b),
            };
            let (size, modified) = group
                .live
                .clone()
                .or_else(|| group.snapshots.last().map(|s| (s.3, s.4.clone())))
                .unwrap_or_default();
            let evidence = roles.names.remove(&base);
            DeliverableSummary {
                name: evidence.clone().unwrap_or_else(|| file_name(&base).to_string()),
                attachments: roles.attachments.remove(&base).unwrap_or_default(),
                evidence,
                kind: kind_of(&base).to_string(),
                size,
                modified,
                exists: group.live.is_some(),
                versions: {
                    // One timeline: the file it continues has the versions before (and between) its own.
                    let mut all: Vec<String> = group.snapshots.iter().map(|s| s.2.clone()).collect();
                    if let Some((_, g)) = &before {
                        all.extend(g.snapshots.iter().map(|s| s.2.clone()).filter(|v| !all.contains(v)).collect::<Vec<_>>());
                    }
                    all.sort_by_key(|v| parse_version(v));
                    all
                },
                prompt_count: prompt_counts.get(&base).copied().unwrap_or(0) + prompt_counts.get(&from).copied().unwrap_or(0),
                open_feedback: open_feedback.get(&base).copied().unwrap_or(0) + open_feedback.get(&from).copied().unwrap_or(0),
                mmm: labels.get(&base).or_else(|| labels.get(&from)).map(|(_, level)| level.clone()),
                latest_at,
                delivered,
                earlier: before.map(|(path, g)| Earlier {
                    versions: g.snapshots.iter().map(|s| s.2.clone()).filter(|v| !group.snapshots.iter().any(|own| own.2 == *v)).collect(),
                    path,
                }),
                path: base,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.path.cmp(&b.path)));
    Ok(out)
}

/// A deliverable, or — if `path` is a "<name>@v1.0.<ext>" snapshot — that published version.
pub fn get(conn: &Connection, project: &str, path: &str) -> Result<DeliverableView> {
    let row: Option<(i64, String, Option<String>)> = query_opt(
        conn,
        "SELECT size, modified, body FROM deliverables WHERE project_slug = ?1 AND path = ?2",
        [project, path],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let (base, version) = split_version(path);
    let (exists, size, modified, body) = match row {
        Some((size, modified, body)) => (true, size, modified, body),
        None => {
            // A file whose live copy is gone (or never was here) but that has published versions is still a deliverable.
            if let Some(v) = &version {
                // A version of the deliverable that is only there as the file it continues ("Plan@v2.0.md" is
                // "Plan@v2.0.pdf"): that file, read-only.
                if let Some(earlier) = earlier_of(conn, project, &base)? {
                    let snapshot = snapshot_path(&earlier, v);
                    if exists(conn, project, &snapshot)? {
                        return get(conn, project, &snapshot);
                    }
                }
                return Err(Error::not_found("Deliverable not found"));
            }
            let versions = versions_of(conn, project, &base)?;
            let Some(newest) = versions.last() else { return Err(Error::not_found("Deliverable not found")) };
            let modified: String = conn.query_row(
                "SELECT modified FROM deliverables WHERE project_slug = ?1 AND path = ?2",
                [project, &snapshot_path(&base, newest)],
                |r| r.get(0),
            )?;
            (false, 0, modified, None)
        }
    };

    let html = body.as_ref().map(|text| markdown::render(&frontmatter::parse(text).map_or_else(|| text.clone(), |p| p.body)));
    let (evidence, attachments) = evidence_of(conn, project, &base)?;
    let mut view = DeliverableView {
        path: path.to_string(),
        name: evidence.clone().unwrap_or_else(|| file_name(&base).to_string()),
        evidence,
        attachments,
        kind: kind_of(&base).to_string(),
        editable: exists && version.is_none() && body.is_some(),
        versions: versions_of(conn, project, &base)?,
        exists,
        prompts: prompts::producing(conn, project, &base)?,
        feedback: feedback::list_for_deliverable(conn, project, &base)?,
        base_path: base,
        version,
        size,
        modified,
        body,
        html,
        earlier: None,
        later: None,
    };
    fill_continued(conn, project, &mut view)?;
    Ok(view)
}

/// The evidence item a deliverable is (its name) and the item's attachments.
fn evidence_of(conn: &Connection, project: &str, base: &str) -> Result<(Option<String>, Vec<String>)> {
    let mut roles = evidence_roles(conn, project)?;
    Ok((roles.names.remove(base), roles.attachments.remove(base).unwrap_or_default()))
}

// ---- writing ----------------------------------------------------------------------------------------

/// Renames a deliverable: its live file and every published version (on disk too, for a file that isn't markdown), and
/// everything that points at it — its facts, prompts, runs, posts, feedback, version records, Portflow evidence and
/// the [[links]] written to it.
/// Refused when `to` is already a deliverable.
pub fn rename_base(conn: &Connection, root: &Path, project: &str, from: &str, to: &str) -> Result<()> {
    if from == to {
        return Ok(());
    }
    if exists(conn, project, to)? || !versions_of(conn, project, to)?.is_empty() {
        return Err(Error::invalid(format!("There is already a deliverable called {to}")));
    }
    let rows: Vec<(String, bool)> =
        query_all(conn, "SELECT path, body IS NULL FROM deliverables WHERE project_slug = ?1", [project], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let moves: Vec<(String, String, bool)> = rows
        .into_iter()
        .filter_map(|(path, binary)| match split_version(&path) {
            (base, None) if base == from => Some((path, to.to_string(), binary)),
            (base, Some(v)) if base == from => Some((path.clone(), snapshot_path(to, &v), binary)),
            _ => None,
        })
        .collect();
    if moves.is_empty() {
        return Err(Error::not_found("Deliverable not found"));
    }
    // The [[links]] to it follow (see the end).
    let before = crate::links::list(conn)?;
    for (old, new, binary) in &moves {
        if *binary {
            // A folder left empty by the move goes too (the store prunes it).
            let store = store::at(root);
            let (a, b) = (rel_of(project, old), rel_of(project, new));
            if store.exists(&a) {
                store.rename(&a, &b).map_err(|e| Error::io(&root.join(&b), e))?;
            }
        }
        conn.execute("UPDATE deliverables SET path = ?3 WHERE project_slug = ?1 AND path = ?2", params![project, old, new])?;
        // What points at the file or at one of its versions.
        for sql in [
            "UPDATE feedback SET deliverable_path = ?3 WHERE project_slug = ?1 AND deliverable_path = ?2",
            "UPDATE evidence_version_files SET path = ?3 WHERE project_slug = ?1 AND path = ?2",
            "UPDATE claude_run_files SET path = ?3 WHERE path = ?2 AND run_id IN (SELECT id FROM claude_runs WHERE project_slug = ?1)",
        ] {
            conn.execute(sql, params![project, old, new])?;
        }
    }
    for sql in [
        "UPDATE deliverable_facts SET path = ?3 WHERE project_slug = ?1 AND path = ?2",
        "UPDATE deliverable_descriptions SET path = ?3 WHERE project_slug = ?1 AND path = ?2",
        "UPDATE version_records SET deliverable_path = ?3 WHERE project_slug = ?1 AND deliverable_path = ?2",
        "UPDATE posts SET deliverable_path = ?3 WHERE project_slug = ?1 AND deliverable_path = ?2",
        "UPDATE evidence_files SET path = ?3 WHERE project_slug = ?1 AND path = ?2",
        "UPDATE deliverable_continued SET earlier = ?3 WHERE project_slug = ?1 AND earlier = ?2",
        "UPDATE deliverable_continued SET later = ?3 WHERE project_slug = ?1 AND later = ?2",
        "UPDATE feedback SET deliverable_path = ?3 WHERE project_slug = ?1 AND deliverable_path = ?2",
        "UPDATE prompt_deliverables SET path = ?3 WHERE path = ?2 AND prompt_id IN (SELECT id FROM prompts WHERE project_slug = ?1)",
        "UPDATE claude_run_files SET path = ?3 WHERE path = ?2 AND run_id IN (SELECT id FROM claude_runs WHERE project_slug = ?1)",
    ] {
        conn.execute(sql, params![project, from, to])?;
    }
    crate::links::follow_rename(conn, &before, "deliverable", project, from, to)
}

/// Gives the deliverable at `path` (its live file, not a version) another name, as typed: the file keeps its folder and
/// its extension (typing it again is fine), and its versions, feedback and everything else that points at it follow
/// (`rename_base`). Returns the new path.
pub fn rename(conn: &Connection, root: &Path, project: &str, path: &str, name: &str) -> Result<String> {
    if split_version(path).1.is_some() {
        return Err(Error::invalid("A published version keeps its name; rename the deliverable itself"));
    }
    let (dir, file) = path.rsplit_once('/').map_or(("", path), |(d, f)| (d, f));
    let ext = file.rsplit_once('.').filter(|(stem, _)| !stem.is_empty()).map(|(_, ext)| ext);
    let mut name = name.trim();
    if let Some(ext) = ext {
        let dotted = format!(".{ext}");
        if name.len() > dotted.len() && name[name.len() - dotted.len()..].eq_ignore_ascii_case(&dotted) {
            name = name[..name.len() - dotted.len()].trim_end();
        }
    }
    if name.is_empty() {
        return Err(Error::invalid("Name is required"));
    }
    let clean = crate::uploads::sanitize(name);
    let file = ext.map_or(clean.clone(), |ext| format!("{clean}.{ext}"));
    let to = if dir.is_empty() { file } else { format!("{dir}/{file}") };
    if is_description(&to) {
        return Err(Error::invalid("A name that ends in .desc.md is an evidence description's: choose another"));
    }
    rename_base(conn, root, project, path, &to)?;
    Ok(to)
}

/// Records a file found in the deliverables folder (or written there by this app).
pub fn upsert(conn: &Connection, project: &str, path: &str, size: i64, modified: &str, body: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO deliverables (project_slug, path, size, modified, body) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![project, path, size, modified, body],
    )?;
    Ok(())
}

/// A new markdown deliverable, not yet produced by any prompt (link it from a prompt, or just write it).
pub fn create(conn: &Connection, project: &str, name: &str, body: &str) -> Result<DeliverableView> {
    projects::require(conn, project)?;
    // "plan.md" means the markdown file "plan", not one called "plan-md".
    let name = name.trim();
    let name = name.strip_suffix(".md").or_else(|| name.strip_suffix(".MD")).unwrap_or(name).trim();
    if name.is_empty() {
        return Err(Error::invalid("Name is required"));
    }
    let stem = ids::slugify_as(name, "deliverable");
    let mut path = format!("{stem}.md");
    let mut n = 2;
    while exists(conn, project, &path)? {
        path = format!("{stem}-{n}.md");
        n += 1;
    }
    let mut text = if body.trim().is_empty() { format!("# {name}\n") } else { body.to_string() };
    if !text.ends_with('\n') {
        text.push('\n');
    }
    upsert(conn, project, &path, text.len() as i64, &ids::now(), Some(&text))?;
    get(conn, project, &path)
}

/// Saves the text of a markdown deliverable. Published versions can't be edited.
pub fn save(conn: &Connection, project: &str, path: &str, body: &str) -> Result<DeliverableView> {
    let view = get(conn, project, path)?;
    if !view.editable {
        return Err(Error::invalid("Only the current version of a markdown deliverable can be edited"));
    }
    conn.execute(
        "UPDATE deliverables SET body = ?3, size = ?4, modified = ?5 WHERE project_slug = ?1 AND path = ?2",
        params![project, path, body, body.len() as i64, ids::now()],
    )?;
    get(conn, project, path)
}

/// Freezes the current file as the next version, as a person typed it, and notes it in the thread the version is filed
/// under (see [`provenance::default_thread`]). The page uses [`publish_with`]; this is the publish with every default.
#[cfg(test)]
pub fn publish(conn: &Connection, root: &Path, project: &str, path: &str, bump: Bump, explicit: Option<&str>) -> Result<DeliverableView> {
    publish_with(conn, root, project, path, bump, explicit, &PublishChoices::default())
}

/// [`publish`], with what the person said about the change: the thread, whether AI made it, its AI-use label.
pub fn publish_with(
    conn: &Connection,
    root: &Path,
    project: &str,
    path: &str,
    bump: Bump,
    explicit: Option<&str>,
    choices: &PublishChoices,
) -> Result<DeliverableView> {
    let (base, version) = split_version(path);
    if version.is_some() {
        return Err(Error::invalid("Publish the current version, not a published one"));
    }
    let origin = provenance::manual_origin(conn, project, &base, choices)?;
    freeze(conn, root, project, &base, bump, explicit, None, &origin)?;
    get(conn, project, &base)
}

/// What a run of a prompt publishes: the next minor version, noted in the prompt's thread. With `held` it is that
/// content (what the file said before the run changed it) rather than the file as it is now. Returns the version,
/// like "0.2"; the snapshot is `snapshot_path(base, version)`.
pub fn publish_run(conn: &Connection, root: &Path, project: &str, base: &str, held: Option<&[u8]>, origin: &Origin) -> Result<String> {
    freeze(conn, root, project, base, Bump::Minor, None, held, origin)
}

/// Writes the snapshot of the live file at `base` (or of `held`, when given) as the next version, records how it came to
/// be, and when the origin has a thread notes it there. Returns the version.
#[allow(clippy::too_many_arguments, reason = "what a version is made from and where it is written: a struct for one call would only name them again")]
fn freeze(
    conn: &Connection,
    root: &Path,
    project: &str,
    base: &str,
    bump: Bump,
    explicit: Option<&str>,
    held: Option<&[u8]>,
    origin: &Origin,
) -> Result<String> {
    let live = get(conn, project, base)?;

    // Of a file that continues another, these are that one's versions too: the numbers go on from them.
    let existing = live.versions.clone();
    let version = match explicit.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => {
            let (major, minor) = parse_version(v).ok_or_else(|| Error::invalid("Version must be in M.m format, e.g. 1.4"))?;
            format!("{major}.{minor}")
        }
        // Bump from the highest published version, so a custom version earlier on never collides.
        None => bump_version(existing.last().map(String::as_str), bump),
    };
    if existing.contains(&version) {
        return Err(Error::invalid(format!("Version {version} is already published")));
    }

    let snapshot = snapshot_path(base, &version);
    let at = ids::now();
    let tx = conn.unchecked_transaction()?;
    let sha256 = match &live.body {
        Some(body) => {
            let held_text = held.map(|bytes| String::from_utf8_lossy(bytes).into_owned());
            let text = held_text.as_deref().unwrap_or(body);
            upsert(&tx, project, &snapshot, text.len() as i64, &at, Some(text))?;
            provenance::sha256_hex(text.as_bytes())
        }
        None => {
            let store = store::at(root);
            let to = rel_of(project, &snapshot);
            let bytes = match held {
                Some(bytes) => bytes.to_vec(),
                None => store.read(&rel_of(project, base)).map_err(|e| Error::io(&file_of(root, project, base), e))?,
            };
            store.write(&to, &bytes).map_err(|e| Error::io(&root.join(&to), e))?;
            upsert(&tx, project, &snapshot, bytes.len() as i64, &at, None)?;
            provenance::sha256_hex(&bytes)
        }
    };
    if let Some(thread) = &origin.thread {
        posts::add(
            &tx,
            project,
            thread,
            "",
            AddOptions {
                version_published: true,
                deliverable_path: Some(base.to_string()),
                deliverable_version: Some(version.clone()),
                prompt_id: origin.prompt.clone(),
                ..Default::default()
            },
        )?;
    }
    provenance::insert_record(&tx, project, base, &version, origin, &sha256, &at)?;
    tx.commit()?;
    Ok(version)
}

/// Deletes the current file; its published versions stay. A markdown file leaves with the next write-through; any
/// other kind is removed from the folder here.
pub fn delete(conn: &Connection, root: &Path, project: &str, path: &str) -> Result<()> {
    let view = get(conn, project, path)?;
    if view.version.is_some() {
        return Err(Error::invalid("Published versions are kept"));
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM deliverables WHERE project_slug = ?1 AND path = ?2", [project, path])?;
    crate::deliverable_facts::forget(&tx, project, path)?;
    // Gone for good (no versions of its own): the file it continued is a deliverable of its own again.
    if view.versions.is_empty() {
        tx.execute("DELETE FROM deliverable_continued WHERE project_slug = ?1 AND later = ?2", [project, path])?;
    }
    tx.execute(
        "DELETE FROM prompt_deliverables WHERE path = ?2 AND prompt_id IN (SELECT id FROM prompts WHERE project_slug = ?1)",
        [project, path],
    )?;
    tx.commit()?;
    if view.body.is_none() {
        store::at(root).remove(&rel_of(project, path)).map_err(|e| Error::io(&file_of(root, project, path), e))?;
    }
    Ok(())
}
