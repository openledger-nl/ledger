//! Markdown to Atlassian Document Format (ADF, what Jira stores a description as) and back.
//!
//! Only the markdown people write in a task or a story is understood: paragraphs (a single line break stays a line
//! break), headings, bullet and numbered lists (nested by indenting), fenced code, quotes, rules, and **bold**,
//! _italic_, ~~struck~~, `code` and [links](https://example.com) inside a line. Anything else is text.
//!
//! The two directions are each other's inverse on what they make: `to_markdown(to_adf(md))` is the canonical form
//! of `md`, and turning that into ADF again gives the same document. The sync compares canonical forms, so a
//! description that only looks different in the other format is never mistaken for an edit.
//!
//! A Jira description can hold more than this (pictures, tables, mentions, colours). [`to_markdown`] keeps their
//! text and says the conversion lost something, so the sync knows not to write such a description back.

use serde_json::{Value, json};

/// The marks a piece of text can carry, outermost first: the order they are written in markdown.
const MARKS: [&str; 5] = ["link", "strong", "em", "strike", "code"];

// ---- markdown to ADF ----------------------------------------------------------------------------------

/// The ADF document for `markdown`, or `None` for an empty one (Jira's way of saying "no description").
pub fn to_adf(markdown: &str) -> Option<Value> {
    let markdown = markdown.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = markdown.trim_matches('\n').lines().collect();
    let content = blocks(&lines);
    if content.is_empty() { None } else { Some(json!({ "type": "doc", "version": 1, "content": content })) }
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    let rest = &line[hashes..];
    ((1..=6).contains(&hashes) && (rest.is_empty() || rest.starts_with(' '))).then(|| (hashes, rest.trim()))
}

fn is_rule(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 3 && ["-", "*", "_"].iter().any(|c| line.chars().all(|x| x.to_string() == *c))
}

struct Item<'a> {
    indent: usize,
    /// The number of a numbered item.
    number: Option<u64>,
    text: &'a str,
}

fn list_item(line: &str) -> Option<Item<'_>> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    let (number, after) = if let Some(after) = rest.strip_prefix(['-', '*', '+']) {
        (None, after)
    } else {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let after = rest[digits..].strip_prefix(['.', ')']).filter(|_| (1..=9).contains(&digits))?;
        (Some(rest[..digits].parse().ok()?), after)
    };
    match after.strip_prefix(' ') {
        Some(text) => Some(Item { indent, number, text: text.trim() }),
        None if after.is_empty() => Some(Item { indent, number, text: "" }),
        None => None,
    }
}

/// Whether the line starts something other than a paragraph, and so ends the paragraph before it.
fn starts_block(line: &str) -> bool {
    is_fence(line) || heading(line).is_some() || is_rule(line) || line.trim_start().starts_with('>') || list_item(line).is_some()
}

fn blocks(lines: &[&str]) -> Vec<Value> {
    let mut out = vec![];
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
        } else if is_fence(line) {
            let language = line.trim_start()[3..].trim();
            let end = lines[i + 1..].iter().position(|l| is_fence(l)).map_or(lines.len(), |p| i + 1 + p);
            let code = lines[i + 1..end].join("\n");
            let mut node = json!({ "type": "codeBlock" });
            if !language.is_empty() {
                node["attrs"] = json!({ "language": language });
            }
            if !code.is_empty() {
                node["content"] = json!([text_node(&code, &[])]);
            }
            out.push(node);
            i = end + 1;
        } else if let Some((level, text)) = heading(line) {
            out.push(with_content(json!({ "type": "heading", "attrs": { "level": level } }), inline(text)));
            i += 1;
        } else if is_rule(line) {
            out.push(json!({ "type": "rule" }));
            i += 1;
        } else if line.trim_start().starts_with('>') {
            let end = lines[i..].iter().position(|l| !l.trim_start().starts_with('>')).map_or(lines.len(), |p| i + p);
            let inner: Vec<&str> = lines[i..end]
                .iter()
                .map(|l| {
                    let l = &l.trim_start()[1..];
                    l.strip_prefix(' ').unwrap_or(l)
                })
                .collect();
            // A quote holds paragraphs, lists and code; a heading in one becomes a paragraph.
            let content: Vec<Value> = blocks(&inner)
                .into_iter()
                .map(|mut b| {
                    if b["type"] == "heading" {
                        b["type"] = json!("paragraph");
                        b.as_object_mut().map(|o| o.remove("attrs"));
                    }
                    b
                })
                .filter(|b| b["type"] != "rule")
                .collect();
            if !content.is_empty() {
                out.push(json!({ "type": "blockquote", "content": content }));
            }
            i = end;
        } else if list_item(line).is_some() {
            let mut items = vec![];
            while let Some(item) = lines.get(i).and_then(|l| list_item(l)) {
                items.push(item);
                i += 1;
            }
            let mut at = 0;
            while at < items.len() {
                let indent = items[at].indent;
                out.push(list(&items, &mut at, indent));
            }
        } else {
            let end = lines[i + 1..].iter().position(|l| l.trim().is_empty() || starts_block(l)).map_or(lines.len(), |p| i + 1 + p);
            let text = lines[i..end].iter().map(|l| l.trim()).collect::<Vec<_>>().join("\n");
            out.push(with_content(json!({ "type": "paragraph" }), inline(&text)));
            i = end;
        }
    }
    out
}

