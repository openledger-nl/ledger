//! A deliverable's markdown as Typst markup, for the PDF export. Everything the markdown says is written as a call
//! (`#heading(level: 2)[…]`, `#list([…], […])`, `#table(…)`), and every piece of text with its markup characters
//! escaped, so nothing a person typed can be read as Typst: a `#`, `$` or `*` in the text stays that character.
//!
//! The markdown is read as the app reads it ([`crate::markdown::render`]): `%%` notes are left out, `[[links]]` and
//! mentions are their words (a PDF has nowhere to send them). A first `# heading` is the document's title, which the
//! cover shows; when it is the only one of its level, the headings under it move up a level, so `##` chapters are
//! chapters. Pictures are handed to the caller, which says under what name the PDF can read each one.

use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};

/// The document as Typst, and what was learned on the way.
#[derive(Debug, Default, PartialEq)]
pub struct Converted {
    pub body: String,
    /// The text of the first heading, when the document starts with a `#` heading (it is not in `body`).
    pub title: Option<String>,
    /// How many headings `body` has, for whether a table of contents is worth it.
    pub headings: usize,
}

/// Converts `source`. `picture` is asked for each picture's address as written and gives the name the PDF reads it
/// under, or `None` when it can't be had (a web address, a file that isn't there): its alt text is shown instead.
pub fn to_typst(source: &str, picture: &mut dyn FnMut(&str) -> Option<String>) -> Converted {
    let source = crate::markdown::strip_comments(source);
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_WIKILINKS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let mut events: Vec<Event> = Parser::new_ext(&source, options).collect();

    // The title: a first-level heading before anything else.
    // Frontmatter is the caller's to read; it is not part of the text.
    events.drain(..metadata_end(&events));
    let mut title = None;
    let at = 0;
    if let Some(Event::Start(Tag::Heading { level: HeadingLevel::H1, .. })) = events.get(at) {
        let end = matching_end(&events, at);
        let text = plain_text(&events[at + 1..end]);
        if !text.trim().is_empty() {
            title = Some(text.trim().to_string());
            events.drain(at..=end);
        }
    }
    let shift = title.is_some() && !events.iter().any(|e| matches!(e, Event::Start(Tag::Heading { level: HeadingLevel::H1, .. })));

    // Footnotes are written where they are referred to, so their text is converted first.
    let mut notes = HashMap::new();
    let no_notes = HashMap::new();
    let mut i = 0;
    let mut body_events = Vec::with_capacity(events.len());
    while i < events.len() {
        if let Event::Start(Tag::FootnoteDefinition(label)) = &events[i] {
            let end = matching_end(&events, i);
            let mut writer = Writer::new(shift, &no_notes);
            writer.run(&events[i + 1..end], picture);
            notes.insert(label.to_string(), writer.finish().trim().to_string());
            i = end + 1;
        } else {
            body_events.push(events[i].clone());
            i += 1;
        }
    }

    let mut writer = Writer::new(shift, &notes);
    writer.run(&body_events, picture);
    let headings = writer.headings;
    Converted { body: writer.finish(), title, headings }
}

/// The index just after a leading YAML metadata block (a deliverable's frontmatter), or 0.
fn metadata_end(events: &[Event]) -> usize {
    match events.first() {
        Some(Event::Start(Tag::MetadataBlock(_))) => events.iter().position(|e| matches!(e, Event::End(TagEnd::MetadataBlock(_)))).map_or(0, |i| i + 1),
        _ => 0,
    }
}

