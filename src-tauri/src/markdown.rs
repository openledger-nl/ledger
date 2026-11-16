use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd, html};

use crate::{dot, frontmatter};

/// What a link to a person mentioned in a meeting's notes starts with: `person:<tag>` (`person:coach/frank`).
pub const PERSON_SCHEME: &str = "person:";

/// Renders user markdown to HTML that is safe to inject into the page: raw HTML in the source is shown as
/// text, and links or images with a scheme other than http(s)/mailto lose their target.
///
/// `[[target]]` and `[[target|label]]` become `<a class="wikilink" data-wikilink="target">`, with no address: what
/// a target points at (a thread, a deliverable, a piece of feedback) is the page's to find out, from the same list
/// the editor completes from (see `links.rs`), so that one set of rules serves both.
pub fn render(source: &str) -> String {
    render_with(source, None)
}

/// A whole document (a deliverable): its frontmatter left out, and a line of its own reading `!dotmethod` shown as the
/// table of the research methods its frontmatter plans (see `dot.rs`).
pub fn render_document(text: &str) -> String {
    match frontmatter::parse(text) {
        Some(parsed) => render_with(&parsed.body, Some(&dot::plan_of(&parsed.meta))),
        None => render_with(text, Some(&[])),
    }
}

/// `render`, and with a `plan`, `!dotmethod` paragraphs become its table.
fn render_with(source: &str, plan: Option<&[dot::PlannedMethod]>) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_WIKILINKS;
    let events = Parser::new_ext(source, options).map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(Tag::Link { link_type: LinkType::WikiLink { has_pothole }, dest_url, .. }) => {
            let alias = if has_pothole { " data-alias=\"1\"" } else { "" };
            Event::Html(format!("<a class=\"wikilink\" data-wikilink=\"{}\"{alias}>", escape_attribute(&dest_url)).into())
        }
        // A Portflow mention in a reading guide (`[@Analysis](portflow:evidence-2110705)`): a chip with no address, which
        // the page points at the evidence item's thread, or at Portflow (Markdown.svelte).
        Event::Start(Tag::Link { dest_url, .. }) if dest_url.starts_with(crate::portflow_html::MENTION_SCHEME) => {
            let (id, kind) = crate::portflow_html::parse_mention_url(&dest_url).unwrap_or_default();
            Event::Html(format!("<a class=\"mention\" data-mention=\"{}\" data-mention-type=\"{}\">", escape_attribute(&id), escape_attribute(&kind)).into())
        }
        // A person mentioned in a meeting's notes (`[@Frank](person:coach/frank)`): a chip the page points at the
        // feedback that person gave (Markdown.svelte).
        Event::Start(Tag::Link { dest_url, .. }) if dest_url.starts_with(PERSON_SCHEME) => {
            Event::Html(format!("<a class=\"mention person\" data-person=\"{}\">", escape_attribute(&dest_url[PERSON_SCHEME.len()..])).into())
        }
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Link { link_type, dest_url: safe_url(dest_url), title, id })
        }
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Image { link_type, dest_url: safe_url(dest_url), title, id })
        }
        other => other,
    });

    let mut out = String::with_capacity(source.len() * 3 / 2);
    match plan {
        Some(plan) => html::push_html(&mut out, with_dot_tables(events.collect(), plan).into_iter()),
        None => html::push_html(&mut out, events),
    }
    out
}

/// Each paragraph that says only `!dotmethod` (in a code block it stays as written) replaced by the plan's table.
fn with_dot_tables<'a>(events: Vec<Event<'a>>, plan: &[dot::PlannedMethod]) -> Vec<Event<'a>> {
    let mut out = Vec::with_capacity(events.len());
    let mut i = 0;
    while i < events.len() {
        if matches!(events[i], Event::Start(Tag::Paragraph)) {
            // The text may come in pieces (`!` can start a picture), so it is put together first.
            let mut text = String::new();
            let mut j = i + 1;
            while let Some(Event::Text(piece)) = events.get(j) {
                text.push_str(piece);
                j += 1;
            }
            if matches!(events.get(j), Some(Event::End(TagEnd::Paragraph))) && text.trim().eq_ignore_ascii_case(dot::TABLE_COMMAND) {
                out.push(Event::Html(dot::table_html(plan).into()));
                i = j + 1;
                continue;
            }
        }
        out.push(events[i].clone());
        i += 1;
    }
    out
}

fn escape_attribute(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Keeps http(s), mailto, in-page anchors and scheme-less (relative) targets; everything else — notably
/// `javascript:` and `data:` — is dropped. Whitespace and control characters are ignored when looking
/// for a scheme, since browsers ignore them too.
pub(crate) fn safe_url(url: CowStr<'_>) -> CowStr<'_> {
    let probe: String = url
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_lowercase();

    let allowed = probe.starts_with("http://")
        || probe.starts_with("https://")
        || probe.starts_with("mailto:")
        || probe.starts_with('#')
        || !probe.contains(':');

    if allowed { url } else { CowStr::Borrowed("") }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_person_mentioned_in_notes_is_a_chip_with_their_tag() {
        let html = super::render("Asked [@Frank de Vries](person:coach/frank) about it.");
        assert_eq!(html, "<p>Asked <a class=\"mention person\" data-person=\"coach/frank\">@Frank de Vries</a> about it.</p>\n");
    }

    #[test]
    fn dotmethod_on_a_line_of_its_own_shows_the_plan() {
        let doc = "---\ndot:\n- method: Survey\n  why: Ask many\n---\n\n# Report\n\n!dotmethod\n\nSay !dotmethod here.\n\n```\n!dotmethod\n```\n";
        let html = super::render_document(doc);
        assert_eq!(html.matches("<table class=\"dot-plan\">").count(), 1, "{html}");
        assert!(html.contains("<td>Ask many</td>") && html.contains("Say !dotmethod here.") && html.contains("<code>!dotmethod\n</code>"), "{html}");
        assert!(!html.contains("dot:"), "the frontmatter is not shown: {html}");
        // Outside a document it is just text, and in one without a plan it says how to make one.
        assert_eq!(super::render("!dotmethod"), "<p>!dotmethod</p>\n");
        assert!(super::render_document("!dotmethod").contains("dot-empty"));
    }

    #[test]
    fn a_person_mention_cannot_carry_markup() {
        let html = super::render("[@x](person:a\"onclick=\"b)");
        assert!(!html.contains("\"onclick"), "{html}");
    }
}
