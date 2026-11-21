//! The Tauri command surface: thin wrappers over the stores. Reads are answered from the in-memory database;
//! everything that changes something goes through `App::mutate`, which writes it through to the vault's files.
//! Argument names are snake_case here and camelCase from the frontend.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::github::GithubRemote;
use crate::history;
use crate::keystore::{self, EncryptionStatus, KeyStore};
use crate::models::*;
use crate::branding::{self, Branding, Side};
use crate::deliverable_facts::{self, DeliverableMeta};
use crate::project_facts::{self, ProjectMeta, ProjectPerson};
use crate::posts::AddOptions;
use crate::screenshot::{self, Region, ScreenshotSettings};
use crate::settings::{self, SyncConfig, SyncEnv};
use crate::state::App;
use crate::syncer::SyncStatus;
use crate::{activity, board, semesters, deliverables, feedback, links, markdown, posts, projects, prompts, provenance, search, stories, templates, threads, uploads};

#[tauri::command]
pub fn render_markdown(body: String) -> String {
    markdown::render(&body)
}

/// What a `[[link]]` in this project's markdown can point at.
#[tauri::command]
pub fn list_links(app: State<App>, project: String) -> Result<Vec<LinkTarget>> {
    app.read(|c, _| links::list(c, &project))
}

#[tauri::command]
pub fn search(app: State<App>, query: String) -> Result<Vec<SearchHit>> {
    app.read(|c, _| search::search(c, &query))
}

/// Every semester with its sprints, the earliest first.
#[tauri::command]
pub fn list_semesters(app: State<App>) -> Result<Vec<SemesterView>> {
    app.read(|c, _| semesters::list(c))
}

#[tauri::command]
pub fn create_semester(app: State<App>, draft: SemesterDraft) -> Result<SemesterView> {
    app.mutate(|c, _| semesters::create(c, &draft))
}

#[tauri::command]
pub fn update_semester(app: State<App>, id: String, draft: SemesterDraft) -> Result<SemesterView> {
    app.mutate(|c, _| semesters::update(c, &id, &draft))
}

#[tauri::command]
pub fn delete_semester(app: State<App>, id: String) -> Result<()> {
    app.mutate(|c, _| semesters::delete(c, &id))
}

/// Everything made in the vault and when, oldest first: the contribution graph's events.
#[tauri::command]
pub fn list_activity(app: State<App>) -> Result<Vec<ActivityEvent>> {
    app.read(|c, _| activity::list(c))
}

// ---- projects ---------------------------------------------------------------------------------------

/// Every project with its threads, for the sidebar.
#[tauri::command]
pub fn list_projects(app: State<App>) -> Result<Vec<ProjectSummary>> {
    app.read(|c, _| projects::list(c))
}

#[tauri::command]
pub fn create_project(app: State<App>, name: String) -> Result<ProjectSummary> {
    app.mutate(|c, _| projects::create(c, &name))
}

#[tauri::command]
pub fn rename_project(app: State<App>, slug: String, name: String) -> Result<ProjectSummary> {
    app.mutate(|c, _| projects::rename(c, &slug, &name))
}

#[tauri::command]
pub fn get_project(app: State<App>, slug: String) -> Result<ProjectView> {
    app.read(|c, _| projects::get(c, &slug))
}

#[tauri::command]
pub fn save_readme(app: State<App>, syncer: State<crate::portflow::PortflowSyncer>, slug: String, text: String) -> Result<ProjectView> {
    let saved = app.mutate(|c, _| projects::save_readme(c, &slug, &text))?;
    // A guide that goes to Portflow (that write switched on) follows the save; editing never waits for Portflow.
    crate::portflow::guide_saved(&app, &syncer);
    Ok(saved)
}

/// What a post is (a thought, devlog, research or reflection) and how sure you are of it.
#[tauri::command]
pub fn set_post_meta(app: State<App>, project: String, thread: String, id: String, kind: Option<String>, labels: Vec<String>) -> Result<PostView> {
    app.mutate(|c, _| posts::set_meta(c, &project, &thread, &id, kind.as_deref(), &labels))
}

/// Keeps with a published version that it got another label than the project's intended one, and why.
#[tauri::command]
pub fn confirm_label(app: State<App>, project: String, path: String, version: String, intended: String, reason: String) -> Result<()> {
    app.mutate(|c, _| provenance::confirm_label(c, &project, &path, &version, &intended, &reason))
}

/// A deliverable's number, goal, own label, question and template; `None` for one made without them.
#[tauri::command]
pub fn get_deliverable_meta(app: State<App>, project: String, path: String) -> Result<Option<DeliverableMeta>> {
    app.read(|c, _| deliverable_facts::get(c, &project, &path))
}

#[tauri::command]
pub fn list_deliverable_meta(app: State<App>, project: String) -> Result<Vec<DeliverableMeta>> {
    app.read(|c, _| deliverable_facts::list(c, &project))
}

#[tauri::command]
pub fn save_deliverable_meta(app: State<App>, meta: DeliverableMeta) -> Result<DeliverableMeta> {
    app.mutate(|c, _| deliverable_facts::save(c, &meta))
}

/// A deliverable's evidence description (What? / Goal? / Why it contributes?); empty when it has none.
#[tauri::command]
pub fn get_deliverable_description(app: State<App>, project: String, path: String) -> Result<deliverable_facts::DescriptionView> {
    app.read(|c, _| deliverable_facts::description(c, &project, &path))
}

/// Sets it: its three parts, or `text` as it is; empty takes it away.
#[tauri::command]
pub fn save_deliverable_description(
    app: State<App>,
    project: String,
    path: String,
    parts: Option<crate::portflow_evidence_text::Parts>,
    text: Option<String>,
) -> Result<deliverable_facts::DescriptionView> {
    let text = match parts {
        Some(parts) if [&parts.what, &parts.goal, &parts.why].iter().all(|t| t.trim().is_empty()) => String::new(),
        Some(parts) => parts.to_markdown(),
        None => text.unwrap_or_default(),
    };
    app.mutate(|c, _| deliverable_facts::save_description(c, &project, &path, &text))
}

/// The number the project's next deliverable gets.
#[tauri::command]
pub fn next_deliverable_number(app: State<App>, project: String) -> Result<i64> {
    app.read(|c, _| deliverable_facts::next_number(c, &project))
}

/// A project's kind, goal and end; `None` while it has none.
#[tauri::command]
pub fn get_project_meta(app: State<App>, slug: String) -> Result<Option<ProjectMeta>> {
    app.read(|c, _| project_facts::get(c, &slug))
}