/// The index of the `End` that closes the `Start` at `start`.
fn matching_end(events: &[Event], start: usize) -> usize {
    let mut depth = 0usize;
    for (i, e) in events.iter().enumerate().skip(start) {
        match e {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    events.len() - 1
}

/// The words of some inline events, without their markup.
fn plain_text(events: &[Event]) -> String {
    let mut out = String::new();
    for e in events {
        match e {
            Event::Text(t) | Event::Code(t) => out.push_str(t),
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            _ => {}
        }
    }
    out
}

/// Text as Typst markup that says exactly that: every ASCII mark that could mean something in Typst gets a backslash.
/// Quotes are left for Typst to curl, and the marks that end nothing (`,` `!` `?` `:` `;`) are left as they are.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    for c in text.chars() {
        if c.is_ascii_punctuation() && !matches!(c, '\'' | '"' | ',' | '!' | '?' | ':' | ';' | '%' | '&' | '|' | '^') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A Typst string literal: `"…"` with its quotes, backslashes and line ends escaped.
pub fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// What is being written into, besides the main text: a picture's alt text, a code block's lines.
enum Capture {
    Image { name: Option<String> },
    Code { lang: Option<String> },
}

struct Writer<'a> {
    out: String,
    /// Text set aside while a picture's alt text or a code block is read; written out at its end.
    captures: Vec<(Capture, String)>,
    shift: bool,
    notes: &'a HashMap<String, String>,
    headings: usize,
    /// Per open table: whether its head is being written.
    in_head: Vec<bool>,
    /// Per open link: whether it is written as a link (`#link(…)[…]`) or as its words.
    links: Vec<bool>,
}

impl<'a> Writer<'a> {
    fn new(shift: bool, notes: &'a HashMap<String, String>) -> Self {
        Writer { out: String::new(), captures: Vec::new(), shift, notes, headings: 0, in_head: Vec::new(), links: Vec::new() }
    }

    fn finish(self) -> String {
        self.out
    }

    /// Where text goes now: the innermost capture, or the document.
    fn sink(&mut self) -> &mut String {
        match self.captures.last_mut() {
            Some((_, text)) => text,
            None => &mut self.out,
        }
    }

    fn push(&mut self, text: &str) {
        self.sink().push_str(text);
    }

    fn run(&mut self, events: &[Event], picture: &mut dyn FnMut(&str) -> Option<String>) {
        for event in events {
            self.event(event, picture);
        }
    }

    fn event(&mut self, event: &Event, picture: &mut dyn FnMut(&str) -> Option<String>) {
        // Inside a code block every line is its text, as it is.
        if let Some((Capture::Code { .. }, text)) = self.captures.last_mut() {
            match event {
                Event::Text(t) => {
                    text.push_str(t);
                    return;
                }
                Event::End(TagEnd::CodeBlock) => {}
                _ => return,
            }
        }
        match event {
            Event::Start(tag) => self.start(tag, picture),
            Event::End(tag) => self.end(*tag),
            Event::Text(t) => {
                let text = escape(t);
                self.push(&text);
            }
            Event::Code(t) => {
                let code = format!("#raw({})", string(t));
                self.push(&code);
            }
            Event::SoftBreak => self.push(" "),
            Event::HardBreak => self.push("#linebreak() "),
            Event::Rule => self.push("#ledger-rule()\n\n"),
            Event::TaskListMarker(done) => {
                let marker = format!("#ledger-task({done}) ");
                self.push(&marker);
            }
            Event::FootnoteReference(label) => {
                let note = self.notes.get(label.as_ref()).cloned().unwrap_or_default();
                let call = format!("#footnote[{note}]");
                self.push(&call);
            }
            // Raw HTML is left out (its words come as text of their own), but a line break is kept.
            Event::Html(html) | Event::InlineHtml(html) => {
                let tag = html.trim().to_ascii_lowercase();
                if tag.starts_with("<br") {
                    self.push("#linebreak() ");
                }
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => {
                let text = escape(t);
                self.push(&text);
            }
        }
    }

    fn start(&mut self, tag: &Tag, picture: &mut dyn FnMut(&str) -> Option<String>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.headings += 1;
                let mut n = *level as usize;
                if self.shift && n > 1 {
                    n -= 1;
                }
                let open = format!("#heading(level: {n})[");
                self.push(&open);
            }
            Tag::BlockQuote(_) => self.push("#quote(block: true)["),
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().filter(|l| !l.is_empty()).map(str::to_string),
                    CodeBlockKind::Indented => None,
                };
                self.captures.push((Capture::Code { lang }, String::new()));
            }
            Tag::List(None) => self.push("#list("),
            Tag::List(Some(start)) => {
                let open = format!("#enum(start: {start}, ");
                self.push(&open);
            }
            Tag::Item => self.push("["),
            Tag::Table(alignments) => {
                let align: Vec<&str> = alignments
                    .iter()
                    .map(|a| match a {
                        pulldown_cmark::Alignment::Center => "center",
                        pulldown_cmark::Alignment::Right => "right",
                        _ => "left",
                    })
                    .collect();
                // As wide as the page: the last column takes what the others leave.
                let columns = alignments.len().max(1);
                let open = format!("#table(columns: (auto,) * {} + (1fr,), align: ({},), ", columns - 1, align.join(", "));
                self.push(&open);
                self.in_head.push(false);
            }
            Tag::TableHead => {
                if let Some(h) = self.in_head.last_mut() {
                    *h = true;
                }
                self.push("table.header(");
            }
            Tag::TableRow => {}
            Tag::TableCell => self.push("["),
            Tag::Emphasis => self.push("#emph["),
            Tag::Strong => self.push("#strong["),
            Tag::Strikethrough => self.push("#strike["),
            Tag::Superscript => self.push("#super["),
            Tag::Subscript => self.push("#sub["),
            Tag::Link { link_type, dest_url, .. } => {
                let url = dest_url.trim();
                let web = !matches!(link_type, LinkType::WikiLink { .. })
                    && ["http://", "https://", "mailto:"].iter().any(|s| url.to_ascii_lowercase().starts_with(s));
                if web {
                    let open = format!("#link({})[", string(url));
                    self.push(&open);
                }
                self.links.push(web);
            }
            Tag::Image { dest_url, .. } => {
                let name = picture(dest_url);
                self.captures.push((Capture::Image { name }, String::new()));
            }
            Tag::FootnoteDefinition(_) | Tag::MetadataBlock(_) | Tag::HtmlBlock | Tag::DefinitionList => {}
            Tag::DefinitionListTitle => self.push("#strong["),
            Tag::DefinitionListDefinition => self.push("#pad(left: 1.2em)["),
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.push("\n\n"),
            TagEnd::Heading(_) => self.push("]\n\n"),
            TagEnd::BlockQuote(_) => self.push("]\n\n"),
            TagEnd::CodeBlock => {
                if let Some((Capture::Code { lang }, text)) = self.captures.pop() {
                    let text = text.strip_suffix('\n').unwrap_or(&text);
                    let lang = lang.map(|l| format!(", lang: {}", string(&l))).unwrap_or_default();
                    let block = format!("#raw(block: true{lang}, {})\n\n", string(text));
                    self.push(&block);
                }
            }
            TagEnd::List(_) => self.push(")\n\n"),
            TagEnd::Item => {
                // An item's own paragraphs end in a blank line, which would only add space at its end.
                let sink = self.sink();
                while sink.ends_with('\n') {
                    sink.pop();
                }
                sink.push_str("], ");
            }
            TagEnd::Table => {
                self.in_head.pop();
                self.push(")\n\n");
            }
            TagEnd::TableHead => {
                if let Some(h) = self.in_head.last_mut() {
                    *h = false;
                }
                self.push("), ");
            }
            TagEnd::TableRow => {}
            TagEnd::TableCell => self.push("], "),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Superscript | TagEnd::Subscript => self.push("]"),
            TagEnd::Link => {
                if self.links.pop().unwrap_or(false) {
                    self.push("]");
                }
            }
            TagEnd::Image => {
                if let Some((Capture::Image { name }, alt)) = self.captures.pop() {
                    let alt_arg = if alt.trim().is_empty() { "none".to_string() } else { format!("[{}]", alt.trim()) };
                    let call = match name {
                        Some(name) => format!("#ledger-image({}, {alt_arg})", string(&name)),
                        None => format!("#ledger-missing-image([{}])", alt.trim()),
                    };
                    self.push(&call);
                }
            }
            TagEnd::DefinitionListTitle | TagEnd::DefinitionListDefinition => self.push("]\n\n"),
            TagEnd::FootnoteDefinition | TagEnd::MetadataBlock(_) | TagEnd::HtmlBlock | TagEnd::DefinitionList => {}
        }
    }
}
