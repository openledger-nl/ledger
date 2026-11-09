//! The shapes that cross the command boundary. Responses are flat camelCase JSON the frontend consumes
//! directly: the stored records plus rendered HTML where markdown is involved.

use serde::{Deserialize, Serialize};

use crate::claude_code::records::RunRecordView;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedThread {
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub project_slug: String,
    pub slug: String,
    pub title: String,
    pub created_at: String,
    pub linked_thread_slugs: Vec<String>,
    pub linked_count: i64,
    pub post_count: i64,
    pub prompt_count: i64,
    pub feedback_count: i64,
    pub upload_count: i64,
    pub story_count: i64,
    pub last_activity: String,
}

/// A project with its threads, as the sidebar shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub slug: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub threads: Vec<ThreadSummary>,
    pub deliverable_count: i64,
    pub story_count: i64,
    pub task_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub slug: String,
    pub name: String,
    pub description: String,
    /// The reading guide (README.md), raw and rendered.
    pub readme: String,
    pub readme_html: String,
    pub thread_count: i64,
    pub deliverable_count: i64,
    pub story_count: i64,
    pub task_count: i64,
    pub open_task_count: i64,
}

/// `guiding_question`: seeded from a template's guiding questions.
/// `version_published`: an automatic note announcing a published deliverable version; `deliverable_path` and
/// `deliverable_version` point at the snapshot, and `prompt_id` at the prompt that caused it, when one did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostView {
    pub id: String,
    pub parent_id: Option<String>,
    pub guiding_question: bool,
    pub version_published: bool,
    pub deliverable_path: Option<String>,
    pub deliverable_version: Option<String>,
    /// The prompt that caused the version, and its name and thread while it still exists.
    pub prompt_id: Option<String>,
    pub prompt_title: Option<String>,
    pub prompt_thread: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub body: String,
    pub html: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDeliverable {
    /// The live path: a prompt belongs to the deliverable, not to one of its versions.
    pub path: String,
    /// Whether the file is (still) in the project's deliverables folder.
    pub exists: bool,
    /// The versions published so far, oldest first.
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptView {
    pub id: String,
    pub thread_slug: String,
    pub title: String,
    pub template_id: Option<String>,
    pub template_name: Option<String>,
    /// What is filled in for the person, from a template or from feedback; they can edit it.
    pub base: String,
    pub base_html: String,
    /// What the person adds, in their own words.
    pub body: String,
    pub html: String,
    pub created_at: String,
    pub updated_at: String,
    pub deliverables: Vec<PromptDeliverable>,
    /// The pieces of feedback (ids) it was written to resolve.
    pub feedback: Vec<String>,
    /// The times it was run with Claude Code, newest first.
    pub runs: Vec<RunRecordView>,
}

/// What a new prompt is made of. It has no title: it is named from what it asks.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPrompt {
    /// Filled in from the template or the feedback (see [`BaseSpec`]), and edited or not.
    #[serde(default)]
    pub base: String,
    /// What the person adds.
    #[serde(default)]
    pub body: String,
    pub template_id: Option<String>,
    /// A deliverable that exists, which the prompt asks changes to (given as any of its paths; it is linked as the live file).
    pub deliverable: Option<String>,
    /// The name of a new deliverable to start for the prompt. Blank: whatever produces the deliverable names it.
    pub new_deliverable: Option<String>,
    /// The pieces of feedback (ids) it is meant to resolve.
    #[serde(default)]
    pub feedback: Vec<String>,
}

/// What a new prompt's base text is made from.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseSpec {
    /// The template a new deliverable starts from.
    pub template_id: Option<String>,
    /// The feedback the prompt is to resolve.
    #[serde(default)]
    pub feedback: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackView {
    pub id: String,
    pub thread_slug: String,
    pub deliverable_path: String,
    pub deliverable_version: Option<String>,
    /// "open" or "resolved".
    pub status: String,
    pub body: String,
    pub html: String,
    pub created_at: String,
    pub updated_at: String,
    /// The passage this is about, as plain text, when it is not about the whole document; with the words just before
    /// and after it (see [`Quote`]).
    pub quote: Option<String>,
    pub quote_prefix: Option<String>,
    pub quote_suffix: Option<String>,
    /// The version it was given on: the one it names, or for feedback on the live file the first version published after
    /// it was written (what that version froze). `None` while no version has been published since.
    pub given_on: Option<String>,
}