#[tauri::command]
pub fn list_project_meta(app: State<App>) -> Result<Vec<ProjectMeta>> {
    app.read(|c, _| project_facts::list(c))
}

/// The people of a project (its coach, teammates, client…), or of every project without one.
#[tauri::command]
pub fn list_project_people(app: State<App>, slug: Option<String>) -> Result<Vec<ProjectPerson>> {
    app.read(|c, _| project_facts::people(c, slug.as_deref()))
}

#[tauri::command]
pub fn save_project_people(app: State<App>, slug: String, people: Vec<ProjectPerson>) -> Result<Vec<ProjectPerson>> {
    app.mutate(|c, _| project_facts::save_people(c, &slug, &people))
}

/// How the project's exported PDFs look and who they name.
#[tauri::command]
pub fn get_project_branding(app: State<App>, slug: String) -> Result<Branding> {
    app.read(|c, _| branding::get(c, &slug))
}

/// Sets the names and the colour of the project's PDFs (the logos are set with `set_project_logo`).
#[tauri::command]
pub fn save_project_branding(app: State<App>, slug: String, branding: Branding) -> Result<Branding> {
    app.mutate(|c, _| branding::save(c, &slug, &branding))
}

/// Copies the picture at `source` into the project as the logo of `side` ("own" or "client"); without `source` the
/// logo is removed.
#[tauri::command]
pub fn set_project_logo(app: State<App>, slug: String, side: Side, source: Option<String>) -> Result<Branding> {
    app.mutate(|c, root| branding::set_logo(c, root, &slug, side, source.as_deref().map(Path::new)))
}

/// Takes your side of the branding (your name, your own name and logo, the colour) from another project.
#[tauri::command]
pub fn copy_project_branding(app: State<App>, slug: String, from: String) -> Result<Branding> {
    app.mutate(|c, root| branding::copy_own(c, root, &slug, &from))
}

#[tauri::command]
pub fn save_project_meta(app: State<App>, slug: String, meta: ProjectMeta) -> Result<ProjectMeta> {
    app.mutate(|c, _| project_facts::save(c, &slug, &meta))
}

/// Deletes the project and its whole folder, attachments and non-markdown deliverables included.
#[tauri::command]
pub fn delete_project(app: State<App>, slug: String) -> Result<()> {
    {
        let mut guard = app.lock();
        let core = &mut *guard;
        safe_segment(&slug)?;
        projects::delete(&core.conn, &slug)?;
        core.vault.persist(&core.conn)?;
        core.vault.remove_project_dir(&slug)?;
    }
    app.syncer.dirty();
    Ok(())
}

// ---- threads ----------------------------------------------------------------------------------------

#[tauri::command]
pub fn create_thread(app: State<App>, project: String, title: String) -> Result<ThreadSummary> {
    app.mutate(|c, _| threads::create(c, &project, &title))
}

#[tauri::command]
pub fn rename_thread(app: State<App>, project: String, slug: String, title: String) -> Result<ThreadSummary> {
    app.mutate(|c, _| threads::rename(c, &project, &slug, &title))
}

#[tauri::command]
pub fn get_thread(app: State<App>, project: String, slug: String) -> Result<ThreadView> {
    app.read(|c, root| threads::get(c, root, &project, &slug))
}

#[tauri::command]
pub fn delete_thread(app: State<App>, project: String, slug: String) -> Result<()> {
    app.mutate(|c, root| threads::delete(c, root, &project, &slug))
}

#[tauri::command]
pub fn link_threads(app: State<App>, project: String, slug: String, other_slug: String) -> Result<()> {
    app.mutate(|c, _| threads::link(c, &project, &slug, &other_slug))
}

#[tauri::command]
pub fn unlink_threads(app: State<App>, project: String, slug: String, other_slug: String) -> Result<()> {
    app.mutate(|c, _| threads::unlink(c, &project, &slug, &other_slug))
}

// ---- posts ------------------------------------------------------------------------------------------

#[tauri::command]
pub fn add_post(app: State<App>, project: String, thread: String, body: String, reply_to_id: Option<String>) -> Result<PostView> {
    app.mutate(|c, _| posts::add(c, &project, &thread, &body, AddOptions { reply_to_id, ..Default::default() }))
}

#[tauri::command]
pub fn update_post(app: State<App>, project: String, thread: String, id: String, body: String) -> Result<PostView> {
    app.mutate(|c, _| posts::update(c, &project, &thread, &id, &body))
}

#[tauri::command]
pub fn delete_post(app: State<App>, project: String, thread: String, id: String) -> Result<()> {
    app.mutate(|c, _| posts::delete(c, &project, &thread, &id))
}

// ---- prompts ----------------------------------------------------------------------------------------

/// A new prompt, with what it can come with: an existing deliverable it asks changes to, or the name of a new one (blank:
/// the AI names it), and the feedback it is meant to resolve. It has no title: Claude Code names it, in the background.
#[tauri::command]
pub fn create_prompt(handle: AppHandle, app: State<App>, project: String, thread: String, draft: NewPrompt) -> Result<PromptView> {
    let prompt = app.mutate(|c, _| prompts::create(c, &project, &thread, &draft))?;
    crate::claude_code::name_prompt_later(&handle, &project, &thread, &prompt.id);
    Ok(prompt)
}

/// What a new prompt's base text starts as, from the template or the feedback it is made from.
#[tauri::command]
pub fn suggest_prompt_base(app: State<App>, project: String, spec: BaseSpec) -> Result<String> {
    app.read(|c, _| prompts::suggest_base(c, &project, &spec))
}

#[tauri::command]
pub fn update_prompt(app: State<App>, project: String, thread: String, id: String, base: Option<String>, body: Option<String>) -> Result<PromptView> {
    app.mutate(|c, _| prompts::update(c, &project, &thread, &id, base.as_deref(), body.as_deref()))
}

#[tauri::command]
pub fn delete_prompt(app: State<App>, project: String, thread: String, id: String) -> Result<()> {
    app.mutate(|c, _| prompts::delete(c, &project, &thread, &id))
}

#[tauri::command]
pub fn link_prompt_deliverable(app: State<App>, project: String, thread: String, id: String, path: String) -> Result<PromptView> {
    app.mutate(|c, _| prompts::link_deliverable(c, &project, &thread, &id, &path))
}

