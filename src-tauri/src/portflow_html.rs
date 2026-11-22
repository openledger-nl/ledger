//! Portflow keeps its texts (collection descriptions, comments, changelogs, outcome explanations) as HTML from its
//! editor (TipTap): paragraphs, headings, bold and italic, links, lists, quotes, code, line breaks, and `@` mentions
//! as `<span data-type="mention" data-id="…" data-label="…">`. Ledger keeps markdown, so this turns that HTML into
//! markdown. It is a small reader for that well-formed subset, not a general HTML parser: an element it doesn't know
//! keeps its text and loses its markup, and nothing in the HTML is ever run or kept as HTML.

/// A mention found in the text: what it points at in Portflow, and its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    /// "evidence", "goal", "sub_goal", "user", … as Portflow names it (`data-mention-type` or the id's prefix).
    pub kind: String,
    pub id: String,
    pub label: String,
}

#[derive(Debug)]
enum Token {
    Open { name: String, attrs: Vec<(String, String)> },
    Close(String),
    Text(String),
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => {
                let code = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()));
                code.and_then(char::from_u32)
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn parse_attrs(text: &str) -> Vec<(String, String)> {
    let mut attrs = vec![];
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '/') {
            i += 1;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' && chars[i] != '/' {
            i += 1;
        }
        if start == i {
            i += 1;
            continue;
        }
        let name: String = chars[start..i].iter().collect::<String>().to_ascii_lowercase();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                let quote = chars[i];
                i += 1;
                let begin = i;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                value = chars[begin..i].iter().collect();
                i += 1;
            } else {
                let begin = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                value = chars[begin..i].iter().collect();
            }
        }
        attrs.push((name, decode_entities(&value)));
    }
    attrs
}

fn tokenize(html: &str) -> Vec<Token> {
    let mut tokens = vec![];
    let mut rest = html;
    while !rest.is_empty() {
        match rest.find('<') {
            Some(0) => {
                if let Some(after) = rest.strip_prefix("<!--") {
                    rest = after.find("-->").map_or("", |end| &after[end + 3..]);
                    continue;
                }
                let Some(end) = rest.find('>') else {
                    tokens.push(Token::Text(decode_entities(rest)));
                    break;
                };
                let inner = &rest[1..end];
                rest = &rest[end + 1..];
                if let Some(name) = inner.strip_prefix('/') {
                    tokens.push(Token::Close(name.trim().to_ascii_lowercase()));
                } else if !inner.starts_with('!') && !inner.starts_with('?') {
                    let split = inner.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(inner.len());
                    let name = inner[..split].to_ascii_lowercase();
                    tokens.push(Token::Open { name, attrs: parse_attrs(&inner[split..]) });
                }
            }
            Some(at) => {
                tokens.push(Token::Text(decode_entities(&rest[..at])));
                rest = &rest[at..];
            }
            None => {
                tokens.push(Token::Text(decode_entities(rest)));
                break;
            }
        }
    }
    tokens
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
}

/// Characters that would start markdown syntax are escaped, so Portflow's text stays text.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '#' | '|') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

struct Writer {
    blocks: Vec<String>,
    line: String,
    /// The list we're in, innermost last: `None` for a bullet list, `Some(n)` for a numbered one at item n.
    lists: Vec<Option<usize>>,
    quote: usize,
    link: Vec<Option<String>>,
    pre: bool,
    mention: Option<String>,
    mentions: Vec<Mention>,
    /// A mention as a token that keeps its id (`[@Label](portflow:…)`, for a reading guide), or as a bold label.
    tokens: bool,
}

impl Writer {
    fn prefix(&self) -> String {
        "> ".repeat(self.quote)
    }

    fn flush(&mut self) {
        let text = self.line.trim_end().to_string();
        self.line.clear();
        if text.trim().is_empty() {
            return;
        }
        let prefix = self.prefix();
        let block = text.lines().map(|l| if l.trim().is_empty() { prefix.trim_end().to_string() } else { format!("{prefix}{l}") }).collect::<Vec<_>>().join("\n");
        self.blocks.push(block);
    }