/// One list, from `items[*at]` on: the items at `indent`, with the deeper ones under the item before them.
fn list(items: &[Item], at: &mut usize, indent: usize) -> Value {
    let number = items[*at].number;
    let mut nodes: Vec<Value> = vec![];
    while let Some(item) = items.get(*at) {
        if item.indent < indent {
            break;
        }
        if item.indent > indent {
            let nested = list(items, at, item.indent);
            match nodes.last_mut().and_then(|n| n["content"].as_array_mut()) {
                Some(content) => content.push(nested),
                None => nodes.push(json!({ "type": "listItem", "content": [{ "type": "paragraph" }, nested] })),
            }
            continue;
        }
        if item.number.is_some() != number.is_some() {
            break;
        }
        nodes.push(json!({ "type": "listItem", "content": [with_content(json!({ "type": "paragraph" }), inline(item.text))] }));
        *at += 1;
    }
    match number {
        Some(order) if order != 1 => json!({ "type": "orderedList", "attrs": { "order": order }, "content": nodes }),
        Some(_) => json!({ "type": "orderedList", "content": nodes }),
        None => json!({ "type": "bulletList", "content": nodes }),
    }
}

fn with_content(mut node: Value, content: Vec<Value>) -> Value {
    if !content.is_empty() {
        node["content"] = Value::Array(content);
    }
    node
}

fn text_node(text: &str, marks: &[Value]) -> Value {
    let mut node = json!({ "type": "text", "text": text });
    if !marks.is_empty() {
        node["marks"] = Value::Array(marks.to_vec());
    }
    node
}

/// The inline nodes for a line (or lines: a line break is a hard break).
fn inline(text: &str) -> Vec<Value> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = vec![];
    spans(&chars, &[], &mut out);
    merge_text(out)
}

fn mark(kind: &str) -> Value {
    json!({ "type": kind })
}

/// Where the closing `delimiter` is, from `from` on: the first one that follows something other than a space.
/// An underscore must also not be followed by a letter or digit, so `snake_case_words` stay as they are.
fn closing(chars: &[char], from: usize, delimiter: &[char]) -> Option<usize> {
    let n = delimiter.len();
    (from..chars.len().saturating_sub(n - 1)).find(|&j| {
        chars[j..j + n] == *delimiter
            && j > from
            && !chars[j - 1].is_whitespace()
            && chars[j - 1] != '\\'
            && !(delimiter[0] == '_' && chars.get(j + n).is_some_and(|c| c.is_alphanumeric()))
            // `*` closes on its own, not as half of a `**`.
            && !(n == 1 && (chars.get(j + 1) == Some(&delimiter[0]) || chars[j - 1] == delimiter[0]))
    })
}