#[tauri::command]
pub fn unlink_prompt_deliverable(app: State<App>, project: String, thread: String, id: String, path: String) -> Result<PromptView> {
    app.mutate(|c, _| prompts::unlink_deliverable(c, &project, &thread, &id, &path))
}

/// The prompts about a deliverable (given by its live path), as the deliverable's own page shows them.
#[tauri::command]
pub fn list_deliverable_prompts(app: State<App>, project: String, path: String) -> Result<Vec<PromptView>> {
    app.read(|c, _| prompts::linked_to(c, &project, &path))
}

/// Adds pieces of feedback to a prompt: they are linked, and put in its base text with the version each was given on.
#[tauri::command]
pub fn attach_prompt_feedback(app: State<App>, project: String, thread: String, id: String, feedback: Vec<String>) -> Result<PromptView> {
    app.mutate(|c, _| prompts::attach_feedback(c, &project, &thread, &id, &feedback))
}

#[tauri::command]
pub fn detach_prompt_feedback(app: State<App>, project: String, thread: String, id: String, feedback: String) -> Result<PromptView> {
    app.mutate(|c, _| prompts::detach_feedback(c, &project, &thread, &id, &feedback))
}

/// Starts a deliverable for the prompt: a new markdown file (from the prompt's template, if it has one) that the
/// prompt is linked to.
#[tauri::command]
pub fn create_prompt_deliverable(app: State<App>, project: String, thread: String, id: String, name: String) -> Result<PromptView> {
    app.mutate(|c, _| {
        let prompt = prompts::get(c, &project, &thread, &id)?;
        let skeleton = prompt.template_id.as_deref().and_then(|t| templates::skeleton_for(c, &project, t).ok()).unwrap_or_default();
        let created = deliverables::create(c, &project, &name, &skeleton)?;
        prompts::link_deliverable(c, &project, &thread, &id, &created.path)
    })
}

// ---- feedback ---------------------------------------------------------------------------------------

#[tauri::command]
pub fn add_feedback(
    app: State<App>,
    project: String,
    thread: String,
    deliverable_path: String,
    body: String,
    quote: Option<Quote>,
    from: Option<FeedbackFrom>,
) -> Result<FeedbackView> {
    app.mutate(|c, _| feedback::add_with(c, &project, &thread, &deliverable_path, &body, quote.as_ref(), &from.unwrap_or_default()))
}

/// Every piece of feedback in the vault, newest first: the inbox.
#[tauri::command]
pub fn list_all_feedback(app: State<App>) -> Result<Vec<feedback::InboxFeedback>> {
    app.read(|c, _| feedback::list_all(c))
}

/// Feedback written down from the inbox: your own note or someone else's, on a deliverable, a story or the project.
#[tauri::command]
pub fn record_feedback(app: State<App>, draft: feedback::FeedbackRecord) -> Result<FeedbackView> {
    app.mutate(|c, _| feedback::record(c, &draft))
}

#[tauri::command]
pub fn set_feedback_from(app: State<App>, project: String, thread: String, id: String, from: FeedbackFrom) -> Result<FeedbackView> {
    app.mutate(|c, _| feedback::update_from(c, &project, &thread, &id, &from))
}

#[tauri::command]
/// One piece of feedback, or (`all`) it and every reply to it.
pub fn set_feedback_status(app: State<App>, project: String, thread: String, id: String, status: String, all: Option<bool>) -> Result<FeedbackView> {
    if all.unwrap_or(false) {
        app.mutate(|c, _| feedback::set_status_all(c, &project, &thread, &id, &status))
    } else {
        app.mutate(|c, _| feedback::set_status(c, &project, &thread, &id, &status))
    }
}

#[tauri::command]
pub fn update_feedback(app: State<App>, project: String, thread: String, id: String, body: String) -> Result<FeedbackView> {
    app.mutate(|c, _| feedback::update(c, &project, &thread, &id, &body))
}

#[tauri::command]
pub fn delete_feedback(app: State<App>, project: String, thread: String, id: String) -> Result<()> {
    app.mutate(|c, _| feedback::delete(c, &project, &thread, &id))
}

// ---- meeting notes (daily notes) -------------------------------------------------------------------

/// The days' notes, newest first; with a project or a person (a tag), only the days with a meeting of theirs.
#[tauri::command]
pub fn list_daily_notes(app: State<App>, project: Option<String>, person: Option<String>) -> Result<Vec<crate::daily::DailyNote>> {
    app.read(|c, _| crate::daily::list(c, project.as_deref(), person.as_deref()))
}

#[tauri::command]
pub fn get_daily_note(app: State<App>, date: String) -> Result<Option<crate::daily::DailyNote>> {
    app.read(|c, _| crate::daily::get(c, &date))
}

/// Saves a day's meetings; none (and nothing else in the note) removes its file. Returns the note, or none when it went.
#[tauri::command]
pub fn save_daily_note(app: State<App>, note: crate::daily::DailyDraft) -> Result<Option<crate::daily::DailyNote>> {
    app.mutate(|c, _| crate::daily::save(c, &note))
}

#[tauri::command]
pub fn delete_daily_note(app: State<App>, date: String) -> Result<()> {
    app.mutate(|c, _| crate::daily::delete(c, &date))
}

// ---- uploads and project files ----------------------------------------------------------------------

/// Copies files from this machine into the project's attachments and records them in the thread.
#[tauri::command]
pub fn upload_files(app: State<App>, project: String, thread: String, paths: Vec<String>, description: String) -> Result<Vec<UploadView>> {
    let sources: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    app.mutate(|c, root| uploads::add_files(c, root, &project, &thread, &sources, &description))
}

/// A file dropped on the window: its bytes come as the request's raw body (no JSON, so a large file stays cheap), and
/// where it goes in the headers `project`, `thread` and `name` (percent-encoded, as headers carry only ASCII).
#[tauri::command]
pub fn upload_dropped(app: State<App>, request: tauri::ipc::Request<'_>) -> Result<UploadView> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(Error::invalid("The file didn't arrive: send its bytes as the request body"));
    };
    let header = |key: &str| -> Result<String> {
        let raw = request.headers().get(key).and_then(|v| v.to_str().ok()).ok_or_else(|| Error::invalid(format!("Missing `{key}`")))?;
        percent_decode(raw).ok_or_else(|| Error::invalid(format!("`{key}` is not percent-encoded UTF-8")))
    };
    let (project, thread, name) = (header("project")?, header("thread")?, header("name")?);
    app.mutate(|c, root| uploads::add_bytes(c, root, &project, &thread, &name, bytes, ""))
}