    fn text(&mut self, text: &str) {
        if self.mention.is_some() {
            return;
        }
        if self.pre {
            self.line.push_str(text);
            return;
        }
        let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            if !text.is_empty() && !self.line.is_empty() && !self.line.ends_with([' ', '\n']) {
                self.line.push(' ');
            }
            return;
        }
        if text.starts_with(char::is_whitespace) && !self.line.is_empty() && !self.line.ends_with([' ', '\n']) {
            self.line.push(' ');
        }
        self.line.push_str(&escape(&collapsed));
        if text.ends_with(char::is_whitespace) {
            self.line.push(' ');
        }
    }
}

const LIST_END: &str = "\u{0}";

/// Portflow's HTML as markdown, with the mentions it holds (shown as bold labels).
pub fn to_markdown_with_mentions(html: &str) -> (String, Vec<Mention>) {
    convert(html, false)
}

/// A reading guide from a collection's description: each mention becomes a token that keeps what it points at
/// (`[@Analysis](portflow:evidence-2110705)`), so it can be followed in Ledger and goes back to Portflow unchanged.
pub fn to_guide_markdown(html: &str) -> String {
    convert(html, true).0
}

fn convert(html: &str, tokens: bool) -> (String, Vec<Mention>) {
    let mut w = Writer { blocks: vec![], line: String::new(), lists: vec![], quote: 0, link: vec![], pre: false, mention: None, mentions: vec![], tokens };
    for token in tokenize(html) {
        match token {
            Token::Text(text) => w.text(&text),
            Token::Open { name, attrs } => match name.as_str() {
                "p" | "div" if !w.lists.is_empty() => {
                    // A paragraph in a list item continues the item.
                    if !w.line.trim().is_empty() && !w.line.trim_end().ends_with(['-', '.']) {
                        let indent = "   ".repeat(w.lists.len());
                        w.line.push_str(&format!("  \n{indent}"));
                    }
                }
                "p" | "div" => w.flush(),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    w.flush();
                    let level = name[1..].parse::<usize>().unwrap_or(2);
                    w.line.push_str(&"#".repeat(level));
                    w.line.push(' ');
                }
                "br" => w.line.push_str(if w.pre { "\n" } else { "  \n" }),
                "hr" => {
                    w.flush();
                    w.blocks.push("---".into());
                }
                "strong" | "b" => w.line.push_str("**"),
                "em" | "i" => w.line.push('*'),
                "s" | "del" | "strike" => w.line.push_str("~~"),
                "code" if !w.pre => w.line.push('`'),
                "pre" => {
                    w.flush();
                    w.pre = true;
                    w.line.push_str("```\n");
                }
                "blockquote" => {
                    w.flush();
                    w.quote += 1;
                }
                "ul" => {
                    w.flush();
                    w.lists.push(None);
                }
                "ol" => {
                    w.flush();
                    let start = attr(&attrs, "start").and_then(|s| s.parse().ok()).unwrap_or(1);
                    w.lists.push(Some(start));
                }
                "li" => {
                    w.flush();
                    let depth = w.lists.len().saturating_sub(1);
                    let marker = match w.lists.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}.");
                            *n += 1;
                            m
                        }
                        _ => "-".to_string(),
                    };
                    w.line.push_str(&"   ".repeat(depth));
                    w.line.push_str(&marker);
                    w.line.push(' ');
                }
                "a" => {
                    let href = attr(&attrs, "href").filter(|h| h.starts_with("http://") || h.starts_with("https://") || h.starts_with("mailto:"));
                    if href.is_some() {
                        w.line.push('[');
                    }
                    w.link.push(href.map(str::to_string));
                }
                "span" if attr(&attrs, "data-type") == Some("mention") => {
                    let id = attr(&attrs, "data-id").unwrap_or_default().to_string();
                    let label = attr(&attrs, "data-label").unwrap_or_default().to_string();
                    let kind = attr(&attrs, "data-mention-type").map(str::to_string).unwrap_or_else(|| kind_of(&id));
                    let shown = if label.is_empty() { id.clone() } else { label.clone() };
                    let mention = Mention { kind, id, label: shown.clone() };
                    let text = if w.tokens { mention_token(&mention) } else { format!("**@{}**", escape(&shown)) };
                    w.line.push_str(&text);
                    w.mentions.push(mention);
                    w.mention = Some(shown);
                }
                "img" => {
                    let alt = attr(&attrs, "alt").unwrap_or("image");
                    if let Some(src) = attr(&attrs, "src").filter(|s| s.starts_with("https://")) {
                        w.line.push_str(&format!("[{}]({src})", escape(alt)));
                    }
                }
                _ => {}
            },
            Token::Close(name) => match name.as_str() {
                "p" | "div" if !w.lists.is_empty() => {}
                "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => w.flush(),
                "strong" | "b" => w.line.push_str("**"),
                "em" | "i" => w.line.push('*'),
                "s" | "del" | "strike" => w.line.push_str("~~"),
                "code" if !w.pre => w.line.push('`'),
                "pre" => {
                    if !w.line.ends_with('\n') {
                        w.line.push('\n');
                    }
                    w.line.push_str("```");
                    w.pre = false;
                    // A code block keeps its lines exactly.
                    let text = std::mem::take(&mut w.line);
                    let prefix = w.prefix();
                    w.blocks.push(text.lines().map(|l| format!("{prefix}{l}")).collect::<Vec<_>>().join("\n"));
                }
                "blockquote" => {
                    w.flush();
                    w.quote = w.quote.saturating_sub(1);
                }
                "ul" | "ol" => {
                    w.flush();
                    w.lists.pop();
                    if w.lists.is_empty() {
                        // Two lists in a row must not run into one.
                        w.blocks.push(LIST_END.to_string());
                    }
                }
                "li" => w.flush(),
                "a" => {
                    if let Some(Some(href)) = w.link.pop() {
                        w.line.push_str(&format!("]({href})"));
                    }
                }
                "span" if w.mention.is_some() => w.mention = None,
                _ => {}
            },
        }
    }
    w.flush();

    // List items follow each other without a blank line between them; other blocks get one.
    let mut out = String::new();
    let is_item = |b: &str| {
        let t = b.trim_start_matches("> ").trim_start();
        t.starts_with("- ") || t.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    };
    let mut previous: Option<&str> = None;
    for block in &w.blocks {
        if block == LIST_END {
            previous = Some(LIST_END);
            continue;
        }
        if let Some(prev) = previous {
            out.push_str(if is_item(block) && is_item(prev) { "\n" } else { "\n\n" });
        }
        out.push_str(block);
        previous = Some(block);
    }
    (out.trim().to_string(), w.mentions)
}