fn spans(chars: &[char], marks: &[Value], out: &mut Vec<Value>) {
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Value>| {
        if !plain.is_empty() {
            out.push(text_node(plain, marks));
            plain.clear();
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let opens = |n: usize| chars.get(i + n).is_some_and(|c| !c.is_whitespace());

        if c == '\\' && next.is_some_and(|n| n.is_ascii_punctuation()) {
            plain.push(next.unwrap_or_default());
            i += 2;
            continue;
        }
        if c == '\n' {
            flush(&mut plain, out);
            out.push(json!({ "type": "hardBreak" }));
            i += 1;
            continue;
        }
        if c == '`'
            && let Some(len) = chars[i + 1..].iter().position(|x| *x == '`').filter(|len| *len > 0)
        {
            flush(&mut plain, out);
            // Code takes no other marks but a link.
            let code: Vec<Value> = marks.iter().filter(|m| m["type"] == "link").cloned().chain([mark("code")]).collect();
            out.push(text_node(&chars[i + 1..i + 1 + len].iter().collect::<String>(), &code));
            i += len + 2;
            continue;
        }
        let pair = next.filter(|n| *n == c).map(|_| [c, c]);
        let wrapped = match (c, pair) {
            ('*' | '_', Some(delimiter)) if opens(2) && !(c == '_' && i > 0 && chars[i - 1].is_alphanumeric()) => {
                closing(chars, i + 2, &delimiter).map(|end| (2, end, "strong"))
            }
            ('~', Some(delimiter)) if opens(2) => closing(chars, i + 2, &delimiter).map(|end| (2, end, "strike")),
            ('*' | '_', None) if opens(1) && !(c == '_' && i > 0 && chars[i - 1].is_alphanumeric()) => {
                closing(chars, i + 1, &[c]).map(|end| (1, end, "em"))
            }
            _ => None,
        };
        if let Some((n, end, kind)) = wrapped
            && !marks.iter().any(|m| m["type"] == kind)
        {
            flush(&mut plain, out);
            let inner: Vec<Value> = marks.iter().cloned().chain([mark(kind)]).collect();
            spans(&chars[i + n..end], &inner, out);
            i = end + n;
            continue;
        }
        if c == '['
            && !marks.iter().any(|m| m["type"] == "link")
            && let Some((label_end, href)) = link_at(chars, i)
        {
            flush(&mut plain, out);
            let inner: Vec<Value> = marks.iter().cloned().chain([json!({ "type": "link", "attrs": { "href": href } })]).collect();
            spans(&chars[i + 1..label_end], &inner, out);
            i = label_end + 3 + href.chars().count();
            continue;
        }
        plain.push(c);
        i += 1;
    }
    flush(&mut plain, out);
}

/// `[label](href)` starting at `start`: where the label ends, and the address.
fn link_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    let mut depth = 0;
    let label_end = (start + 1..chars.len()).find(|&j| match chars[j] {
        '[' => {
            depth += 1;
            false
        }
        ']' if depth == 0 => true,
        ']' => {
            depth -= 1;
            false
        }
        _ => false,
    })?;
    if label_end == start + 1 || chars.get(label_end + 1) != Some(&'(') {
        return None;
    }
    let href_end = (label_end + 2..chars.len()).find(|&j| chars[j] == ')')?;
    let href: String = chars[label_end + 2..href_end].iter().collect();
    (!href.is_empty() && !href.contains(char::is_whitespace)).then_some((label_end, href))
}

/// Joins neighbouring text with the same marks.
fn merge_text(nodes: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for node in nodes {
        if let Some(last) = out.last_mut()
            && last["type"] == "text"
            && node["type"] == "text"
            && last.get("marks") == node.get("marks")
        {
            let joined = format!("{}{}", last["text"].as_str().unwrap_or_default(), node["text"].as_str().unwrap_or_default());
            last["text"] = json!(joined);
            continue;
        }
        out.push(node);
    }
    out
}

// ---- ADF to markdown ----------------------------------------------------------------------------------

/// The markdown for a description, and whether nothing was lost on the way: `false` when the description holds
/// something markdown here can't say (a picture, a table, a mention, a colour), which is then kept as text or left
/// out. No description is the empty text.
pub fn to_markdown(adf: Option<&Value>) -> (String, bool) {
    let Some(doc) = adf.filter(|d| !d.is_null()) else { return (String::new(), true) };
    let mut writer = Writer { lossless: true };
    let text = writer.blocks(children(doc));
    (text.trim_matches('\n').to_string(), writer.lossless)
}