/// The passage feedback is about: what was selected, as plain text (marks gone, white space collapsed), and a few words
/// before and after it, which tell apart a phrase that occurs more than once. The page finds the passage again by these,
/// so the feedback keeps its place while the document changes around it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub text: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadView {
    pub id: String,
    pub thread_slug: String,
    /// Relative to the project folder, e.g. "attachments/diagram.png".
    pub file: String,
    pub name: String,
    pub size: i64,
    pub uploaded_at: String,
    pub description: String,
    pub description_html: String,
    pub image: bool,
    /// Whether the file is still where the manifest says.
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub project_slug: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
    pub linked_threads: Vec<LinkedThread>,
    pub posts: Vec<PostView>,
    pub prompts: Vec<PromptView>,
    pub feedback: Vec<FeedbackView>,
    pub uploads: Vec<UploadView>,
    pub stories: Vec<StoryView>,
}

// ---- deliverables -----------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliverableSummary {
    /// The live file, relative to the deliverables folder (even if only published versions of it are left).
    pub path: String,
    pub name: String,
    /// "markdown", "image" or "other".
    pub kind: String,
    pub size: i64,
    pub modified: String,
    /// Whether the live file exists.
    pub exists: bool,
    /// Published versions, oldest first.
    pub versions: Vec<String>,
    pub prompt_count: i64,
    pub open_feedback: i64,
    /// The AI-use (MMM) label of the newest version that has one.
    pub mmm: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliverablePrompt {
    pub thread_slug: String,
    pub thread_title: String,
    pub id: String,
    pub title: String,
}

/// A deliverable, or (`version` set) one of its published versions, which is read-only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliverableView {
    /// The path asked for: the live file, or a "<name>@v1.0.<ext>" snapshot.
    pub path: String,
    pub base_path: String,
    pub name: String,
    pub kind: String,
    pub version: Option<String>,
    pub versions: Vec<String>,
    pub exists: bool,
    pub size: i64,
    pub modified: String,
    /// The text of a markdown deliverable.
    pub body: Option<String>,
    pub html: Option<String>,
    pub editable: bool,
    pub prompts: Vec<DeliverablePrompt>,
    pub feedback: Vec<FeedbackView>,
}

// ---- templates --------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateView {
    /// "" for a vault-wide template, else the project's slug.
    pub scope: String,
    pub id: String,
    pub name: String,
    /// The "use when…" hint.
    pub trigger: String,
    pub questions: Vec<String>,
    /// The skeleton of the deliverable.
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    /// A vault-wide template that the project has replaced with one of its own of the same id.
    pub overridden: bool,
}

// ---- stories, tasks, the board ----------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Criterion {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryView {
    pub project_slug: String,
    pub id: String,
    pub summary: String,
    /// "As a …"
    pub role: String,
    /// "I want to …"
    pub want: String,
    /// "Such that …"
    pub reason: String,
    pub notes: String,
    pub criteria: Vec<Criterion>,
    pub threads: Vec<LinkedThread>,
    pub task_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// The Trello card this story is synced with, if any, and (when the sync has stopped) why.
    pub trello_card: Option<String>,
    pub trello_unlinked: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskView {
    pub id: String,
    pub title: String,
    pub body: String,
    pub column_id: String,
    pub story_id: Option<String>,
    pub story_summary: Option<String>,
    pub thread_slug: Option<String>,
    pub thread_title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// The Trello card this task is synced with, if any, and (when the sync has stopped) why.
    pub trello_card: Option<String>,
    pub trello_unlinked: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorySummary {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub columns: Vec<Column>,
    /// Where a newly created task lands.
    pub backlog_column_id: String,
    pub tasks: Vec<TaskView>,
    pub stories: Vec<StorySummary>,
}

// ---- semesters and sprints ----------------------------------------------------------------------------

/// A stretch of a semester. Dates are calendar days ("2026-09-01"), inclusive at both ends.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Sprint {
    pub name: String,
    pub start: String,
    pub end: String,
    pub goal: String,
}

