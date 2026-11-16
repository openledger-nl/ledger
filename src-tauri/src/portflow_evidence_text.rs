//! An evidence item's description, as teachers like it:
//!
//! ```text
//! Evidence description
//!
//! What? A written plan with the context, the main question and three sub questions, …
//!
//! Goal? To give my expert, the stakeholder and myself a clear picture of what I research and deliver this semester.
//!
//! Why it contributes? It shows that I analyse the problem and plan my research with the DOT framework before I build, …
//! ```
//!
//! Ledger keeps it as the body of the evidence item's file (`projects/<p>/evidence/<slug>.md`), in markdown, written the
//! one way above when it has that shape (`**What?** …`), and as it came otherwise. The body is the plan's place for it
//! (one text per item, readable in Obsidian), and what the shape needs is three labels at the start of their paragraphs,
//! so it is told from the text itself rather than kept twice.
//!
//! It is kept in step with the item's description in Portflow the way a reading guide is with its collection's
//! (`portflow_sync::sync_guide`): what both sides last agreed on is remembered as hashes in `.ledger/portflow-guides.json`
//! (`GuideMemory::evidence`). A change made only in Portflow comes in (in the shape when it has it); one made only in
//! Ledger goes to Portflow with the "Evidence description → evidence item" write on, and waits in Ledger with it off;
//! when both changed, Ledger's text is kept (and sent with the write on, a note in the report with it off). The first
//! round, Portflow's description comes in when Ledger has none yet, and an item Ledger already describes keeps its text.
//! An empty text never empties the other side. Mentions keep their ids both ways (`portflow_html`).

use std::collections::HashMap;
use std::path::Path;

use crate::portflow_api::{PResult, Portflow, PortflowError};
use crate::portflow_config::{GuideMemory, GuideSeen};
use crate::portflow_html::{self, Mention};

/// The first line of a description in the shape.
pub const TITLE: &str = "Evidence description";

/// The three parts, in markdown (mentions as tokens), each as written after its label.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Parts {
    pub what: String,
    pub goal: String,
    pub why: String,
}

const LABELS: [&str; 3] = ["What?", "Goal?", "Why it contributes?"];

impl Parts {
    /// Markdown, as Ledger keeps it: the title, then each label in bold at the start of its paragraph. A part left
    /// empty is left out.
    pub fn to_markdown(&self) -> String {
        let mut out = vec![format!("**{TITLE}**")];
        for (label, text) in LABELS.iter().zip([&self.what, &self.goal, &self.why]) {
            let text = text.trim();
            if !text.is_empty() {
                out.push(format!("**{label}** {text}"));
            }
        }
        out.join("\n\n")
    }
}

/// A label at the start of a paragraph, however it is dressed: `What?`, `**What?**`, `### What?`, `What:`,
/// `**What?:**`; with the rest of the paragraph.
fn label_of(paragraph: &str) -> Option<(usize, String)> {
    let mut rest = paragraph.trim_start().trim_start_matches('#').trim_start();
    let bold = rest.starts_with("**") || rest.starts_with("__");
    if bold {
        rest = &rest[2..];
    }
    let lower = rest.to_lowercase();
    for (i, label) in LABELS.iter().enumerate() {
        let word = label.trim_end_matches('?').to_lowercase();
        if !lower.starts_with(&word) {
            continue;
        }
        let mut after = &rest[word.len()..];
        // The label ends in "?" or ":" (or both), and may be closed in bold after that.
        let mut ended = false;
        for mark in ['?', ':'] {
            if let Some(next) = after.trim_start().strip_prefix(mark) {
                after = next;
                ended = true;
            }
        }
        if bold {
            after = after.trim_start().strip_prefix("**").or_else(|| after.trim_start().strip_prefix("__")).unwrap_or(after);
            for mark in ['?', ':'] {
                if let Some(next) = after.trim_start().strip_prefix(mark) {
                    after = next;
                    ended = true;
                }
            }
        }
        if ended {
            return Some((i, after.trim().to_string()));
        }
    }
    None
}