fn children(node: &Value) -> &[Value] {
    node["content"].as_array().map_or(&[], Vec::as_slice)
}

struct Writer {
    lossless: bool,
}

impl Writer {
    fn blocks(&mut self, nodes: &[Value]) -> String {
        let mut out = String::new();
        for node in nodes {
            let block = self.block(node);
            if block.is_empty() && node["type"] != "paragraph" {
                continue;
            }
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&block);
        }
        out
    }

    fn block(&mut self, node: &Value) -> String {
        match node["type"].as_str().unwrap_or_default() {
            "paragraph" => escape_lines(&self.inline(children(node))),
            "heading" => {
                let level = node["attrs"]["level"].as_u64().unwrap_or(1).clamp(1, 6) as usize;
                format!("{} {}", "#".repeat(level), self.inline(children(node)))
            }
            "bulletList" | "orderedList" => self.list(node, 0),
            "codeBlock" => {
                let language = node["attrs"]["language"].as_str().unwrap_or_default();
                let code: String = children(node).iter().filter_map(|t| t["text"].as_str()).collect();
                format!("```{language}\n{code}\n```")
            }
            "blockquote" => self.blocks(children(node)).lines().map(|l| if l.is_empty() { ">".to_string() } else { format!("> {l}") }).collect::<Vec<_>>().join("\n"),
            "rule" => "---".to_string(),
            _ => {
                // A table, a panel, a picture: its words are kept, the rest is lost.
                self.lossless = false;
                let words = self.blocks(children(node));
                if words.is_empty() { self.inline(children(node)) } else { words }
            }
        }
    }

    fn list(&mut self, node: &Value, depth: usize) -> String {
        let ordered = node["type"] == "orderedList";
        let first = node["attrs"]["order"].as_u64().unwrap_or(1);
        let indent = "  ".repeat(depth);
        let mut lines = vec![];
        for (number, item) in (first..).zip(children(node)) {
            let marker = if ordered { format!("{number}.") } else { "-".to_string() };
            let mut wrote_text = false;
            for part in children(item) {
                match part["type"].as_str() {
                    Some("paragraph") if !wrote_text => {
                        let text = self.inline(children(part));
                        // Inside an item a line break can't be written; it becomes a space.
                        if text.contains('\n') {
                            self.lossless = false;
                        }
                        lines.push(format!("{indent}{marker} {}", text.replace('\n', " ")).trim_end().to_string());
                        wrote_text = true;
                    }
                    Some("bulletList" | "orderedList") => {
                        if !wrote_text {
                            lines.push(format!("{indent}{marker}"));
                            wrote_text = true;
                        }
                        lines.push(self.list(part, depth + 1));
                    }
                    _ => {
                        self.lossless = false;
                        let text = self.block(part).replace('\n', " ");
                        if wrote_text {
                            if let Some(last) = lines.last_mut() {
                                last.push(' ');
                                last.push_str(&text);
                            }
                        } else {
                            lines.push(format!("{indent}{marker} {text}"));
                            wrote_text = true;
                        }
                    }
                }
            }
            if !wrote_text {
                lines.push(format!("{indent}{marker}"));
            }
        }
        lines.join("\n")
    }

    /// Inline nodes as markdown: marks are opened and closed around the runs of text that carry them.
    fn inline(&mut self, nodes: &[Value]) -> String {
        let mut out = String::new();
        let mut open: Vec<Value> = vec![];
        // Spaces at the edge of marked text go outside the marks that end or start there, where markdown expects
        // them, so they wait here until the next text says which marks go on.
        let mut spaces = String::new();
        for node in nodes {
            match node["type"].as_str().unwrap_or_default() {
                "text" => {
                    let text = node["text"].as_str().unwrap_or_default();
                    let mut marks: Vec<Value> = vec![];
                    for m in node["marks"].as_array().into_iter().flatten() {
                        if MARKS.contains(&m["type"].as_str().unwrap_or_default()) {
                            marks.push(m.clone());
                        } else {
                            self.lossless = false;
                        }
                    }
                    marks.sort_by_key(|m| MARKS.iter().position(|k| m["type"] == *k));
                    // Inside code, spaces are part of it.
                    let is_code = marks.iter().any(|m| m["type"] == "code");
                    let core = if is_code { text } else { text.trim() };
                    if core.is_empty() {
                        spaces.push_str(text);
                        continue;
                    }
                    if !is_code {
                        spaces.push_str(&text[..text.len() - text.trim_start().len()]);
                    }
                    // Close what this text doesn't carry (and what was opened inside that), then open what it adds.
                    let keep = open.iter().zip(&marks).take_while(|(a, b)| a == b).count();
                    close_to(&mut out, &mut open, keep);
                    out.push_str(&std::mem::take(&mut spaces));
                    for m in &marks[keep..] {
                        out.push_str(opening(m));
                        open.push(m.clone());
                    }
                    out.push_str(&if is_code { core.to_string() } else { escape(core) });
                    if !is_code {
                        spaces.push_str(&text[text.trim_end().len()..]);
                    }
                }
                "hardBreak" => {
                    close_to(&mut out, &mut open, 0);
                    out.push_str(&std::mem::take(&mut spaces));
                    out.push('\n');
                }
                _ => {
                    // A mention, an emoji, a date, a smart link: what it shows is kept as text.
                    self.lossless = false;
                    close_to(&mut out, &mut open, 0);
                    out.push_str(&std::mem::take(&mut spaces));
                    let attrs = &node["attrs"];
                    out.push_str(["text", "shortName", "url"].iter().find_map(|k| attrs[*k].as_str()).unwrap_or_default());
                }
            }
        }
        close_to(&mut out, &mut open, 0);
        out.push_str(&spaces);
        out
    }
}