/// Undoes `encodeURIComponent`: `%XX` escapes to bytes, read as UTF-8; `None` for a broken escape or text.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}


/// Every file uploaded in the project, from all its threads (the project's Attachments tab).
#[tauri::command]
pub fn list_project_uploads(app: State<App>, project: String) -> Result<Vec<UploadView>> {
    app.read(|c, root| uploads::list_project(c, root, &project))
}

#[tauri::command]
pub fn update_upload(app: State<App>, project: String, thread: String, id: String, description: String) -> Result<UploadView> {
    app.mutate(|c, root| uploads::set_description(c, root, &project, &thread, &id, &description))
}

#[tauri::command]
pub fn delete_upload(app: State<App>, project: String, thread: String, id: String) -> Result<()> {
    app.mutate(|c, root| uploads::delete(c, root, &project, &thread, &id))
}

/// A path inside a project's folder, from a path given relative to it; anything that could leave the folder is refused.
fn project_path(root: &Path, project: &str, rel: &str) -> Result<PathBuf> {
    safe_segment(project)?;
    let rel_path = Path::new(rel);
    if rel.is_empty() || rel_path.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(Error::invalid("That path isn't inside the project"));
    }
    Ok(uploads::project_dir(root, project).join(rel_path))
}

fn safe_segment(segment: &str) -> Result<()> {
    if segment.is_empty() || segment == "." || segment == ".." || segment.contains(['/', '\\']) {
        return Err(Error::invalid("That isn't a project"));
    }
    Ok(())
}