/// Days off within a semester: the sprint clock stops, and starts again after it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Vacation {
    pub name: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemesterView {
    pub id: String,
    pub name: String,
    pub start: String,
    pub end: String,
    pub sprint_weeks: i64,
    pub notes: String,
    pub sprints: Vec<Sprint>,
    pub vacations: Vec<Vacation>,
    pub created_at: String,
    pub updated_at: String,
}

/// What the caller supplies to create or change a semester. Without sprints, they are cut from the start in
/// stretches of `sprint_weeks` weeks of work, around the vacations.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SemesterDraft {
    pub name: String,
    pub start: String,
    pub end: String,
    pub sprint_weeks: i64,
    pub notes: String,
    pub sprints: Vec<Sprint>,
    pub vacations: Vec<Vacation>,
}

// ---- activity ---------------------------------------------------------------------------------------

/// One thing made in a project, at a moment: what the contribution graph counts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub project_slug: String,
    /// thread, post, prompt, feedback, upload, version, run, story or task.
    pub kind: String,
    pub at: String,
}

// ---- links ------------------------------------------------------------------------------------------

/// Something a `[[link]]` in a project's markdown can point at.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTarget {
    /// "thread", "deliverable" or "feedback".
    pub kind: String,
    /// The thread's slug, the deliverable's live path, or the feedback's id.
    pub id: String,
    /// What it is called: a thread's title, a deliverable's file name, the start of what a piece of feedback says.
    pub title: String,
    /// Where it is, which tells same-named things apart: a deliverable's folder, the file a piece of feedback is on.
    pub detail: String,
    /// For a deliverable and for feedback, the path that opens it: the live file, a version, or (when the live file is
    /// gone) the newest version. Empty for a thread.
    pub open: String,
}

// ---- search -----------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    /// "post", "prompt", "feedback" or "deliverable".
    pub kind: String,
    /// The post, prompt or feedback id; the path for a deliverable.
    pub id: String,
    pub project_slug: String,
    pub project_name: String,
    /// The thread's title, or the deliverable's name.
    pub title: String,
    pub thread_slug: Option<String>,
    pub created_at: String,
    pub snippet: String,
}

// ---- requests ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Bump {
    #[default]
    Minor,
    Major,
}

/// What the person says about a version when publishing it. `thread`: absent picks the default one, an empty string
/// files the version under no thread. A `prompt` or `run` only counts when `ai` is set.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PublishChoices {
    pub thread: Option<String>,
    /// The change was made with generative AI (outside the runs Ledger records itself).
    pub ai: bool,
    /// One of the five MMM levels.
    pub mmm: Option<String>,
    pub prompt: Option<String>,
    pub run: Option<String>,
    pub note: Option<String>,
}

/// Only the fields that are present change. `by` is "human" or "ai"; for `mmm`, an empty string removes the label.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionUpdate {
    pub by: Option<String>,
    pub mmm: Option<String>,
    pub note: Option<String>,
}

/// What the caller supplies when creating a story.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoryDraft {
    pub id: String,
    pub summary: String,
    pub role: String,
    pub want: String,
    pub reason: String,
    pub notes: String,
    pub criteria: Vec<Criterion>,
    pub thread_slugs: Vec<String>,
}

/// Only the fields that are present change; `criteria` replaces the whole list.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoryUpdate {
    pub summary: Option<String>,
    pub role: Option<String>,
    pub want: Option<String>,
    pub reason: Option<String>,
    pub notes: Option<String>,
    pub criteria: Option<Vec<Criterion>>,
}

/// Only the fields that are present change. For `story_id` and `thread_slug`, an empty string clears the link.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskUpdate {
    pub title: Option<String>,
    pub body: Option<String>,
    pub column_id: Option<String>,
    pub story_id: Option<String>,
    pub thread_slug: Option<String>,
}