/// Portflow's HTML as markdown.
pub fn to_markdown(html: &str) -> String {
    to_markdown_with_mentions(html).0
}

// ---- mentions in a reading guide -------------------------------------------------------------------
// In the markdown a mention is a link with its own scheme: `[@Analysis](portflow:evidence-2110705)`. It keeps Portflow's
// id exactly (and its kind, as `?type=…`, only when that isn't the id's own prefix), reads as `@Analysis` in Obsidian,
// and the renderer draws it as a chip (markdown.rs). On the way back it is the same TipTap span again.

/// What a link to a mention starts with.
pub const MENTION_SCHEME: &str = "portflow:";

/// The kind an id names by its prefix: `evidence-2110705` → `evidence`; `""` for a bare number.
fn kind_of(id: &str) -> String {
    id.split(['-', ':', '_']).next().filter(|p| !p.is_empty() && p.parse::<i64>().is_err()).unwrap_or("").to_string()
}

/// The number at the end of an id (`evidence-2110705` → 2110705): how a mention is matched to an evidence item.
pub fn mention_number(id: &str) -> Option<i64> {
    id.rsplit(['-', ':', '_']).next()?.parse().ok()
}

/// `[@Label](portflow:<id>)`, with `?type=<kind>` when the kind isn't the id's prefix.
pub fn mention_token(m: &Mention) -> String {
    let mut url = format!("{MENTION_SCHEME}{}", encode(&m.id));
    if !m.kind.is_empty() && m.kind != kind_of(&m.id) {
        url.push_str(&format!("?type={}", encode(&m.kind)));
    }
    format!("[@{}]({url})", escape(&m.label))
}