/// The bytes of a file in a project's folder (an attachment, a deliverable), for the screen to show.
#[tauri::command]
pub fn read_project_file(app: State<App>, project: String, path: String) -> Result<tauri::ipc::Response> {
    let file = app.read(|_, root| project_path(root, &project, &path))?;
    let bytes = std::fs::read(&file).map_err(|e| Error::io(&file, e))?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// A picture in a project's folder, as the window can show it (a JPEG XL one as PNG, see `images`).
#[tauri::command]
pub fn read_project_image(app: State<App>, project: String, path: String) -> Result<tauri::ipc::Response> {
    let file = app.read(|_, root| project_path(root, &project, &path))?;
    Ok(tauri::ipc::Response::new(crate::images::displayable_file(&file)?))
}

/// The preview of a picture being dropped onto the window, from wherever it is on this machine: pictures only.
#[tauri::command]
pub fn preview_dropped_image(path: String) -> Result<tauri::ipc::Response> {
    Ok(tauri::ipc::Response::new(crate::images::preview_of(Path::new(&path))?))
}

/// Opens a file in a project's folder with the system's default program, or shows it in the file manager.
#[tauri::command]
pub fn open_project_file(app: AppHandle, project: String, path: String, reveal: bool) -> Result<()> {
    let state = app.state::<App>();
    let file = state.read(|_, root| project_path(root, &project, &path))?;
    if !file.exists() {
        return Err(Error::not_found("That file isn't there any more"));
    }
    let opener = app.opener();
    if reveal {
        opener.reveal_item_in_dir(&file).map_err(Error::screenshot)
    } else {
        opener.open_path(file.display().to_string(), None::<&str>).map_err(Error::screenshot)
    }
}

// ---- deliverables -----------------------------------------------------------------------------------

#[tauri::command]
pub fn list_deliverables(app: State<App>, project: String) -> Result<Vec<DeliverableSummary>> {
    app.read(|c, _| deliverables::list(c, &project))
}

#[tauri::command]
pub fn get_deliverable(app: State<App>, project: String, path: String) -> Result<DeliverableView> {
    app.read(|c, _| deliverables::get(c, &project, &path))
}

#[tauri::command]
pub fn create_deliverable(app: State<App>, project: String, name: String, body: String) -> Result<DeliverableView> {
    app.mutate(|c, _| deliverables::create(c, &project, &name, &body))
}

/// What changed in a markdown deliverable from `from` to `to`: each a version ("0.3") or "live".
#[tauri::command]
pub fn deliverable_changes(app: State<App>, project: String, path: String, from: String, to: String) -> Result<history::Comparison> {
    app.read(|c, _| history::compare(c, &project, &path, &from, &to))
}

/// The changes in a markdown deliverable's text as it is being typed (`text`), since a version (`against`, the newest
/// when none): where each is, who made it, and how to undo it.
#[tauri::command]
pub fn deliverable_live_changes(app: State<App>, project: String, path: String, text: String, against: Option<String>) -> Result<history::LiveChanges> {
    app.read(|c, _| history::live_changes(c, &project, &path, &text, against.as_deref()))
}

/// Who wrote what is in a markdown deliverable at `at` (a version or "live"); with `focus`, only what that version wrote.
#[tauri::command]
pub fn deliverable_authors(app: State<App>, project: String, path: String, at: String, focus: Option<String>) -> Result<history::Authorship> {
    app.read(|c, _| history::authors(c, &project, &path, &at, focus.as_deref()))
}

#[tauri::command]
pub fn save_deliverable(app: State<App>, project: String, path: String, body: String) -> Result<DeliverableView> {
    app.mutate(|c, _| deliverables::save(c, &project, &path, &body))
}

#[tauri::command]
pub fn publish_deliverable(
    app: State<App>,
    project: String,
    path: String,
    bump: Option<Bump>,
    version: Option<String>,
    choices: Option<PublishChoices>,
) -> Result<DeliverableView> {
    app.mutate(|c, root| {
        deliverables::publish_with(c, root, &project, &path, bump.unwrap_or_default(), version.as_deref(), &choices.unwrap_or_default())
    })
}

/// Makes a PDF of a markdown deliverable (the live file or a published version) in the project's `exports/` folder,
/// and opens it with the system's PDF viewer when `open` is set. Returns its path relative to the project's folder.
#[tauri::command]
pub async fn export_deliverable_pdf(app: AppHandle, project: String, path: String, open: bool) -> Result<String> {
    let state = app.state::<App>();
    let (prepared, root) = state.read(|c, root| Ok((crate::pdf::prepare(c, root, &project, &path)?, root.to_path_buf())))?;
    // Setting a long document takes a moment: off the main thread, without holding the vault.
    let rel = tauri::async_runtime::spawn_blocking(move || crate::pdf::make(prepared))
        .await
        .map_err(|e| Error::export(format!("Making the PDF stopped unexpectedly: {e}")))??;
    // The PDF is a file in the vault, so the sync takes it along.
    state.syncer.dirty();
    if open {
        let file = project_path(&root, &project, &rel)?;
        app.opener().open_path(file.display().to_string(), None::<&str>).map_err(Error::screenshot)?;
    }
    Ok(rel)
}

/// Which layout the project's PDFs are set in, and where the layouts of one's own are.
#[tauri::command]
pub fn pdf_layout(app: State<App>, project: String) -> Result<crate::pdf::LayoutView> {
    app.read(|c, root| {
        projects::require(c, &project)?;
        crate::pdf::layout_view(root, &project)
    })
}

/// Starts a layout of one's own for the project or the whole vault: a copy of the one used now, to change.
#[tauri::command]
pub fn customise_pdf_layout(app: State<App>, project: String, scope: crate::pdf::LayoutScope) -> Result<crate::pdf::LayoutView> {
    let view = app.read(|c, root| {
        projects::require(c, &project)?;
        crate::pdf::customise_layout(root, &project, scope)
    })?;
    app.syncer.dirty();
    Ok(view)
}

/// Deletes a layout of one's own, so the one under it is used again.
#[tauri::command]
pub fn reset_pdf_layout(app: State<App>, project: String, scope: crate::pdf::LayoutScope) -> Result<crate::pdf::LayoutView> {
    let view = app.read(|c, root| {
        projects::require(c, &project)?;
        crate::pdf::reset_layout(root, &project, scope)
    })?;
    app.syncer.dirty();
    Ok(view)
}

/// Opens a layout of one's own in the program the system uses for it, or shows it in its folder.
#[tauri::command]
pub fn open_pdf_layout(app: AppHandle, project: String, scope: crate::pdf::LayoutScope, reveal: bool) -> Result<()> {
    let file = app.state::<App>().read(|_, root| crate::pdf::layout_file(root, &project, scope))?;
    let Some((file, _)) = file else { return Err(Error::not_found("There is no layout of your own there yet")) };
    let opener = app.opener();
    if reveal {
        opener.reveal_item_in_dir(&file).map_err(Error::screenshot)
    } else {
        opener.open_path(file.display().to_string(), None::<&str>).map_err(|_| {
            Error::invalid("No program is set to open .typ files. Show it in its folder and open it with a text editor")
        })
    }
}

/// How the versions of a deliverable came to be, by thread. `path` is the live file or any of its versions.
#[tauri::command]
pub fn get_provenance(app: State<App>, project: String, path: String) -> Result<provenance::Provenance> {
    app.read(|c, _| provenance::for_deliverable(c, &project, &deliverables::split_version(&path).0))
}

/// Says who made a version, or sets its AI-use label or change note.
#[tauri::command]
pub fn update_version_record(app: State<App>, project: String, path: String, version: String, update: VersionUpdate) -> Result<provenance::Provenance> {
    let base = deliverables::split_version(&path).0;
    app.mutate(|c, _| {
        provenance::update_record(c, &project, &base, &version, &update)?;
        provenance::for_deliverable(c, &project, &base)
    })
}

/// Gives a deliverable another name; returns its new path.
#[tauri::command]
pub fn rename_deliverable(app: State<App>, project: String, path: String, name: String) -> Result<String> {
    app.mutate(|c, root| deliverables::rename(c, root, &project, &path, &name))
}

#[tauri::command]
pub fn delete_deliverable(app: State<App>, project: String, path: String) -> Result<()> {
    app.mutate(|c, root| deliverables::delete(c, root, &project, &path))
}

// ---- templates --------------------------------------------------------------------------------------

/// The templates of one scope: "" for the vault-wide ones, or a project's slug.
#[tauri::command]
pub fn list_templates(app: State<App>, scope: String) -> Result<Vec<TemplateView>> {
    app.read(|c, _| templates::list(c, &scope))
}

/// A project's templates followed by the vault-wide ones (marked if the project replaced them).
#[tauri::command]
pub fn project_templates(app: State<App>, project: String) -> Result<Vec<TemplateView>> {
    app.read(|c, _| templates::for_project(c, &project))
}

/// What a prompt in the project can start from.
#[tauri::command]
pub fn effective_templates(app: State<App>, project: String) -> Result<Vec<TemplateView>> {
    app.read(|c, _| templates::effective(c, &project))
}

#[tauri::command]
pub fn create_template(app: State<App>, scope: String, draft: TemplateDraft) -> Result<TemplateView> {
    app.mutate(|c, _| templates::create(c, &scope, &draft))
}

/// Gives the project its own copy of a vault-wide template, which then replaces the shared one there.
#[tauri::command]
pub fn override_template(app: State<App>, project: String, id: String) -> Result<TemplateView> {
    app.mutate(|c, _| templates::override_global(c, &project, &id))
}

#[tauri::command]
pub fn update_template(app: State<App>, scope: String, id: String, draft: TemplateDraft) -> Result<TemplateView> {
    app.mutate(|c, _| templates::update(c, &scope, &id, &draft))
}

/// The readers a template can name by a word: `me`, `client`, `people` and the rest.
#[tauri::command]
pub fn template_reader_presets() -> Vec<templates::Preset> {
    templates::PRESETS.to_vec()
}

/// What a new deliverable in the project starts with from this template: its skeleton with its readers noted.
#[tauri::command]
pub fn template_skeleton(app: State<App>, project: String, id: String) -> Result<String> {
    app.read(|c, _| templates::skeleton_for(c, &project, &id))
}

#[tauri::command]
pub fn delete_template(app: State<App>, scope: String, id: String) -> Result<()> {
    app.mutate(|c, _| templates::delete(c, &scope, &id))
}

// ---- user stories and the board ---------------------------------------------------------------------

#[tauri::command]
pub fn list_stories(app: State<App>, project: String) -> Result<Vec<StoryView>> {
    app.read(|c, _| stories::list(c, &project))
}

#[tauri::command]
pub fn create_story(app: State<App>, project: String, draft: StoryDraft) -> Result<StoryView> {
    app.mutate(|c, _| stories::create(c, &project, draft))
}

#[tauri::command]
pub fn update_story(app: State<App>, project: String, id: String, updates: StoryUpdate) -> Result<StoryView> {
    app.mutate(|c, _| stories::update(c, &project, &id, updates))
}

#[tauri::command]
pub fn delete_story(app: State<App>, project: String, id: String) -> Result<()> {
    app.mutate(|c, _| stories::delete(c, &project, &id))
}

#[tauri::command]
pub fn link_story_thread(app: State<App>, project: String, id: String, thread_slug: String) -> Result<StoryView> {
    app.mutate(|c, _| stories::add_thread(c, &project, &id, &thread_slug))
}

#[tauri::command]
pub fn unlink_story_thread(app: State<App>, project: String, id: String, thread_slug: String) -> Result<StoryView> {
    app.mutate(|c, _| stories::remove_thread(c, &project, &id, &thread_slug))
}

#[tauri::command]
pub fn get_board(app: State<App>, project: String) -> Result<Board> {
    app.read(|c, _| board::get(c, &project))
}

#[tauri::command]
pub fn create_task(app: State<App>, project: String, title: String, body: String, story_id: Option<String>, thread_slug: Option<String>) -> Result<TaskView> {
    app.mutate(|c, _| board::create_task(c, &project, &title, &body, story_id.as_deref(), thread_slug.as_deref()))
}

#[tauri::command]
pub fn update_task(app: State<App>, project: String, id: String, updates: TaskUpdate) -> Result<TaskView> {
    app.mutate(|c, _| board::update_task(c, &project, &id, updates))
}

#[tauri::command]
pub fn delete_task(app: State<App>, project: String, id: String) -> Result<()> {
    app.mutate(|c, _| board::delete_task(c, &project, &id))
}

#[tauri::command]
pub fn add_column(app: State<App>, project: String, name: String) -> Result<Vec<Column>> {
    app.mutate(|c, _| board::add_column(c, &project, &name))
}

#[tauri::command]
pub fn rename_column(app: State<App>, project: String, id: String, name: String) -> Result<Vec<Column>> {
    app.mutate(|c, _| board::rename_column(c, &project, &id, &name))
}

#[tauri::command]
pub fn delete_column(app: State<App>, project: String, id: String) -> Result<Vec<Column>> {
    app.mutate(|c, _| board::delete_column(c, &project, &id))
}

// ---- the vault and GitHub sync ----------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSettings {
    pub repo: String,
    pub branch: String,
    /// The token itself never leaves the backend; the screen only learns that there is one.
    pub has_token: bool,
    pub token_hint: String,
    pub configured: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub vault_path: String,
    /// The location is fixed by the `LEDGER_VAULT` environment variable.
    pub vault_locked: bool,
    /// Where the vault's settings and its secret are kept (both inside the vault, both gitignored).
    pub settings_file: String,
    pub secrets_file: String,
    pub version: String,
    pub sync: SyncSettings,
    pub status: SyncStatus,
    pub screenshot: ScreenshotSettings,
    /// Whether sync encrypts, and whether this machine has the key. Never the key.
    pub encryption: EncryptionStatus,
}