/// Whether a paragraph is the title, dressed or not.
fn is_title(paragraph: &str) -> bool {
    let bare: String = paragraph.trim().trim_start_matches('#').trim().trim_matches(['*', '_', ':']).trim().to_string();
    bare.eq_ignore_ascii_case(TITLE)
}

/// The three parts of a description in the shape: an optional title, then paragraphs that each start with a label
/// (What?, Goal?, Why it contributes?), the text after one running on until the next. `None` when it isn't in the shape:
/// text before the first label, or no label at all.
pub fn parse(markdown: &str) -> Option<Parts> {
    let text = markdown.replace("\r\n", "\n");
    let mut parts = [String::new(), String::new(), String::new()];
    let mut current: Option<usize> = None;
    let mut found = false;
    for paragraph in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if let Some((i, rest)) = label_of(paragraph) {
            found = true;
            current = Some(i);
            if !parts[i].is_empty() && !rest.is_empty() {
                parts[i].push_str("\n\n");
            }
            parts[i].push_str(&rest);
        } else if current.is_none() && !found && is_title(paragraph) {
            continue;
        } else if let Some(i) = current {
            if !parts[i].is_empty() {
                parts[i].push_str("\n\n");
            }
            parts[i].push_str(paragraph);
        } else {
            return None;
        }
    }
    let [what, goal, why] = parts;
    found.then_some(Parts { what, goal, why })
}

/// The description as Ledger keeps it: in the shape when it has one (written the one way), as it is otherwise.
pub fn canonical(markdown: &str) -> String {
    match parse(markdown) {
        Some(parts) => parts.to_markdown(),
        None => markdown.trim().to_string(),
    }
}

/// Ledger's text as HTML for Portflow: `<p><strong>Evidence description</strong></p>` and a paragraph per part, each
/// starting with its label in bold; mentions back as Portflow's spans.
pub fn to_html(markdown: &str) -> String {
    portflow_html::to_portflow_html(&canonical(markdown), &HashMap::<String, Mention>::new())
}

/// Portflow's description as Ledger keeps it: markdown with mention tokens, in the shape when it has one.
pub fn from_html(html: &str) -> String {
    canonical(&portflow_html::to_guide_markdown(html))
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").replace("> <", "><")
}

fn html_hash(html: &str) -> String {
    crate::provenance::sha256_hex(squash(html).as_bytes())
}

fn text_hash(text: &str) -> String {
    crate::provenance::sha256_hex(text.replace("\r\n", "\n").trim().as_bytes())
}

/// What one evidence item's description looks like on both sides this round.
pub struct Item<'a> {
    pub project: &'a str,
    pub name: &'a str,
    /// Its id in Portflow.
    pub portflow: i64,
    /// Ledger's text (the body of its file).
    pub ledger: &'a str,
    /// Portflow's description, HTML.
    pub portflow_html: &'a str,
}

/// What a round did with one item's description.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Portflow's description, as Ledger keeps it, to be written to the item's file.
    pub take: Option<String>,
    /// Ledger's text went to Portflow.
    pub written: bool,
    pub note: Option<String>,
}

