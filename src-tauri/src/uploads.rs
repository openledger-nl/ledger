//! Files uploaded into a thread. The file itself goes in the project's `attachments/` folder (whatever the
//! upload was: a file picked in the thread, or a screenshot); the thread gets a manifest — when it was uploaded,
//! where the file is, and a description.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, Row, params};

use crate::db::{query_all, query_opt};
use crate::error::{Error, Result};
use crate::models::UploadView;
use crate::{ids, markdown, store, threads};

const COLUMNS: &str = "id, thread_slug, file, name, size, uploaded_at, description";
const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "jxl"];

pub fn project_dir(root: &Path, project: &str) -> PathBuf {
    root.join("projects").join(project)
}

/// A file in the project's folder (`attachments/a.png`), by its path in the vault.
pub fn project_rel(project: &str, file: &str) -> String {
    format!("projects/{project}/{file}")
}

pub fn is_image(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

fn view(root: &Path, project: &str) -> impl Fn(&Row<'_>) -> rusqlite::Result<UploadView> {
    let store = store::at(root);
    let project = project.to_string();
    move |r| {
        let (file, name, description): (String, String, String) = (r.get(2)?, r.get(3)?, r.get(6)?);
        Ok(UploadView {
            id: r.get(0)?,
            thread_slug: r.get(1)?,
            image: is_image(&name),
            exists: store.exists(&project_rel(&project, &file)),
            file,
            name,
            size: r.get(4)?,
            uploaded_at: r.get(5)?,
            description_html: markdown::render(&description),
            description,
        })
    }
}

/// Oldest first.
pub fn list(conn: &Connection, root: &Path, project: &str, thread: &str) -> Result<Vec<UploadView>> {
    query_all(
        conn,
        &format!("SELECT {COLUMNS} FROM uploads WHERE project_slug = ?1 AND thread_slug = ?2 ORDER BY uploaded_at, rowid"),
        [project, thread],
        view(root, project),
    )
}

/// Every upload in the project, from all its threads, newest first, and then every other file in its `attachments/`
/// folder (one put there by hand, say a PDF someone sent), newest first: what its Attachments tab lists. Such a file
/// has no thread (`thread_slug` is empty) and goes by its path (`id` is `file:<path>`).
pub fn list_project(conn: &Connection, root: &Path, project: &str) -> Result<Vec<UploadView>> {
    let mut found = query_all(
        conn,
        &format!("SELECT {COLUMNS} FROM uploads WHERE project_slug = ?1 ORDER BY uploaded_at DESC, rowid DESC"),
        [project],
        view(root, project),
    )?;
    let recorded: std::collections::HashSet<String> = found.iter().map(|u| u.file.clone()).collect();
    let mut loose: Vec<UploadView> = folder_files(root, project)
        .into_iter()
        .filter(|(file, _, _)| !recorded.contains(file))
        .map(|(file, size, modified)| {
            let name = file.rsplit('/').next().unwrap_or(&file).to_string();
            UploadView {
                id: format!("file:{file}"),
                thread_slug: String::new(),
                image: is_image(&name),
                exists: true,
                file,
                name,
                size,
                uploaded_at: modified,
                description: String::new(),
                description_html: String::new(),
            }
        })
        .collect();
    loose.sort_by(|a, b| b.uploaded_at.cmp(&a.uploaded_at));
    found.extend(loose);
    Ok(found)
}

/// Every file in the project's `attachments/` folder, subfolders included: its path in the project
/// (`attachments/hans/brief.pdf`), its size and when it last changed.
pub fn folder_files(root: &Path, project: &str) -> Vec<(String, i64, String)> {
    let prefix = format!("projects/{project}/");
    store::at(root)
        .list_under(&project_rel(project, ATTACHMENTS))
        .into_iter()
        .filter_map(|(rel, (size, nanos))| {
            let file = rel.strip_prefix(&prefix)?.to_string();
            Some((file, size as i64, ids::format_millis((nanos / 1_000_000) as i64)))
        })
        .collect()
}

/// The folder in a project that its attachments are in.
pub const ATTACHMENTS: &str = "attachments";

fn get(conn: &Connection, root: &Path, project: &str, thread: &str, id: &str) -> Result<UploadView> {
    query_opt(
        conn,
        &format!("SELECT {COLUMNS} FROM uploads WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3"),
        [project, thread, id],
        view(root, project),
    )?
    .ok_or_else(|| Error::not_found("Upload not found"))
}

/// Copies files from anywhere on this machine into the project's attachments and records each in the thread.
pub fn add_files(conn: &Connection, root: &Path, project: &str, thread: &str, sources: &[PathBuf], description: &str) -> Result<Vec<UploadView>> {
    threads::require(conn, project, thread)?;
    let mut added = vec![];
    for source in sources {
        if !source.is_file() {
            return Err(Error::invalid(format!("{} isn't a file", source.display())));
        }
        let name = source.file_name().and_then(|n| n.to_str()).unwrap_or("file");
        let file = place(root, project, name);
        let rel = project_rel(project, &file);
        let size = store::at(root).import_file(source, &rel).map_err(|e| Error::io(&root.join(&rel), e))?;
        added.push(insert(conn, root, project, thread, &file, size, description)?);
    }
    Ok(added)
}

/// Saves bytes (a screenshot, say) as a new attachment and records it in the thread.
pub fn add_bytes(conn: &Connection, root: &Path, project: &str, thread: &str, name: &str, bytes: &[u8], description: &str) -> Result<UploadView> {
    threads::require(conn, project, thread)?;
    let file = place(root, project, name);
    let rel = project_rel(project, &file);
    store::at(root).write(&rel, bytes).map_err(|e| Error::io(&root.join(&rel), e))?;
    insert(conn, root, project, thread, &file, bytes.len() as u64, description)
}

pub fn set_description(conn: &Connection, root: &Path, project: &str, thread: &str, id: &str, description: &str) -> Result<UploadView> {
    let changed = conn.execute(
        "UPDATE uploads SET description = ?4 WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3",
        params![project, thread, id, description.trim()],
    )?;
    if changed == 0 {
        return Err(Error::not_found("Upload not found"));
    }
    get(conn, root, project, thread, id)
}

/// Removes the manifest only: the file stays in the project's `attachments/`. Ledger never deletes a file you put
/// there; take it out yourself (in a file browser or Obsidian) when you no longer want it.
pub fn delete(conn: &Connection, project: &str, thread: &str, id: &str) -> Result<()> {
    conn.execute("DELETE FROM uploads WHERE project_slug = ?1 AND thread_slug = ?2 AND id = ?3", [project, thread, id])?;
    Ok(())
}

fn insert(conn: &Connection, root: &Path, project: &str, thread: &str, file: &str, size: u64, description: &str) -> Result<UploadView> {
    let ids::Stamp { id, at } = ids::new_stamp();
    let name = file.rsplit('/').next().unwrap_or(file);
    conn.execute(
        "INSERT INTO uploads (id, project_slug, thread_slug, file, name, size, uploaded_at, description)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, project, thread, file, name, size as i64, at, description.trim()],
    )?;
    get(conn, root, project, thread, &id)
}

/// A free spot in the project's attachments, as recorded ("attachments/<name>"). A name that is taken gets a number.
fn place(root: &Path, project: &str, name: &str) -> String {
    let store = store::at(root);

    let clean = sanitize(name);
    let (stem, ext) = match clean.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (clean.clone(), String::new()),
    };
    let mut candidate = clean;
    let mut n = 2;
    while store.exists(&project_rel(project, &format!("attachments/{candidate}"))) {
        candidate = format!("{stem}-{n}{ext}");
        n += 1;
    }
    format!("attachments/{candidate}")
}

/// A file name that is safe on every platform.
pub(crate) fn sanitize(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_matches('.').chars().take(120).collect::<String>();
    if cleaned.is_empty() { "file".to_string() } else { cleaned }
}