fn view(app: &App) -> SettingsView {
    let (root, locked, env, keys) = {
        let core = app.lock();
        let root = core.vault.root().to_path_buf();
        let env = settings::read_env(&root);
        (root, core.vault_from_env, env, core.keys.clone())
    };
    let token_hint = if env.token.chars().count() >= 8 {
        format!("...{}", env.token.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<String>())
    } else {
        String::new()
    };
    SettingsView {
        vault_path: root.display().to_string(),
        vault_locked: locked,
        settings_file: settings::settings_path(&root).display().to_string(),
        secrets_file: settings::secrets_path(&root).display().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        sync: SyncSettings {
            configured: env.complete().is_some(),
            has_token: !env.token.is_empty(),
            token_hint,
            repo: env.repo.clone(),
            branch: env.branch.clone(),
        },
        status: app.syncer.status(),
        screenshot: app.screenshots.settings(&root),
        encryption: keystore::status(&root, &*keys),
    }
}

#[tauri::command]
pub fn get_settings(app: State<App>) -> SettingsView {
    view(&app)
}

#[tauri::command]
pub fn sync_status(app: State<App>) -> SyncStatus {
    app.syncer.status()
}

/// What GitHub holds of the vault as of the last sync that finished; nothing while sync is off or hasn't run yet.
#[tauri::command]
pub fn sync_overview(app: State<App>) -> Option<crate::sync::Overview> {
    let (root, config) = {
        let core = app.lock();
        (core.vault.root().to_path_buf(), core.sync.clone())
    };
    let config = config?;
    crate::sync::overview(&root, &config.repo, &config.branch)
}

/// How many commits the synced branch holds, as far as the last agreed commit, asked of GitHub. Nothing while sync is
/// off or hasn't run yet; an error when GitHub can't be reached, which the sync card leaves out.
#[tauri::command]
pub async fn sync_commits(app: State<'_, App>) -> Result<Option<u64>> {
    let (root, config) = {
        let core = app.lock();
        (core.vault.root().to_path_buf(), core.sync.clone())
    };
    let Some(config) = config else { return Ok(None) };
    let Some(head) = crate::sync::overview(&root, &config.repo, &config.branch).and_then(|o| o.head) else { return Ok(None) };
    let count = tauri::async_runtime::spawn_blocking(move || GithubRemote::new(&config)?.commit_count(&head))
        .await
        .map_err(|e| Error::sync(format!("Counting the commits stopped unexpectedly: {e}")))??;
    Ok(Some(count))
}

/// Saves the repository and branch to the vault's settings and the token to its `.env`, and starts syncing. A
/// blank token keeps the one already saved.
#[tauri::command]
pub fn save_sync_settings(app: State<App>, repo: String, branch: String, token: Option<String>) -> Result<SettingsView> {
    let repo = settings::normalize_repo(&repo)?;
    let branch = settings::normalize_branch(&branch)?;

    let config = {
        let mut core = app.lock();
        let root = core.vault.root().to_path_buf();
        let token = token
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| settings::read_env(&root).token);
        if token.is_empty() {
            return Err(Error::invalid("Enter an access token"));
        }
        settings::write_env(&root, &SyncEnv { repo, branch, token })?;
        core.sync = settings::read_env(&root).complete();
        core.sync.clone()
    };
    app.syncer.reconfigure(config.as_ref());
    Ok(view(&app))
}

/// Turns sync off. The repository and branch stay in the settings; the token is removed.
#[tauri::command]
pub fn disconnect_sync(app: State<App>) -> Result<SettingsView> {
    {
        let mut core = app.lock();
        let root = core.vault.root().to_path_buf();
        let env = settings::read_env(&root);
        settings::write_env(&root, &SyncEnv { token: String::new(), ..env })?;
        core.sync = None;
    }
    app.syncer.reconfigure(None);
    Ok(view(&app))
}