fn opening(mark: &Value) -> &'static str {
    match mark["type"].as_str() {
        Some("link") => "[",
        Some("strong") => "**",
        Some("em") => "_",
        Some("strike") => "~~",
        _ => "`",
    }
}

/// Closes the open marks, innermost first, until `keep` are left.
fn close_to(out: &mut String, open: &mut Vec<Value>, keep: usize) {
    while open.len() > keep
        && let Some(m) = open.pop()
    {
        match m["type"].as_str() {
            Some("link") => {
                out.push_str("](");
                out.push_str(m["attrs"]["href"].as_str().unwrap_or_default());
                out.push(')');
            }
            _ => out.push_str(opening(&m)),
        }
    }
}

/// Plain text that would read as markup gets its markup characters escaped; text that wouldn't is left alone.
fn escape(text: &str) -> String {
    if !text.contains('`') && inline(text) == vec![text_node(text, &[])] {
        return text.to_string();
    }
    text.chars().fold(String::new(), |mut out, c| {
        if matches!(c, '\\' | '*' | '_' | '`' | '~' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
        out
    })
}

/// A line of a paragraph that would start a heading, a list or a quote gets a backslash: in front of it, or for a
/// numbered item (`2026. was a year`) in front of the dot.
fn escape_lines(text: &str) -> String {
    let escape_line = |line: &str| -> String {
        if !starts_block(line) {
            return line.to_string();
        }
        match list_item(line).filter(|item| item.number.is_some()) {
            Some(_) => {
                let digits = line.chars().take_while(char::is_ascii_digit).count();
                format!("{}\\{}", &line[..digits], &line[digits..])
            }
            None => format!("\\{line}"),
        }
    };
    text.split('\n').map(escape_line).collect::<Vec<_>>().join("\n")
}

/// The form a description is compared in: what it becomes after a trip through ADF.
pub fn canonical(markdown: &str) -> String {
    to_markdown(to_adf(markdown).as_ref()).0
}

/// A description from Jira in the form it is compared in, and whether its conversion lost nothing.
pub fn from_jira(adf: Option<&Value>) -> (String, bool) {
    let (markdown, lossless) = to_markdown(adf);
    (canonical(&markdown), lossless)
}
