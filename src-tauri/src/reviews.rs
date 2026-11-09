//! Code reviews written with margin-code, the VS Code extension: one round of reviewing one diff of a git repository,
//! with comments on lines of its files and replies to them, kept in the thread the review belongs to.
//!
//! ```text
//! projects/<project>/threads/<thread>/reviews/<review>.md             a review: repo, base..head, draft or verdict, summary
//! projects/<project>/threads/<thread>/reviews/<review>/<comment>.md   a comment in it, or a reply
//! ```
//!
//! margin-code writes and deletes these files; Ledger only reads them, into the database, and shows them in the thread.
//! It never writes them: they are not part of the export, so the write-through can't rewrite or delete them. They go
//! only with their thread (see `Vault::persist`) or their project, when the user deletes that in Ledger.
//!
//! The format, version 1, is shared with margin-code and written out in the README ("Code reviews with
//! margin-code"). Reading is lenient, as for every file in the vault: what is missing or unreadable falls back to
//! something sensible, keys Ledger doesn't know are passed over, and a file in a newer format is shown as far as it
//! can be read, with a note.

use std::collections::{BTreeMap, HashMap};

use rusqlite::{Connection, params};
use serde_yaml_ng::Mapping;

use crate::db::query_all;
use crate::error::Result;
use crate::frontmatter::MetaExt;
use crate::markdown;
use crate::models::{ReviewCommentView, ReviewFileView, ReviewView};

/// The version of the format this Ledger reads in full.
pub const FORMAT: i64 = 1;

/// The format a file says it is in. One that says nothing, or nothing readable, is taken to be the first.
fn format_of(meta: &Mapping) -> i64 {
    meta.text("format").and_then(|f| f.trim().parse().ok()).unwrap_or(FORMAT)
}

/// A positive whole number, as line numbers are.
fn number(meta: &Mapping, key: &str) -> Option<i64> {
    meta.text(key).and_then(|n| n.trim().parse().ok()).filter(|n: &i64| *n >= 1)
}

/// Reads a review file (`id` is its name) into the database. `stamp` is the file's modified time, for a file that
/// doesn't say when it was made.
pub fn insert_review(conn: &Connection, project: &str, thread: &str, id: &str, meta: &Mapping, summary: &str, stamp: &str) -> Result<()> {
    let submitted = meta.text("state").is_some_and(|s| s.trim().eq_ignore_ascii_case("submitted"));
    let repo = meta.text("repo").unwrap_or_default();
    let repo_name = meta.text("repoName").unwrap_or_else(|| repo.rsplit(['/', ':']).next().unwrap_or_default().to_string());
    let created_at = meta.text("createdAt").unwrap_or_else(|| stamp.to_string());
    let updated_at = meta.text("updatedAt").unwrap_or_else(|| created_at.clone());
    // A verdict and a submission time belong to a submitted review only; one submitted without a time was at its last change.
    let verdict = meta.text("verdict").filter(|_| submitted).map(|v| v.trim().to_lowercase());
    let submitted_at = submitted.then(|| meta.text("submittedAt").unwrap_or_else(|| updated_at.clone()));
    conn.execute(
        "INSERT OR IGNORE INTO reviews (project_slug, thread_slug, id, format, repo, repo_name, base, base_ref, head, head_ref, state,
                                        verdict, author, created_at, updated_at, submitted_at, summary)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            project,
            thread,
            id,
            format_of(meta),
            repo,
            repo_name,
            meta.text("base").unwrap_or_default(),
            meta.text("baseRef"),
            meta.text("head").unwrap_or_default(),
            meta.text("headRef"),
            if submitted { "submitted" } else { "draft" },
            verdict,
            meta.text("author").unwrap_or_default(),
            created_at,
            updated_at,
            submitted_at,
            summary,
        ],
    )?;
    Ok(())
}

/// Reads a comment file of review `review` (`id` is its name) into the database. A reply sits where the comment it
/// answers does, so whatever it says about a place is passed over.
#[allow(clippy::too_many_arguments)]
pub fn insert_comment(conn: &Connection, project: &str, thread: &str, review: &str, id: &str, meta: &Mapping, body: &str, stamp: &str) -> Result<()> {
    let reply_to = meta.text("replyTo").map(|r| r.trim().to_string()).filter(|r| !r.is_empty() && r != id);
    let place = reply_to.is_none();
    let path = meta.text("path").filter(|_| place).map(|p| p.trim().replace('\\', "/")).filter(|p| !p.is_empty());
    let line = number(meta, "line").filter(|_| place);
    // A last line before the first is read as a single line; without a first line there are no lines.
    let end_line = line.map(|first| number(meta, "endLine").filter(|last| *last >= first).unwrap_or(first));
    let side = if place && meta.text("side").is_some_and(|s| s.trim().eq_ignore_ascii_case("old")) { "old" } else { "new" };
    let code = |key: &str| meta.text(key).filter(|_| place);
    let created_at = meta.text("createdAt").unwrap_or_else(|| stamp.to_string());
    conn.execute(
        "INSERT OR IGNORE INTO review_comments (project_slug, thread_slug, review_id, id, format, path, side, line, end_line, commit_sha,
                                                code, code_before, code_after, reply_to, resolved, author, created_at, updated_at, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            project,
            thread,
            review,
            id,
            format_of(meta),
            path,
            side,
            line,
            end_line,
            meta.text("commit").filter(|_| place),
            code("code"),
            code("codeBefore"),
            code("codeAfter"),
            reply_to,
            meta.flag("resolved"),
            meta.text("author").unwrap_or_default(),
            created_at.clone(),
            meta.text("updatedAt").unwrap_or(created_at),
            body,
        ],
    )?;
    Ok(())
}