/// Tries the connection with the values on screen (a blank token means the saved one) without saving anything.
#[tauri::command]
pub async fn test_sync(app: State<'_, App>, repo: String, branch: String, token: Option<String>) -> Result<String> {
    let repo = settings::normalize_repo(&repo)?;
    let branch = settings::normalize_branch(&branch)?;
    let token = {
        let core = app.lock();
        token
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| settings::read_env(core.vault.root()).token)
    };
    if token.is_empty() {
        return Err(Error::invalid("Enter an access token"));
    }

    let config = SyncConfig { repo: repo.clone(), branch: branch.clone(), token };
    let check = tauri::async_runtime::spawn_blocking(move || GithubRemote::new(&config)?.check())
        .await
        .map_err(|e| Error::sync(format!("The connection test stopped unexpectedly: {e}")))??;
    if !check.private {
        return Err(Error::sync(format!("{repo} is a public repository. Ledger only syncs to private repositories.")));
    }
    if !check.can_push {
        return Err(Error::sync("The token can read this repository but not write to it. It needs write access to its contents."));
    }
    Ok(if check.branch_exists {
        format!("Connected to {repo}. The branch {branch} exists.")
    } else {
        format!("Connected to {repo}. The branch {branch} doesn't exist yet and will be created on the first sync (the default branch is {}).", check.default_branch)
    })
}

#[tauri::command]
pub fn sync_now(app: State<App>) -> Result<()> {
    if app.lock().sync.is_none() {
        return Err(Error::invalid("GitHub sync isn't set up yet"));
    }
    app.syncer.now();
    Ok(())
}

// ---- encrypting what goes to GitHub -----------------------------------------------------------------

/// Where the vault is and what holds its key, taken out of the core so the key store is asked without the vault
/// lock held.
fn key_context(app: &App) -> (PathBuf, Arc<dyn KeyStore>) {
    let core = app.lock();
    (core.vault.root().to_path_buf(), core.keys.clone())
}

#[tauri::command]
pub fn encryption_status(app: State<App>) -> EncryptionStatus {
    let (root, keys) = key_context(&app);
    keystore::status(&root, &*keys)
}

/// Turns encryption of what is synced on or off, and syncs so that it takes effect. It can't be turned on until
/// there is a key.
#[tauri::command]
pub fn set_encryption_enabled(app: State<App>, enabled: bool) -> Result<SettingsView> {
    let (root, keys) = key_context(&app);
    keystore::set_enabled(&root, &*keys, enabled)?;
    if app.lock().sync.is_some() {
        app.syncer.now();
    }
    Ok(view(&app))
}

/// Makes a new random key and keeps it in the vault's `.ledger/.env`. With a key already on this machine that
/// takes `replace`.
#[tauri::command]
pub fn generate_encryption_key(app: State<App>, replace: bool) -> Result<SettingsView> {
    let (root, keys) = key_context(&app);
    keystore::generate(&root, &*keys, replace)?;
    Ok(view(&app))
}

/// Keeps a key made on another machine: its recovery string, or the bare key as hex or base64.
#[tauri::command]
pub fn import_encryption_key(app: State<App>, key: String, replace: bool) -> Result<SettingsView> {
    let key = Zeroizing::new(key);
    let (root, keys) = key_context(&app);
    keystore::import(&root, &*keys, &key, replace)?;
    Ok(view(&app))
}

/// The recovery string for the key: the one command that hands the key out, and only when asked for.
#[tauri::command]
pub fn export_recovery_key(app: State<App>) -> Result<String> {
    let (root, keys) = key_context(&app);
    let recovery = keystore::export(&root, &*keys)?;
    Ok(String::clone(&recovery))
}

/// Moves the app to the vault in `path`. With `copy`, an empty destination first receives the current vault.
#[tauri::command]
pub fn set_vault_path(app: AppHandle, path: String, copy: bool) -> Result<SettingsView> {
    switch_vault(&app, |core| core.set_vault(&PathBuf::from(path.trim()), copy))?;
    Ok(view(&app.state::<App>()))
}

/// Switches vaults with `change`, then sets up what belongs to the new one: its sync, its Portflow and its screenshot
/// hotkey.
fn switch_vault<T>(app: &AppHandle, change: impl FnOnce(&mut crate::state::Core) -> Result<T>) -> Result<T> {
    let state = app.state::<App>();
    let (out, config) = {
        let mut core = state.lock();
        let out = change(&mut core)?;
        (out, core.sync.clone())
    };
    state.syncer.reconfigure(config.as_ref());
    if let Some(portflow) = app.try_state::<crate::portflow::PortflowSyncer>() {
        portflow.vault_switched();
    }
    screenshot::register_saved(app);
    Ok(out)
}

// ---- the vault picker -------------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntry {
    pub path: String,
    /// The folder's name, which is what the picker calls the vault.
    pub name: String,
    /// The folder is still there (a vault on a drive that isn't plugged in, or that was deleted, isn't).
    pub exists: bool,
    /// It is the vault the app has open.
    pub open: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultsView {
    /// A vault is open; before the first one, the app shows the picker.
    pub open: bool,
    /// The location is fixed by `LEDGER_VAULT`, so no other vault can be opened.
    pub locked: bool,
    /// The open vault, then the others opened on this machine, the most recent first.
    pub vaults: Vec<VaultEntry>,
    /// Where a new vault goes unless another folder is picked: the user's documents.
    pub suggested_parent: Option<String>,
}

fn vault_entry(path: &Path, open: bool) -> VaultEntry {
    VaultEntry {
        path: path.display().to_string(),
        name: path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().to_string()),
        exists: path.is_dir(),
        open,
    }
}

fn vaults_view(app: &AppHandle) -> VaultsView {
    let state = app.state::<App>();
    let (open, locked, current, config_dir) = {
        let core = state.lock();
        (core.open, core.vault_from_env, core.vault.root().to_path_buf(), core.config_dir.clone())
    };
    let mut vaults = Vec::new();
    if open {
        vaults.push(vault_entry(&current, true));
    }
    for path in settings::load_config(&config_dir).recent_vaults {
        let path = PathBuf::from(path);
        if !(open && path == current) {
            vaults.push(vault_entry(&path, false));
        }
    }
    let suggested_parent = app.path().document_dir().ok().map(|d| d.display().to_string());
    VaultsView { open, locked, vaults, suggested_parent }
}

