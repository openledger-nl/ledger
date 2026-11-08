use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, html};

/// Renders user markdown to HTML that is safe to inject into the page: raw HTML in the source is shown as
/// text, and links or images with a scheme other than http(s)/mailto lose their target.
///
/// `[[target]]` and `[[target|label]]` become `<a class="wikilink" data-wikilink="target">`, with no address: what
/// a target points at (a thread, a deliverable, a piece of feedback) is the page's to find out, from the same list
/// the editor completes from (see `links.rs`), so that one set of rules serves both.
pub fn render(source: &str) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_WIKILINKS;
    let events = Parser::new_ext(source, options).map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(Tag::Link { link_type: LinkType::WikiLink { has_pothole }, dest_url, .. }) => {
            let alias = if has_pothole { " data-alias=\"1\"" } else { "" };
            Event::Html(format!("<a class=\"wikilink\" data-wikilink=\"{}\"{alias}>", escape_attribute(&dest_url)).into())
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
fn safe_url(url: CowStr<'_>) -> CowStr<'_> {
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