/// Keeps one evidence item's description in step (the rules are in this module's docs). `write_on`: the "Evidence
/// description → evidence item" write is on. Only what Ledger should store is returned; what is remembered is updated
/// in `memory` and written to `.ledger/` here.
pub fn sync(portflow: &dyn Portflow, p: i64, item: &Item, write_on: bool, memory: &mut GuideMemory, root: &Path) -> PResult<Outcome> {
    let key = format!("{}/{}", item.project, item.portflow);
    let now = GuideSeen { description: html_hash(item.portflow_html), readme: text_hash(item.ledger) };
    let seen = memory.evidence.get(&key).cloned();
    let remember = |memory: &mut GuideMemory, agreed: GuideSeen| -> PResult<()> {
        if memory.evidence.get(&key) != Some(&agreed) {
            memory.evidence.insert(key.clone(), agreed);
            crate::portflow_config::write_guides(root, memory).map_err(PortflowError::Ledger)?;
        }
        Ok(())
    };
    let pulled = from_html(item.portflow_html);
    let ledger = item.ledger.trim();

    // Saying the same thing is agreeing, whichever side changed.
    if (ledger.is_empty() && pulled.trim().is_empty()) || (!ledger.is_empty() && (pulled == canonical(ledger) || squash(&to_html(ledger)) == squash(item.portflow_html))) {
        remember(memory, now)?;
        return Ok(Outcome::default());
    }
    let (in_portflow, in_ledger) = match &seen {
        Some(s) => (s.description != now.description, s.readme != now.readme),
        None if ledger.is_empty() => (true, false),
        None if pulled.trim().is_empty() => (false, true),
        // The first round for an item both sides describe differently: Ledger's text is kept, as for a guide edited
        // in Ledger, so nothing written in Ledger is lost.
        None => (true, true),
    };

    if in_portflow && !in_ledger {
        if pulled.trim().is_empty() {
            remember(memory, now)?;
            return Ok(Outcome::default());
        }
        remember(memory, GuideSeen { description: now.description, readme: text_hash(&pulled) })?;
        crate::applog::info("portflow", format!("Took the description of the evidence item “{}” in {} from Portflow", item.name, item.project));
        return Ok(Outcome { take: Some(pulled), ..Default::default() });
    }
    if !in_ledger || ledger.is_empty() {
        // Nothing new, or emptied in Ledger: the description in Portflow stays, and a later change there comes in.
        remember(memory, now)?;
        return Ok(Outcome::default());
    }
    if !write_on {
        let note = in_portflow.then(|| {
            format!(
                "The description of the evidence item “{}” in {} changed in Ledger and in Portflow: Ledger's is kept. Switch on “Evidence description → evidence item” to send it.",
                item.name, item.project
            )
        });
        // What Portflow has now is seen; Ledger's text stays changed until it is sent.
        let readme = seen.map(|s| s.readme).unwrap_or_default();
        remember(memory, GuideSeen { description: now.description, readme })?;
        return Ok(Outcome { note, ..Default::default() });
    }
    let html = to_html(ledger);
    match portflow.set_evidence_description(p, item.portflow, &html) {
        Ok(answer) => {
            let stored = answer.unwrap_or_else(|| html.clone());
            remember(memory, GuideSeen { description: html_hash(&stored), readme: now.readme })?;
            crate::applog::info("portflow", format!("Wrote the description of the evidence item “{}” in {} to Portflow", item.name, item.project));
            Ok(Outcome { written: true, ..Default::default() })
        }
        Err(PortflowError::SignedOut) => Err(PortflowError::SignedOut),
        Err(e) => Ok(Outcome { note: Some(format!("The description of the evidence item “{}” in {} didn't go to Portflow: {e}", item.name, item.project)), ..Default::default() }),
    }
}