#[tauri::command]
pub fn list_vaults(app: AppHandle) -> VaultsView {
    vaults_view(&app)
}

/// Makes a new vault called `name` in the folder `parent`, opens it, and starts the setup guide for it.
#[tauri::command]
pub fn create_vault(app: AppHandle, parent: String, name: String) -> Result<VaultsView> {
    let config_dir = switch_vault(&app, |core| {
        core.create_vault(&PathBuf::from(parent.trim()), &name)?;
        Ok(core.config_dir.clone())
    })?;
    crate::onboarding::restart(&config_dir)?;
    Ok(vaults_view(&app))
}

/// Opens a folder that exists as the vault: one from the list, or any folder picked.
#[tauri::command]
pub fn open_vault(app: AppHandle, path: String) -> Result<VaultsView> {
    switch_vault(&app, |core| core.open_vault(&PathBuf::from(path.trim())))?;
    Ok(vaults_view(&app))
}

/// Takes a vault off the list. Its files are left where they are.
#[tauri::command]
pub fn forget_vault(app: AppHandle, path: String) -> Result<VaultsView> {
    let config_dir = app.state::<App>().lock().config_dir.clone();
    settings::forget_vault(&config_dir, &path)?;
    Ok(vaults_view(&app))
}

/// Reads the vault in again from its files.
#[tauri::command]
pub fn reload_vault(app: State<App>) -> Result<()> {
    app.lock().reload()?;
    app.syncer.dirty();
    Ok(())
}

/// Reloads if the files were changed behind the app's back (in Obsidian, say). Whether anything was reloaded.
#[tauri::command]
pub fn check_vault_changes(app: State<App>) -> Result<bool> {
    let mut core = app.lock();
    if !core.vault.changed_on_disk() {
        return Ok(false);
    }
    core.reload()?;
    // What was edited elsewhere is part of the vault now, and goes to GitHub like anything else.
    app.syncer.dirty();
    Ok(true)
}

// ---- screenshots ------------------------------------------------------------------------------------

/// Saves the screenshot hotkey in the vault's settings. It is registered first, replacing the old one live; if the
/// system refuses it nothing is saved and the old one keeps working.
#[tauri::command]
pub fn save_screenshot_hotkey(app: AppHandle, hotkey: String) -> Result<SettingsView> {
    screenshot::apply_hotkey(&app, &hotkey)?;
    let state = app.state::<App>();
    let root = state.lock().vault.root().to_path_buf();
    settings::update_settings(&root, |s| s.screenshot_hotkey = Some(hotkey.trim().to_string()))?;
    Ok(view(&state))
}

/// The shell's layout as this vault's settings have it (the folded sidebar: an icon rail or nothing).
#[tauri::command]
pub fn layout_settings(app: State<App>) -> settings::LayoutSettings {
    let root = app.lock().vault.root().to_path_buf();
    settings::read_settings(&root).layout
}

/// Saves what a folded sidebar becomes, in `.ledger/settings.json`.
#[tauri::command]
pub fn save_sidebar_fold(app: State<App>, fold: settings::SidebarFold) -> Result<settings::LayoutSettings> {
    let root = app.lock().vault.root().to_path_buf();
    settings::update_settings(&root, |s| s.layout.sidebar_fold = fold)?;
    Ok(settings::read_settings(&root).layout)
}

/// Chooses between the built-in capture and the user's own tool (and saves the tool's command either way).
#[tauri::command]
pub fn save_screenshot_tool(app: State<App>, use_command: bool, command: String) -> Result<SettingsView> {
    let root = app.lock().vault.root().to_path_buf();
    screenshot::save_tool(&root, use_command, &command)?;
    Ok(view(&app))
}

/// The screenshot waiting to be attached, if there is one (Ledger may have been launched with a picture).
#[tauri::command]
pub fn pending_screenshot(app: AppHandle) -> Option<screenshot::Taken> {
    screenshot::pending(&app)
}

/// Takes a screenshot as if the hotkey had been pressed.
#[tauri::command]
pub fn take_screenshot(app: AppHandle) {
    screenshot::trigger(&app);
}

/// Lets go of the hotkey while the settings screen records a new one (and takes it back afterwards).
#[tauri::command]
pub fn pause_screenshot_hotkey(app: AppHandle, paused: bool) {
    screenshot::pause_hotkey(&app, paused);
}

/// The overlay's picture of the screen, as raw PNG bytes.
#[tauri::command]
pub fn screenshot_image(app: AppHandle) -> Result<tauri::ipc::Response> {
    Ok(tauri::ipc::Response::new(screenshot::overlay_image(&app)?))
}

#[tauri::command]
pub fn screenshot_overlay_ready(app: AppHandle) {
    screenshot::overlay_ready(&app);
}

#[tauri::command]
pub fn screenshot_finish(app: AppHandle, region: Region) -> Result<()> {
    screenshot::finish(&app, region)
}

#[tauri::command]
pub fn screenshot_cancel(app: AppHandle) {
    screenshot::cancel(&app);
}

/// The screenshot waiting to be attached, as PNG bytes.
#[tauri::command]
pub fn pending_screenshot_image(app: AppHandle) -> Result<tauri::ipc::Response> {
    Ok(tauri::ipc::Response::new(screenshot::pending_image(&app)?))
}

/// Saves the waiting screenshot in the project's attachments and records it in the thread.
#[tauri::command]
pub fn attach_screenshot(app: AppHandle, project: String, thread: String, description: String) -> Result<UploadView> {
    let state = app.state::<App>();
    let png = screenshot::pending_image(&app)?;
    let upload = state.mutate(|c, root| uploads::add_bytes(c, root, &project, &thread, &screenshot::file_name(), &png, &description))?;
    screenshot::discard_pending(&app);
    Ok(upload)
}

#[tauri::command]
pub fn discard_screenshot(app: AppHandle) {
    screenshot::discard_pending(&app);
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn percent_decoding_undoes_encode_uri_component() {
        assert_eq!(percent_decode("Sch%C3%A9ma%20v2.png").as_deref(), Some("Schéma v2.png"));
        assert_eq!(percent_decode("plain-name.pdf").as_deref(), Some("plain-name.pdf"));
        assert_eq!(percent_decode("bad%2"), None);
        assert_eq!(percent_decode("bad%zz"), None);
        assert_eq!(percent_decode("%FF"), None);
    }
}
