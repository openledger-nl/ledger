use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd, html};

/// Renders user markdown to HTML that is safe to inject into the page: raw HTML in the source is shown as
/// text, and links or images with a scheme other than http(s)/mailto lose their target.
///
/// Notes between `%%` marks (Obsidian's comments: a template's guidance to the writer) are left out, as Obsidian's
/// reading view leaves them out; see `strip_comments`.
///
/// `[[target]]` and `[[target|label]]` become `<a class="wikilink" data-wikilink="target">`, with no address: what
/// a target points at (a thread, a deliverable, a piece of feedback) is the page's to find out, from the same list
/// the editor completes from (see `links.rs`), so that one set of rules serves both.
///
/// `![[target]]` (an embed, as Obsidian writes it) is the same link with `data-embed`: the page shows a picture in its
/// place, and anything else it points at (a PDF, a deliverable, a thread) stays a link to it.
pub fn render(source: &str) -> String {
    let source = &*strip_comments(source);
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_WIKILINKS;
    // Whether the picture being read is an embed, whose end is a link's end.
    let mut embed = false;
    let events = Parser::new_ext(source, options).map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(Tag::Image { link_type: LinkType::WikiLink { has_pothole }, dest_url, .. }) => {
            embed = true;
            let alias = if has_pothole { " data-alias=\"1\"" } else { "" };
            Event::Html(format!("<a class=\"wikilink\" data-wikilink=\"{}\" data-embed=\"1\"{alias}>", escape_attribute(&dest_url)).into())
        }
        Event::End(TagEnd::Image) if embed => {
            embed = false;
            Event::Html("</a>".into())
        }
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
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Link { link_type, dest_url: safe_url(dest_url), title, id })
        }
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Image { link_type, dest_url: safe_url(dest_url), title, id })
        }
        other => other,
    });

    let mut out = String::with_capacity(source.len() * 3 / 2);
    html::push_html(&mut out, events);
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

/// The text without its `%%` notes, as Obsidian reads it: from a `%%` to the next one, across lines, and to the end when
/// there is no next one. A `%%` in code (a fenced block, or between backticks on one line) is just text.
pub fn strip_comments(source: &str) -> std::borrow::Cow<'_, str> {
    if !source.contains("%%") {
        return std::borrow::Cow::Borrowed(source);
    }
    let mut out = String::with_capacity(source.len());
    let mut in_comment = false;
    let mut fence: Option<(u8, usize)> = None;
    for line in source.split_inclusive('\n') {
        if !in_comment {
            if let Some((mark, len)) = fence {
                if fence_of(line).is_some_and(|(m, l)| m == mark && l >= len && line.trim().bytes().all(|b| b == mark)) {
                    fence = None;
                }
                out.push_str(line);
                continue;
            }
            if let Some(opened) = fence_of(line) {
                fence = Some(opened);
                out.push_str(line);
                continue;
            }
        }
        // `%` and `` ` `` are ASCII, so every index cut at is a character boundary.
        let bytes = line.as_bytes();
        let (mut i, mut start) = (0, 0);
        while i < bytes.len() {
            let pair = bytes[i] == b'%' && bytes.get(i + 1) == Some(&b'%');
            if in_comment {
                if pair {
                    in_comment = false;
                    start = i + 2;
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            if bytes[i] == b'`' {
                let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
                i = closing_backticks(bytes, i + run, run).unwrap_or(i + run);
            } else if pair {
                out.push_str(&line[start..i]);
                in_comment = true;
                i += 2;
            } else {
                i += 1;
            }
        }
        if !in_comment {
            out.push_str(&line[start..]);
        }
    }
    std::borrow::Cow::Owned(out)
}

/// A line that opens or closes a code fence: its mark (`` ` `` or `~`) and how many of them.
pub(crate) fn fence_of(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let mark = *trimmed.as_bytes().first()?;
    let len = trimmed.bytes().take_while(|&b| b == mark).count();
    ((mark == b'`' || mark == b'~') && len >= 3).then_some((mark, len))
}

/// Just after the run of exactly `run` backticks that closes a code span opened before `from`, on this line.
fn closing_backticks(bytes: &[u8], from: usize, run: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let n = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            if n == run {
                return Some(i + n);
            }
            i += n;
        } else {
            i += 1;
        }
    }
    None
}