/// One evidence item of a round (`portflow_sync::run`, after the item itself is stored): its description in Portflow
/// (from the list, or asked for when the list leaves it out) and in Ledger, kept in step by `sync`.
#[allow(clippy::too_many_arguments)]
pub fn round(
    portflow: &dyn Portflow,
    ledger: &dyn crate::portflow_sync::Ledger,
    p: i64,
    project: &str,
    evidence: &crate::portflow_api::Evidence,
    write_on: bool,
    memory: &mut GuideMemory,
    root: &Path,
    report: &mut crate::portflow_sync::Report,
) -> PResult<()> {
    let mut found: Option<(String, String)> = None;
    ledger
        .read(&mut |conn| {
            found = match crate::portflow_store::evidence_slug_for(conn, project, evidence.id)? {
                Some(slug) => {
                    let text: String = conn.query_row("SELECT description FROM evidence WHERE project_slug = ?1 AND slug = ?2", [project, slug.as_str()], |r| r.get(0))?;
                    Some((slug, text))
                }
                None => None,
            };
            Ok(())
        })
        .map_err(PortflowError::Ledger)?;
    let Some((slug, text)) = found else { return Ok(()) };
    let html = match &evidence.description {
        Some(html) => html.clone(),
        None => match portflow.evidence_item(p, evidence.id) {
            Ok(item) => item.description.unwrap_or_default(),
            Err(PortflowError::SignedOut) => return Err(PortflowError::SignedOut),
            // Portflow didn't say: nothing to compare, so nothing is done this round.
            Err(e) => {
                crate::applog::debug("portflow", format!("The description of evidence item {} wasn't read: {e}", evidence.id));
                return Ok(());
            }
        },
    };
    let item = Item { project, name: &evidence.name, portflow: evidence.id, ledger: &text, portflow_html: &html };
    let outcome = sync(portflow, p, &item, write_on, memory, root)?;
    if let Some(take) = outcome.take {
        report.files_touched += ledger
            .write(&mut |conn, _| crate::portflow_store::set_evidence_description(conn, project, &slug, &take))
            .map_err(PortflowError::Ledger)?;
        report.descriptions_read += 1;
    }
    if outcome.written {
        report.descriptions_written += 1;
    }
    report.notes.extend(outcome.note);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEACHERS: &str = "Evidence description\n\nWhat? A written plan with the context, the main question and three sub questions.\n\nGoal? To give my expert a clear picture.\n\nWhy it contributes? It shows that I analyse the problem.";

    #[test]
    fn the_teachers_shape_reads_into_three_parts_and_is_written_one_way() {
        let parts = parse(TEACHERS).unwrap();
        assert_eq!(parts.what, "A written plan with the context, the main question and three sub questions.");
        assert_eq!(parts.goal, "To give my expert a clear picture.");
        assert_eq!(parts.why, "It shows that I analyse the problem.");
        let md = parts.to_markdown();
        assert_eq!(
            md,
            "**Evidence description**\n\n**What?** A written plan with the context, the main question and three sub questions.\n\n**Goal?** To give my expert a clear picture.\n\n**Why it contributes?** It shows that I analyse the problem."
        );
        assert_eq!(parse(&md).unwrap(), parts, "the way Ledger writes it reads back the same");
        // Dressed differently: headings, colons, no title, bold labels with the mark inside or out.
        let other = "### What:\nA plan.\n\n**Goal**? Clarity.\n\n__Why it contributes?__ Shows analysis.";
        assert_eq!(parse(other).unwrap(), Parts { what: "A plan.".into(), goal: "Clarity.".into(), why: "Shows analysis.".into() });
    }

    #[test]
    fn free_text_is_kept_as_it_is() {
        assert_eq!(parse("Just a plan I wrote.\n\nWith two paragraphs."), None);
        assert_eq!(canonical("Just a plan I wrote."), "Just a plan I wrote.");
        assert_eq!(parse("Intro first.\n\nWhat? Then the label."), None, "text before the first label isn't the shape");
    }

    #[test]
    fn it_goes_to_portflow_in_the_shape_and_comes_back_the_same() {
        let html = to_html(TEACHERS);
        assert!(html.starts_with("<p><strong>Evidence description</strong></p>"), "{html}");
        assert!(html.contains("<p><strong>What?</strong> A written plan"), "{html}");
        assert!(html.contains("<p><strong>Goal?</strong> To give my expert"), "{html}");
        assert!(html.contains("<p><strong>Why it contributes?</strong> It shows"), "{html}");
        assert_eq!(from_html(&html), canonical(TEACHERS));
        // As Portflow's editor might hold it: plain paragraphs, no bold.
        let plain = "<p>Evidence description</p><p>What? A plan.</p><p>Goal? Clarity.</p><p>Why it contributes? Shows analysis.</p>";
        assert_eq!(from_html(plain), "**Evidence description**\n\n**What?** A plan.\n\n**Goal?** Clarity.\n\n**Why it contributes?** Shows analysis.");
    }

    #[test]
    fn mentions_keep_their_ids_both_ways() {
        let html = r#"<p>What? See <span data-type="mention" data-mention-type="evidence" data-id="2177688" data-label="Project plan DAE">@Project plan DAE</span>.</p>"#;
        let md = from_html(html);
        assert!(md.contains("[@Project plan DAE](portflow:2177688"), "{md}");
        let back = to_html(&md);
        assert!(back.contains(r#"data-id="2177688""#) && back.contains(r#"data-label="Project plan DAE""#), "{back}");
    }
}