/// The id and the kind in a mention link (`portflow:evidence-1?type=goal`); `None` for any other link.
pub fn parse_mention_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix(MENTION_SCHEME)?;
    let (id, query) = rest.split_once('?').unwrap_or((rest, ""));
    let id = decode(id);
    if id.is_empty() {
        return None;
    }
    let kind = query.split('&').find_map(|p| p.strip_prefix("type=")).map(decode).unwrap_or_else(|| kind_of(&id));
    Some((id, kind))
}

/// What can't stand in a link target as it is: white space, brackets, `?`, `&`, `%`.
fn encode(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_whitespace() || "()<>[]?&%#\\".contains(c) { format!("%{:02X}", c as u32) } else { c.to_string() })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(b) = text.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn attribute(text: &str) -> String {
    text.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A mention as Portflow's editor (TipTap) writes it.
pub fn mention_span(m: &Mention) -> String {
    let kind = if m.kind.is_empty() { kind_of(&m.id) } else { m.kind.clone() };
    let label = attribute(&m.label);
    format!(r#"<span data-type="mention" data-mention-type="{}" data-id="{}" data-label="{label}">@{label}</span>"#, attribute(&kind), attribute(&m.id))
}

/// A reading guide as HTML for Portflow: markdown rendered, a mention token turned back into its span, and a
/// `[[link]]` to a thread that holds an evidence item turned into a mention of that item (`threads`, keyed by the
/// thread's slug and its title, folded). Any other `[[link]]` keeps only its words: Portflow can't follow it.
pub fn to_portflow_html(markdown: &str, threads: &std::collections::HashMap<String, Mention>) -> String {
    use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_WIKILINKS;
    let mut events: Vec<Event> = vec![];
    // Inside a mention link (or a [[link]]): what it stands for, and the words in it.
    let mut inside: Option<(Option<Mention>, String)> = None;
    for event in Parser::new_ext(markdown, options) {
        if let Some((_, words)) = inside.as_mut() {
            match event {
                Event::End(TagEnd::Link) => {
                    let (mention, words) = inside.take().unwrap_or_default();
                    match mention {
                        Some(mut m) => {
                            if m.label.is_empty() {
                                m.label = words.trim().trim_start_matches('@').to_string();
                            }
                            events.push(Event::InlineHtml(mention_span(&m).into()));
                        }
                        None => events.push(Event::Text(words.into())),
                    }
                }
                Event::Text(t) | Event::Code(t) => words.push_str(&t),
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(Tag::Link { link_type: LinkType::WikiLink { .. }, dest_url, .. }) => {
                inside = Some((threads.get(&fold(&dest_url)).cloned(), String::new()));
            }
            Event::Start(Tag::Link { ref dest_url, .. }) if dest_url.starts_with(MENTION_SCHEME) => {
                let mention = parse_mention_url(dest_url).map(|(id, kind)| Mention { kind, id, label: String::new() });
                inside = Some((mention, String::new()));
            }
            Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
                events.push(Event::Start(Tag::Link { link_type, dest_url: crate::markdown::safe_url(dest_url), title, id }));
            }
            // Raw HTML in the guide is text, as everywhere else in Ledger.
            Event::Html(t) | Event::InlineHtml(t) => events.push(Event::Text(t)),
            other => events.push(other),
        }
    }
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events.into_iter());
    out
}

/// How a thread's slug or title is looked up: no difference made for case or runs of white space.
pub fn fold(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_marks_and_lists() {
        let html = "<p>Hi <strong>there</strong>, see <em>this</em>.</p><ul><li><p>one</p></li><li><p>two</p></li></ul><ol><li>first</li><li>second</li></ol>";
        assert_eq!(to_markdown(html), "Hi **there**, see *this*.\n\n- one\n- two\n\n1. first\n2. second");
    }

    #[test]
    fn headings_links_breaks_and_entities() {
        let html = "<h2>Plan &amp; scope</h2><p>Line one<br>line two &lt;3 <a href=\"https://example.com/x\">link</a></p>";
        assert_eq!(to_markdown(html), "## Plan & scope\n\nLine one  \nline two \\<3 [link](https://example.com/x)");
    }

    #[test]
    fn scripts_and_odd_links_are_text_only() {
        let html = "<p><a href=\"javascript:alert(1)\">click</a><script>bad()</script></p>";
        assert_eq!(to_markdown(html), "clickbad()");
    }

    #[test]
    fn mentions_become_bold_labels() {
        let html = r#"<p>See <span data-type="mention" data-id="evidence-2110705" data-label="Project plan">@Project plan</span> now</p>"#;
        let (md, mentions) = to_markdown_with_mentions(html);
        assert_eq!(md, "See **@Project plan** now");
        assert_eq!(mentions, vec![Mention { kind: "evidence".into(), id: "evidence-2110705".into(), label: "Project plan".into() }]);
    }

    #[test]
    fn a_guide_keeps_each_mention_and_gives_back_the_same_span() {
        let span = r#"<span data-type="mention" data-mention-type="evidence" data-id="evidence-2110705" data-label="Project plan">@Project plan</span>"#;
        let html = format!("<p>See {span} first.</p>");
        let md = to_guide_markdown(&html);
        assert_eq!(md, "See [@Project plan](portflow:evidence-2110705) first.");
        let back = to_portflow_html(&md, &std::collections::HashMap::new());
        assert_eq!(back.trim(), format!("<p>See {span} first.</p>"));
        // A kind that isn't the id's prefix is kept with it.
        let goal = r#"<p><span data-type="mention" data-mention-type="goal" data-id="96397" data-label="Analysis">@Analysis</span></p>"#;
        let md = to_guide_markdown(goal);
        assert_eq!(md, "[@Analysis](portflow:96397?type=goal)");
        assert_eq!(to_portflow_html(&md, &std::collections::HashMap::new()).trim(), goal);
    }

    #[test]
    fn a_link_to_an_evidence_thread_becomes_its_mention_and_other_links_their_words() {
        let mut threads = std::collections::HashMap::new();
        threads.insert(fold("Plan"), Mention { kind: "evidence".into(), id: "evidence-700".into(), label: "Project plan".into() });
        let html = to_portflow_html("Read [[Plan]], then [[Nothing here|the rest]].", &threads);
        assert_eq!(
            html.trim(),
            r#"<p>Read <span data-type="mention" data-mention-type="evidence" data-id="evidence-700" data-label="Project plan">@Project plan</span>, then the rest.</p>"#
        );
        // Raw HTML stays text, and a script link is dropped, on the way to Portflow too.
        let html = to_portflow_html("<b>x</b> [go](javascript:alert(1))", &threads);
        assert!(!html.contains("<b>") && !html.contains("javascript"), "{html}");
    }

    #[test]
    fn mention_links_round_trip_their_ids() {
        assert_eq!(parse_mention_url("portflow:evidence-1"), Some(("evidence-1".into(), "evidence".into())));
        assert_eq!(parse_mention_url("portflow:a%20b?type=user"), Some(("a b".into(), "user".into())));
        assert_eq!(parse_mention_url("https://x"), None);
        assert_eq!(mention_number("evidence-2110705"), Some(2110705));
    }

    #[test]
    fn quotes_and_code() {
        let html = "<blockquote><p>quoted</p></blockquote><pre><code>let x = 1;\nlet y = 2;</code></pre>";
        assert_eq!(to_markdown(html), "> quoted\n\n```\nlet x = 1;\nlet y = 2;\n```");
    }

    #[test]
    fn markdown_characters_are_escaped() {
        assert_eq!(to_markdown("<p>a_b *c* [d]</p>"), "a\\_b \\*c\\* \\[d\\]");
    }
}