/// A comment as stored, with what the view needs to place it.
struct Comment {
    review: String,
    reply_to: Option<String>,
    view: ReviewCommentView,
}

/// The thread's reviews, oldest first, each with its comments grouped by file.
pub fn list_for_thread(conn: &Connection, project: &str, thread: &str) -> Result<Vec<ReviewView>> {
    let mut comments: HashMap<String, Vec<Comment>> = HashMap::new();
    for comment in query_all(
        conn,
        "SELECT review_id, id, format, path, side, line, end_line, commit_sha, code, code_before, code_after, reply_to, resolved,
                author, created_at, updated_at, body
         FROM review_comments WHERE project_slug = ?1 AND thread_slug = ?2 ORDER BY created_at, id",
        [project, thread],
        |r| {
            let format: i64 = r.get(2)?;
            let body: String = r.get(16)?;
            Ok(Comment {
                review: r.get(0)?,
                reply_to: r.get(11)?,
                view: ReviewCommentView {
                    id: r.get(1)?,
                    format,
                    newer_format: format > FORMAT,
                    path: r.get(3)?,
                    side: r.get(4)?,
                    line: r.get(5)?,
                    end_line: r.get(6)?,
                    commit: r.get(7)?,
                    code: r.get(8)?,
                    code_before: r.get(9)?,
                    code_after: r.get(10)?,
                    resolved: r.get(12)?,
                    author: r.get(13)?,
                    created_at: r.get(14)?,
                    updated_at: r.get(15)?,
                    html: markdown::render(&body),
                    body,
                    replies: vec![],
                },
            })
        },
    )? {
        comments.entry(comment.review.clone()).or_default().push(comment);
    }

    query_all(
        conn,
        "SELECT id, format, repo, repo_name, base, base_ref, head, head_ref, state, verdict, author, created_at, updated_at, submitted_at, summary
         FROM reviews WHERE project_slug = ?1 AND thread_slug = ?2 ORDER BY created_at, id",
        [project, thread],
        |r| {
            let format: i64 = r.get(1)?;
            let summary: String = r.get(14)?;
            Ok(ReviewView {
                id: r.get(0)?,
                thread_slug: thread.to_string(),
                format,
                newer_format: format > FORMAT,
                repo: r.get(2)?,
                repo_name: r.get(3)?,
                base: r.get(4)?,
                base_ref: r.get(5)?,
                head: r.get(6)?,
                head_ref: r.get(7)?,
                state: r.get(8)?,
                verdict: r.get(9)?,
                author: r.get(10)?,
                created_at: r.get(11)?,
                updated_at: r.get(12)?,
                submitted_at: r.get(13)?,
                summary_html: markdown::render(&summary),
                summary,
                files: vec![],
                comment_count: 0,
                open_count: 0,
            })
        },
    )
    .map(|reviews| {
        reviews
            .into_iter()
            .map(|mut review| {
                let list = comments.remove(&review.id).unwrap_or_default();
                review.comment_count = list.len() as i64;
                review.files = conversations(list);
                review.open_count = review.files.iter().flat_map(|f| &f.comments).filter(|c| !c.resolved).count() as i64;
                review
            })
            .collect()
    })
}

/// Puts each reply under the first comment of its conversation (following replies to replies), and the conversations
/// under their file. A reply whose conversation is gone is shown as a conversation of its own, on no file.
fn conversations(list: Vec<Comment>) -> Vec<ReviewFileView> {
    let parent: HashMap<String, Option<String>> = list.iter().map(|c| (c.view.id.clone(), c.reply_to.clone())).collect();
    let root_of = |id: &str| -> String {
        let mut current = id.to_string();
        // At most as many steps as there are comments: a loop of replies ends where it started.
        for _ in 0..parent.len() {
            match parent.get(&current) {
                Some(Some(up)) if parent.contains_key(up) => current = up.clone(),
                _ => break,
            }
        }
        current
    };

    let mut firsts: Vec<ReviewCommentView> = vec![];
    let mut replies: HashMap<String, Vec<ReviewCommentView>> = HashMap::new();
    for comment in list {
        let root = root_of(&comment.view.id);
        if root == comment.view.id || comment.reply_to.is_none() {
            firsts.push(comment.view);
        } else {
            replies.entry(root).or_default().push(comment.view);
        }
    }
    // (in a loop of replies no comment is its conversation's first: each is shown as one)
    let shown: std::collections::HashSet<String> = firsts.iter().map(|c| c.id.clone()).collect();
    for (root, answers) in std::mem::take(&mut replies) {
        if shown.contains(&root) {
            replies.insert(root, answers);
        } else {
            firsts.extend(answers);
        }
    }

    let mut files: BTreeMap<String, Vec<ReviewCommentView>> = BTreeMap::new();
    for mut first in firsts {
        first.replies = replies.remove(&first.id).unwrap_or_default();
        first.replies.sort_by(|a, b| (&a.created_at, &a.id).cmp(&(&b.created_at, &b.id)));
        files.entry(first.path.clone().unwrap_or_default()).or_default().push(first);
    }
    files
        .into_iter()
        .map(|(path, mut comments)| {
            // The whole file first, then by line; the same lines in the order they were written.
            comments.sort_by(|a, b| (a.line.unwrap_or(0), &a.created_at, &a.id).cmp(&(b.line.unwrap_or(0), &b.created_at, &b.id)));
            ReviewFileView { path, comments }
        })
        .collect()
}
