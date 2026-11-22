//! What changed between the versions of a markdown deliverable, and who wrote what is in it.
//!
//! Nothing here is a source of truth. Every published version is a whole, frozen copy in `versions/`, and who made it is
//! in its record (see [`provenance`]). So a comparison is worked out from two copies when it is asked for ([`compare`]),
//! and who wrote a passage is the version it first appeared in ([`authors`]). That is enough to tell AI from you because
//! a Claude Code run always gets a version of its own: edits of yours that no version holds are published before the run
//! changes the file (see `claude_code::versions`).
//!
//! Nothing is written: the history is in the versions and their manifests, and worked out from them here.
//!
//! How text is compared: a document is cut into blocks (a paragraph, a heading, a list item, a table, a code block) and
//! each block into tokens (words, spaces, punctuation, and markdown that has to stay whole, such as a link or `code`).
//! Blocks are matched first, so a rewritten paragraph is shown as one changed paragraph rather than scattered words, and
//! a paragraph that only moved is shown as moved. Within a changed block the tokens are compared.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use rusqlite::Connection;
use serde::Serialize;
use similar::{Algorithm, ChangeTag, DiffOp, TextDiff, capture_diff_slices_deadline};

use crate::db::query_opt;
use crate::deliverables::{self, split_version};
use crate::error::{Error, Result};
use crate::{markdown, provenance};

/// How long one comparison may take before it settles for a rougher answer.
const DEADLINE: Duration = Duration::from_secs(2);

/// A block this short that turns up elsewhere is not called moved: "Yes." or a lone "---" is just text.
const MOVE_MIN_CHARS: usize = 16;

/// A block that went out in one place and came in, edited, in another moved, from this much of its words in common.
const MOVED_EDITED: f32 = 0.6;

/// Looking for moved and edited blocks compares every block that went with every one that came: up to this many pairs.
const MOVE_PAIRS: usize = 2500;

/// Two blocks are the same block, changed, from this much of their words in common.
const SAME_BLOCK: f32 = 0.45;

// ---- tokens -----------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Space,
    Newline,
    /// What a line starts with to be a heading, list item or quote, and a fence or table rule: never marked.
    Struct,
    /// A run of `*`, `_`, `~`, `=` or `|`: emphasis and table cells, never marked, so marks can't cross them.
    Delim,
    /// Markdown that has to stay whole: a link, `code`, a `%%` note, an escaped character.
    Atom,
    Punct,
}

impl Kind {
    fn markable(self) -> bool {
        matches!(self, Kind::Word | Kind::Space | Kind::Atom | Kind::Punct)
    }

    /// Counted as a word.
    fn counts(self) -> bool {
        matches!(self, Kind::Word | Kind::Atom)
    }
}

#[derive(Debug, Clone, Copy)]
struct Tok {
    kind: Kind,
    start: usize,
    end: usize,
}

fn push(out: &mut Vec<Tok>, kind: Kind, start: usize, end: usize) {
    if end > start {
        out.push(Tok { kind, start, end });
    }
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let (start, end) = (pos, pos + body.len());
        if let Some((mark, len)) = markdown::fence_of(body) {
            match fence {
                Some((m, l)) if m == mark && len >= l && body.trim().bytes().all(|b| b == mark) => fence = None,
                Some(_) => {}
                None => fence = Some((mark, len)),
            }
            push(&mut out, Kind::Struct, start, end);
        } else if fence.is_some() {
            inline(body, start, true, &mut out);
        } else if is_rule(body) {
            push(&mut out, Kind::Struct, start, end);
        } else {
            let (p, _) = prefix(body);
            push(&mut out, Kind::Struct, start, start + p);
            inline(&body[p..], start + p, false, &mut out);
        }
        if line.len() > body.len() {
            push(&mut out, Kind::Newline, end, end + 1);
        }
        pos += line.len();
    }
    out
}

/// A line that is a rule: a thematic break, a setext heading's underline, or a table's `|---|---|`.
fn is_rule(line: &str) -> bool {
    let t: Vec<u8> = line.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let Some(&first) = t.first() else {
        return false;
    };
    let same = t.iter().all(|&b| b == first);
    (same && first == b'=' && t.len() >= 2)
        || (same && matches!(first, b'-' | b'*' | b'_') && t.len() >= 3)
        || (t.contains(&b'-') && t.contains(&b'|') && t.iter().all(|&b| matches!(b, b'|' | b':' | b'-')))
}

/// How long the start of the line is that makes it a heading, list item or quote, and which of those it is.
fn prefix(line: &str) -> (usize, Option<BlockKind>) {
    let b = line.as_bytes();
    let skip = |i: &mut usize| {
        while *i < b.len() && (b[*i] == b' ' || b[*i] == b'\t') {
            *i += 1;
        }
    };
    let mut i = 0;
    skip(&mut i);
    let mut quote = false;
    while i < b.len() && b[i] == b'>' {
        quote = true;
        i += 1;
        skip(&mut i);
    }
    let fallback = (i, quote.then_some(BlockKind::Quote));
    if i < b.len() && b[i] == b'#' {
        let n = b[i..].iter().take_while(|&&c| c == b'#').count();
        if n <= 6 && (i + n == b.len() || b[i + n] == b' ' || b[i + n] == b'\t') {
            i += n;
            skip(&mut i);
            return (i, Some(BlockKind::Heading));
        }
        return fallback;
    }
    let mut j = i;
    if j < b.len() && matches!(b[j], b'-' | b'*' | b'+') {
        j += 1;
    } else {
        let digits = b[j..].iter().take_while(|c| c.is_ascii_digit()).count();
        if (1..=9).contains(&digits) && j + digits < b.len() && matches!(b[j + digits], b'.' | b')') {
            j += digits + 1;
        }
    }
    if j > i && (j == b.len() || b[j] == b' ' || b[j] == b'\t') {
        i = j;
        skip(&mut i);
        if b.len() >= i + 3 && b[i] == b'[' && matches!(b[i + 1], b' ' | b'x' | b'X') && b[i + 2] == b']' && (i + 3 == b.len() || b[i + 3] == b' ') {
            i += 3;
            skip(&mut i);
        }
        return (i, Some(BlockKind::Item));
    }
    fallback
}

fn inline(s: &str, base: usize, code: bool, out: &mut Vec<Tok>) {
    let mut i = 0;
    while i < s.len() {
        let c = s[i..].chars().next().unwrap_or(' ');
        let (kind, len) = if c.is_whitespace() {
            (Kind::Space, run(s, i, char::is_whitespace))
        } else if c.is_alphanumeric() {
            (Kind::Word, word_len(s, i))
        } else if code {
            (Kind::Punct, c.len_utf8())
        } else if let Some(len) = atom_len(&s[i..]) {
            (Kind::Atom, len)
        } else if matches!(c, '*' | '_' | '~' | '=' | '|') {
            (Kind::Delim, run(s, i, |x| x == c))
        } else {
            (Kind::Punct, c.len_utf8())
        };
        push(out, kind, base + i, base + i + len);
        i += len.max(1);
    }
}

fn run(s: &str, i: usize, keep: impl Fn(char) -> bool) -> usize {
    s[i..].char_indices().find(|(_, c)| !keep(*c)).map_or(s.len() - i, |(k, _)| k)
}

/// A word, with the `_`, `'` or `'` inside it ("snake_case", "don't").
fn word_len(s: &str, i: usize) -> usize {
    let mut chars = s[i..].char_indices().peekable();
    let mut end = 0;
    while let Some((k, c)) = chars.next() {
        if c.is_alphanumeric() {
            end = k + c.len_utf8();
        } else {
            // ui-text: allow (a curly apostrophe inside a word, in someone's text)
            let joins = matches!(c, '_' | '\'' | '\u{2019}');
            if !(joins && chars.peek().is_some_and(|(_, n)| n.is_alphanumeric())) {
                break;
            }
        }
    }
    end
}

/// The length of the markdown at the start of `rest` that has to stay whole, if it starts with any.
fn atom_len(rest: &str) -> Option<usize> {
    let b = rest.as_bytes();
    match b[0] {
        b'\\' => rest[1..].chars().next().map(|c| 1 + c.len_utf8()),
        b'%' if rest.starts_with("%%") => Some(rest[2..].find("%%").map_or(rest.len(), |k| k + 4)),
        b'`' => {
            let n = b.iter().take_while(|&&c| c == b'`').count();
            let mut j = n;
            while j < b.len() {
                if b[j] == b'`' {
                    let m = b[j..].iter().take_while(|&&c| c == b'`').count();
                    if m == n {
                        return Some(j + m);
                    }
                    j += m;
                } else {
                    j += 1;
                }
            }
            None
        }
        b'!' | b'[' => {
            let bang = usize::from(b[0] == b'!');
            let r = &rest[bang..];
            if r.starts_with("[[") {
                return r.find("]]").map(|k| bang + k + 2);
            }
            if !r.starts_with('[') {
                return None;
            }
            let close = bracket_close(r)?;
            let after = &r[close + 1..];
            if after.starts_with('(') {
                after.find(')').map(|k| bang + close + 1 + k + 1)
            } else if after.starts_with('[') {
                after.find(']').map(|k| bang + close + 1 + k + 1)
            } else {
                None
            }
        }
        b'<' => {
            let k = rest.find('>')?;
            let inner = &rest[1..k];
            let first = inner.chars().next()?;
            ((first.is_ascii_alphanumeric() || first == '/') && !inner.contains('<')).then_some(k + 1)
        }
        _ => None,
    }
}

fn bracket_close(r: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;
    for (k, c) in r.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

// ---- blocks -----------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Para,
    Heading,
    Item,
    Quote,
    Code,
    Table,
    Rule,
    /// The frontmatter: not part of what reads.
    Meta,
    /// A `%%` note over several lines: not part of what reads either.
    Note,
}

impl BlockKind {
    fn shown(self) -> bool {
        !matches!(self, BlockKind::Meta | BlockKind::Note)
    }

    /// Text compared word by word and marked inline in a note; code and tables are shown as lines.
    fn prose(self) -> bool {
        !matches!(self, BlockKind::Code | BlockKind::Table)
    }
}

#[derive(Debug, Clone, Copy)]
struct Block {
    kind: BlockKind,
    start: usize,
    end: usize,
    /// Its tokens, `t0..t1` in the document's.
    t0: usize,
    t1: usize,
}

fn cut_blocks(text: &str) -> Vec<Block> {
    let mut lines: Vec<(usize, usize)> = Vec::new();
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        lines.push((pos, pos + body.len()));
        pos += line.len();
    }
    let new = |kind, (s, e): (usize, usize)| Block { kind, start: s, end: e, t0: 0, t1: 0 };
    let mut out: Vec<Block> = Vec::new();
    let mut cur: Option<Block> = None;
    let close = |cur: &mut Option<Block>, out: &mut Vec<Block>| {
        if let Some(b) = cur.take() {
            out.push(b);
        }
    };

    let mut i = 0;
    if lines.first().is_some_and(|&(s, e)| text[s..e].trim_end() == "---")
        && let Some(k) = (1..lines.len()).find(|&k| matches!(text[lines[k].0..lines[k].1].trim_end(), "---" | "..."))
    {
        out.push(new(BlockKind::Meta, (0, lines[k].1)));
        i = k + 1;
    }

    let mut fence: Option<(u8, usize)> = None;
    let mut comment = false;
    while i < lines.len() {
        let (s, e) = lines[i];
        let line = &text[s..e];
        i += 1;
        if let Some((m, l)) = fence {
            if let Some(b) = cur.as_mut() {
                b.end = e;
            }
            if markdown::fence_of(line).is_some_and(|(mm, ll)| mm == m && ll >= l && line.trim().bytes().all(|b| b == m)) {
                fence = None;
                close(&mut cur, &mut out);
            }
            continue;
        }
        if comment {
            if let Some(b) = cur.as_mut() {
                b.end = e;
            }
            if line.matches("%%").count() % 2 == 1 {
                comment = false;
                close(&mut cur, &mut out);
            }
            continue;
        }
        if line.trim().is_empty() {
            close(&mut cur, &mut out);
            continue;
        }
        if let Some(opened) = markdown::fence_of(line) {
            close(&mut cur, &mut out);
            cur = Some(new(BlockKind::Code, (s, e)));
            fence = Some(opened);
            continue;
        }
        if line.trim_start().starts_with("%%") && line.matches("%%").count() % 2 == 1 {
            close(&mut cur, &mut out);
            cur = Some(new(BlockKind::Note, (s, e)));
            comment = true;
            continue;
        }
        if is_rule(line) {
            let t = line.trim();
            let kind = cur.as_ref().map(|b| b.kind);
            let setext = kind == Some(BlockKind::Para) && (t.bytes().all(|b| b == b'=') || t.bytes().all(|b| b == b'-'));
            let table = t.contains('|') && matches!(kind, Some(BlockKind::Para | BlockKind::Table));
            if let Some(b) = cur.as_mut().filter(|_| setext || table) {
                b.end = e;
                if table {
                    b.kind = BlockKind::Table;
                } else {
                    close(&mut cur, &mut out);
                }
                continue;
            }
            close(&mut cur, &mut out);
            out.push(new(BlockKind::Rule, (s, e)));
            continue;
        }
        let kind = cur.as_ref().map(|b| b.kind);
        match prefix(line).1 {
            Some(BlockKind::Heading) => {
                close(&mut cur, &mut out);
                out.push(new(BlockKind::Heading, (s, e)));
            }
            Some(BlockKind::Item) => {
                close(&mut cur, &mut out);
                cur = Some(new(BlockKind::Item, (s, e)));
            }
            Some(BlockKind::Quote) if kind == Some(BlockKind::Quote) => cur.as_mut().map_or((), |b| b.end = e),
            Some(_) => {
                close(&mut cur, &mut out);
                cur = Some(new(BlockKind::Quote, (s, e)));
            }
            None => match kind {
                Some(BlockKind::Table) if line.contains('|') => cur.as_mut().map_or((), |b| b.end = e),
                Some(BlockKind::Para | BlockKind::Item | BlockKind::Quote) => cur.as_mut().map_or((), |b| b.end = e),
                _ => {
                    close(&mut cur, &mut out);
                    cur = Some(new(BlockKind::Para, (s, e)));
                }
            },
        }
    }
    close(&mut cur, &mut out);
    out
}

/// A text, cut into its tokens and blocks.
struct Doc<'a> {
    text: &'a str,
    toks: Vec<Tok>,
    blocks: Vec<Block>,
}

impl<'a> Doc<'a> {
    fn new(text: &'a str) -> Doc<'a> {
        let toks = tokenize(text);
        let mut blocks = cut_blocks(text);
        for b in &mut blocks {
            b.t0 = toks.partition_point(|t| t.start < b.start);
            b.t1 = toks.partition_point(|t| t.start < b.end);
        }
        Doc { text, toks, blocks }
    }

    fn tok(&self, i: usize) -> &'a str {
        &self.text[self.toks[i].start..self.toks[i].end]
    }

    fn keys(&self, from: usize, to: usize) -> Vec<&'a str> {
        (from..to).map(|i| self.tok(i)).collect()
    }

    fn block_text(&self, b: &Block) -> &'a str {
        &self.text[b.start..b.end]
    }

    fn words(&self, b: &Block) -> usize {
        (b.t0..b.t1).filter(|&i| self.toks[i].kind.counts()).count()
    }
}

/// Whether most of a block's words are flagged (taken out, or put in).
fn mostly(d: &Doc, b: &Block, flags: &[bool]) -> bool {
    let words: Vec<usize> = (b.t0..b.t1).filter(|&i| d.toks[i].kind.counts()).collect();
    !words.is_empty() && words.iter().filter(|&&i| flags[i]).count() * 2 > words.len()
}

fn diff<T: Hash + Eq + Ord>(alg: Algorithm, a: &[T], b: &[T]) -> Vec<DiffOp> {
    capture_diff_slices_deadline(alg, a, b, Some(Instant::now() + DEADLINE))
}

// ---- who wrote what ---------------------------------------------------------------------------------

/// For every text in a row of versions: which of them each of its tokens comes from, and per version how many words of
/// each earlier one it took out.
struct Walk {
    origins: Vec<Vec<u16>>,
    removed: Vec<HashMap<u16, usize>>,
}

fn walk(docs: &[Doc]) -> Walk {
    let mut origins: Vec<Vec<u16>> = Vec::with_capacity(docs.len());
    let mut removed: Vec<HashMap<u16, usize>> = Vec::with_capacity(docs.len());
    for (j, doc) in docs.iter().enumerate() {
        let here = u16::try_from(j).unwrap_or(u16::MAX);
        let (next, gone) = match (j.checked_sub(1), origins.last()) {
            (Some(p), Some(prev)) => step(&docs[p], prev, doc, here),
            _ => (vec![here; doc.toks.len()], HashMap::new()),
        };
        origins.push(next);
        removed.push(gone);
    }
    Walk { origins, removed }
}

/// One step of [`walk`]: who wrote each token of `doc` (by the version at index `here`), given who wrote each of
/// `prev_doc`'s; and how many words of each earlier version this step took out.
fn step(prev_doc: &Doc, prev: &[u16], doc: &Doc, here: u16) -> (Vec<u16>, HashMap<u16, usize>) {
    let mut next = vec![here; doc.toks.len()];
    let mut gone: HashMap<u16, usize> = HashMap::new();
    let ops = diff(Algorithm::Myers, &prev_doc.keys(0, prev_doc.toks.len()), &doc.keys(0, doc.toks.len()));
    let mut deleted = vec![false; prev_doc.toks.len()];
    let mut inserted = vec![false; doc.toks.len()];
    for op in &ops {
        match *op {
            DiffOp::Equal { old_index, new_index, len } => next[new_index..new_index + len].copy_from_slice(&prev[old_index..old_index + len]),
            DiffOp::Delete { old_index, old_len, .. } => deleted[old_index..old_index + old_len].fill(true),
            DiffOp::Insert { new_index, new_len, .. } => inserted[new_index..new_index + new_len].fill(true),
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                deleted[old_index..old_index + old_len].fill(true);
                inserted[new_index..new_index + new_len].fill(true);
            }
        }
    }
    // A block that went out in one place and came in, word for word, in another was moved: its words keep who
    // wrote them. (The comparison may have kept a few of its words where they were, a "the" or a full stop.)
    let mut away: HashMap<&str, Vec<usize>> = HashMap::new();
    for (k, b) in prev_doc.blocks.iter().enumerate() {
        let key = prev_doc.block_text(b).trim_end();
        if key.len() >= MOVE_MIN_CHARS && (b.t0..b.t1).any(|i| deleted[i] && prev_doc.toks[i].kind.counts()) {
            away.entry(key).or_default().push(k);
        }
    }
    let mut claimed = vec![false; prev_doc.blocks.len()];
    let mut open: Vec<usize> = Vec::new();
    for (n, b) in doc.blocks.iter().enumerate() {
        let key = doc.block_text(b).trim_end();
        if !(b.t0..b.t1).any(|i| inserted[i] && doc.toks[i].kind.counts()) {
            continue;
        }
        if let Some(k) = away.get_mut(key).and_then(Vec::pop) {
            let from = prev_doc.blocks[k];
            if from.t1 - from.t0 == b.t1 - b.t0 {
                next[b.t0..b.t1].copy_from_slice(&prev[from.t0..from.t1]);
                deleted[from.t0..from.t1].fill(false);
                claimed[k] = true;
                continue;
            }
        }
        if mostly(doc, b, &inserted) {
            open.push(n);
        }
    }
    // One that came in edited: the words it kept keep who wrote them.
    let gone_blocks: Vec<usize> = (0..prev_doc.blocks.len()).filter(|&k| !claimed[k] && mostly(prev_doc, &prev_doc.blocks[k], &deleted)).collect();
    if open.len() * gone_blocks.len() <= MOVE_PAIRS {
        let mut taken = vec![false; gone_blocks.len()];
        for n in open {
            let b = doc.blocks[n];
            let best = gone_blocks
                .iter()
                .enumerate()
                .filter(|&(g, &k)| !taken[g] && compatible(prev_doc.blocks[k].kind, b.kind))
                .map(|(g, &k)| (g, likeness(prev_doc, &prev_doc.blocks[k], doc, &b)))
                .filter(|&(_, l)| l >= MOVED_EDITED)
                .max_by(|x, y| x.1.total_cmp(&y.1));
            let Some((g, _)) = best else { continue };
            taken[g] = true;
            let from = prev_doc.blocks[gone_blocks[g]];
            for op in diff(Algorithm::Myers, &prev_doc.keys(from.t0, from.t1), &doc.keys(b.t0, b.t1)) {
                if let DiffOp::Equal { old_index, new_index, len } = op {
                    let (o, n) = (from.t0 + old_index, b.t0 + new_index);
                    next[n..n + len].copy_from_slice(&prev[o..o + len]);
                    deleted[o..o + len].fill(false);
                }
            }
        }
    }
    for (k, &d) in deleted.iter().enumerate() {
        if d && prev_doc.toks[k].kind.counts() {
            *gone.entry(prev[k]).or_default() += 1;
        }
    }
    (next, gone)
}

/// [`walk`] over the published versions, kept: a text being typed only needs the last step from the newest version.
/// Who wrote each token, for each of a row of versions.
type Origins = Arc<Vec<Vec<u16>>>;

fn published_origins(texts: &[String], docs: &[Doc]) -> Origins {
    static WALKS: LazyLock<Mutex<HashMap<u64, Origins>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut hasher = DefaultHasher::new();
    texts.hash(&mut hasher);
    let key = hasher.finish();
    let mut cache = WALKS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = cache.get(&key) {
        return found.clone();
    }
    if cache.len() > 64 {
        cache.clear();
    }
    let origins = Arc::new(walk(docs).origins);
    cache.insert(key, origins.clone());
    origins
}

// ---- comparing two texts ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    None,
    /// Added, by the version at this index.
    Ins(u16),
    Del,
    /// Written by the version at this index (who wrote what, nothing compared).
    By(u16),
}

#[derive(Debug, Clone, Copy)]
struct Seg<'a> {
    text: &'a str,
    kind: Kind,
    mark: Mark,
}

/// What became of a block, between the two texts. Indices are of `a`'s blocks (the older) and `b`'s (the newer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Same(usize),
    Added(usize),
    Removed(usize),
    Changed(usize, usize),
    /// `b`'s block, which is `a`'s, from elsewhere.
    Moved(usize, usize),
    /// Where `a`'s block was before it moved.
    MovedAway(usize),
}

fn compatible(a: BlockKind, b: BlockKind) -> bool {
    a == b || (a.prose() && b.prose() && a.shown() && b.shown())
}

/// A block's tokens without the spaces.
fn spoken<'a>(d: &Doc<'a>, blk: &Block) -> Vec<&'a str> {
    (blk.t0..blk.t1).filter(|&i| d.toks[i].kind != Kind::Space).map(|i| d.tok(i)).collect()
}

/// How much of their words two blocks have in common, from 0 to 1.
fn likeness(a: &Doc, x: &Block, b: &Doc, y: &Block) -> f32 {
    let (p, q) = (spoken(a, x), spoken(b, y));
    if p.is_empty() && q.is_empty() {
        return 1.0;
    }
    let same: usize = diff(Algorithm::Myers, &p, &q)
        .iter()
        .map(|op| match op {
            DiffOp::Equal { len, .. } => *len,
            _ => 0,
        })
        .sum();
    2.0 * same as f32 / (p.len() + q.len()) as f32
}

fn compare_blocks(a: &Doc, b: &Doc) -> Vec<Item> {
    let ka: Vec<&str> = a.blocks.iter().map(|x| a.block_text(x).trim_end()).collect();
    let kb: Vec<&str> = b.blocks.iter().map(|x| b.block_text(x).trim_end()).collect();
    let mut items = Vec::new();
    let mut olds: Vec<usize> = Vec::new();
    let mut news: Vec<usize> = Vec::new();
    let flush = |olds: &mut Vec<usize>, news: &mut Vec<usize>, items: &mut Vec<Item>| {
        // Each new block takes the most alike old one after the last that was taken, if any is alike enough.
        let mut from = 0;
        for &n in news.iter() {
            let mut best: Option<(usize, f32)> = None;
            for (oi, &o) in olds.iter().enumerate().skip(from).take(40) {
                if !compatible(a.blocks[o].kind, b.blocks[n].kind) {
                    continue;
                }
                let s = likeness(a, &a.blocks[o], b, &b.blocks[n]);
                if s >= SAME_BLOCK && best.is_none_or(|(_, bs)| s > bs) {
                    best = Some((oi, s));
                }
            }
            match best {
                Some((oi, _)) => {
                    for &o in &olds[from..oi] {
                        items.push(Item::Removed(o));
                    }
                    items.push(Item::Changed(olds[oi], n));
                    from = oi + 1;
                }
                None => items.push(Item::Added(n)),
            }
        }
        for &o in &olds[from..] {
            items.push(Item::Removed(o));
        }
        olds.clear();
        news.clear();
    };
    for op in diff(Algorithm::Patience, &ka, &kb) {
        match op {
            DiffOp::Equal { new_index, len, .. } => {
                flush(&mut olds, &mut news, &mut items);
                items.extend((new_index..new_index + len).map(Item::Same));
            }
            DiffOp::Delete { old_index, old_len, .. } => olds.extend(old_index..old_index + old_len),
            DiffOp::Insert { new_index, new_len, .. } => news.extend(new_index..new_index + new_len),
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                olds.extend(old_index..old_index + old_len);
                news.extend(new_index..new_index + new_len);
            }
        }
    }
    flush(&mut olds, &mut news, &mut items);

    // A block that went out and came in again unchanged, somewhere else, moved. So did a heading, however short.
    let mut away: HashMap<&str, Vec<usize>> = HashMap::new();
    for item in &items {
        if let Item::Removed(o) = *item
            && (ka[o].len() >= MOVE_MIN_CHARS || a.blocks[o].kind == BlockKind::Heading)
        {
            away.entry(ka[o]).or_default().push(o);
        }
    }
    let mut moved: BTreeSet<usize> = BTreeSet::new();
    for item in &mut items {
        if let Item::Added(n) = *item
            && let Some(o) = away.get_mut(kb[n]).and_then(|v| (!v.is_empty()).then(|| v.remove(0)))
        {
            *item = Item::Moved(n, o);
            moved.insert(o);
        }
    }
    // And so did one that came in edited, when most of its words are those of one that went.
    let gone: Vec<usize> = items.iter().filter_map(|i| if let Item::Removed(o) = *i { Some(o) } else { None }).collect();
    let came = items.iter().filter(|i| matches!(i, Item::Added(_))).count();
    if came * gone.len() <= MOVE_PAIRS {
        for item in &mut items {
            let Item::Added(n) = *item else { continue };
            let y = b.blocks[n];
            if b.words(&y) < 4 {
                continue;
            }
            let best = gone
                .iter()
                .filter(|&&o| !moved.contains(&o) && compatible(a.blocks[o].kind, y.kind) && a.words(&a.blocks[o]) >= 4)
                .map(|&o| (o, likeness(a, &a.blocks[o], b, &y)))
                .filter(|&(_, l)| l >= MOVED_EDITED)
                .max_by(|x, y| x.1.total_cmp(&y.1));
            if let Some((o, _)) = best {
                *item = Item::Moved(n, o);
                moved.insert(o);
            }
        }
    }
    for item in &mut items {
        if let Item::Removed(o) = *item
            && moved.contains(&o)
        {
            *item = Item::MovedAway(o);
        }
    }
    items
}

/// The tokens of a changed block, old and new woven together.
fn changed_segs<'a>(a: &Doc<'a>, x: &Block, b: &Doc<'a>, y: &Block, origin: &[u16]) -> Vec<Seg<'a>> {
    let mut segs = Vec::new();
    let seg = |d: &Doc<'a>, i: usize, mark| Seg { text: d.tok(i), kind: d.toks[i].kind, mark };
    for op in diff(Algorithm::Myers, &a.keys(x.t0, x.t1), &b.keys(y.t0, y.t1)) {
        let (old, new) = (op.old_range(), op.new_range());
        match op.tag() {
            similar::DiffTag::Equal => segs.extend(new.map(|k| seg(b, y.t0 + k, Mark::None))),
            _ => {
                segs.extend(old.map(|k| seg(a, x.t0 + k, Mark::Del)));
                segs.extend(new.map(|k| seg(b, y.t0 + k, Mark::Ins(origin[y.t0 + k]))));
            }
        }
    }
    segs
}

fn block_segs<'a>(d: &Doc<'a>, x: &Block, mark: impl Fn(usize) -> Mark) -> Vec<Seg<'a>> {
    (x.t0..x.t1).map(|i| Seg { text: d.tok(i), kind: d.toks[i].kind, mark: mark(i) }).collect()
}

fn origin_char(o: u16) -> char {
    char::from_u32(0xE100 + u32::from(o.min(0x0DFF))).unwrap_or('\u{E100}')
}

/// Where a marked run starts, as a private-use character that [`expand`] turns into a tag once the markdown is HTML.
fn open(mark: Mark) -> String {
    match mark {
        Mark::Ins(o) => format!("\u{E000}{}", origin_char(o)),
        Mark::Del => "\u{E002}".into(),
        Mark::By(o) => format!("\u{E004}{}", origin_char(o)),
        _ => String::new(),
    }
}

fn close(mark: Mark) -> &'static str {
    match mark {
        Mark::Ins(_) => "\u{E001}",
        Mark::Del => "\u{E003}",
        Mark::By(_) => "\u{E005}",
        _ => "",
    }
}

/// The markdown of `segs`, with the marked runs marked. A mark never covers a line's start (`## `, `- `), emphasis or a
/// table's `|`, and never runs over the end of a line, so the markdown keeps its shape. What was taken out of the
/// structure itself (a list marker, a `**`) is left out: the text says what it is now.
fn emit(segs: &[Seg], code: bool) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < segs.len() {
        let s = segs[i];
        if !s.kind.markable() {
            match (s.kind, s.mark) {
                (Kind::Newline, Mark::Del) => out.push_str(if code { "\n" } else { " " }),
                (_, Mark::Del) => {}
                _ => out.push_str(s.text),
            }
            i += 1;
            continue;
        }
        if s.mark == Mark::None {
            out.push_str(s.text);
            i += 1;
            continue;
        }
        let mut j = i;
        while j < segs.len() && segs[j].kind.markable() && segs[j].mark == s.mark {
            j += 1;
        }
        let mut from = i;
        while from < j && segs[from].kind == Kind::Space {
            out.push_str(segs[from].text);
            from += 1;
        }
        let mut to = j;
        while to > from && segs[to - 1].kind == Kind::Space {
            to -= 1;
        }
        if from < to {
            out.push_str(&open(s.mark));
            for seg in &segs[from..to] {
                out.push_str(seg.text);
            }
            out.push_str(close(s.mark));
        }
        for seg in &segs[to..j] {
            out.push_str(seg.text);
        }
        i = j;
    }
    out
}

/// Where a version's words came from, as the views name it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// `None` for the live file.
    pub version: Option<String>,
    /// "ai", "human" or "unknown".
    pub by: &'static str,
    /// "Claude Code", "AI, as declared", "You", "Origin not recorded".
    pub who: String,
    pub at: String,
    pub prompt: Option<String>,
}

impl Source {
    fn title(&self) -> String {
        match &self.version {
            Some(v) => format!("{}, v{v}", self.who),
            None => format!("{}, not published yet", self.who),
        }
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Turns the marks [`emit`] put in the markdown into tags, now that it is HTML. A mark that ended up inside a tag (in
/// an attribute) is dropped, and every tag opened is closed, so a mark can never break the page.
fn expand(html: &str, sources: &[Source]) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut stack: Vec<&'static str> = Vec::new();
    let mut in_tag = false;
    let mut chars = html.chars();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                // A mark never outlasts the block it is in: one still open when the block ends is closed there.
                let rest = &html[html.len() - chars.as_str().len()..];
                if rest.starts_with('/') && BLOCK_ENDS.iter().any(|t| rest[1..].starts_with(t)) {
                    while let Some(tag) = stack.pop() {
                        let _ = write!(out, "</{tag}>");
                    }
                }
                in_tag = true;
                out.push(c);
            }
            '>' => {
                in_tag = false;
                out.push(c);
            }
            '\u{E000}' | '\u{E004}' => {
                let o = chars.next().map_or(0, |d| (d as u32).saturating_sub(0xE100) as usize);
                if in_tag {
                    continue;
                }
                let (tag, class) = if c == '\u{E000}' { ("ins", "dv-ins") } else { ("span", "dv-by") };
                let (by, title) = sources.get(o).map_or(("unknown", String::new()), |s| (s.by, s.title()));
                let _ = write!(out, "<{tag} class=\"{class}\" data-by=\"{by}\" data-o=\"{o}\" title=\"{}\">", escape(&title));
                stack.push(tag);
            }
            '\u{E002}' => {
                if !in_tag {
                    out.push_str("<del class=\"dv-del\">");
                    stack.push("del");
                }
            }
            '\u{E001}' | '\u{E003}' | '\u{E005}' => {
                let want = match c {
                    '\u{E001}' => "ins",
                    '\u{E003}' => "del",
                    _ => "span",
                };
                if !in_tag && stack.last() == Some(&want) {
                    stack.pop();
                    let _ = write!(out, "</{want}>");
                }
            }
            _ => out.push(c),
        }
    }
    while let Some(tag) = stack.pop() {
        let _ = write!(out, "</{tag}>");
    }
    out
}

/// The tags that end a block, after `</`.
const BLOCK_ENDS: [&str; 13] = ["p>", "li>", "h1>", "h2>", "h3>", "h4>", "h5>", "h6>", "td>", "th>", "pre>", "blockquote>", "code>"];

fn html_of(segs: &[Seg], code: bool, sources: &[Source]) -> String {
    expand(&markdown::render(&emit(segs, code)), sources)
}

/// The version most of these words come from.
fn majority(origins: impl Iterator<Item = u16>) -> Option<u16> {
    let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
    for o in origins {
        *counts.entry(o).or_default() += 1;
    }
    counts.into_iter().max_by_key(|&(o, n)| (n, o)).map(|(o, _)| o)
}

// ---- what the views get -----------------------------------------------------------------------------

/// One block of a document as a view shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShownBlock {
    /// "same", "added", "removed", "changed", "moved" or "movedAway".
    pub state: &'static str,
    pub html: String,
    /// The source (an index into `sources`) most of its words come from: what was added, or (who wrote what) all of it.
    pub by: Option<u16>,
}

/// Words, from one source.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub source: u16,
    pub words: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub text: String,
    /// The changed words of a changed line.
    pub emph: bool,
}

/// A line of the line-by-line comparison, as git shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// "same", "add" or "del".
    pub tag: &'static str,
    pub old: Option<usize>,
    pub new: Option<usize>,
    pub parts: Vec<Part>,
    /// Who wrote an added line.
    pub by: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// The two sides: a version, or `None` for the live file.
    pub from: Option<String>,
    pub to: Option<String>,
    pub sources: Vec<Source>,
    pub blocks: Vec<ShownBlock>,
    pub hunks: Vec<Hunk>,
    pub words_added: usize,
    pub words_removed: usize,
    pub moved: usize,
    pub changed: usize,
    /// The words added, by who wrote them.
    pub added_by: Vec<Share>,
}

/// What became of the words one version added.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    pub source: u16,
    pub wrote: usize,
    /// Still in the text shown.
    pub kept: usize,
    /// Taken out since, by each later version.
    pub removed_by: Vec<Share>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authorship {
    pub at: Option<String>,
    pub sources: Vec<Source>,
    pub blocks: Vec<ShownBlock>,
    /// The words in it, by who wrote them.
    pub words: Vec<Share>,
    pub trace: Option<Trace>,
}

// ---- the versions of a deliverable ------------------------------------------------------------------

/// The texts of a markdown deliverable, oldest first, and the live file last when it holds something no version does.
struct Chain {
    texts: Vec<String>,
    sources: Vec<Source>,
}

impl Chain {
    fn index(&self, which: &str) -> Result<usize> {
        let want = if which == "live" { None } else { Some(which.trim_start_matches('v')) };
        if want.is_none() && self.sources.last().is_some_and(|s| s.version.is_some()) {
            // The live file is the same as the newest version.
            return Ok(self.sources.len() - 1);
        }
        self.sources
            .iter()
            .position(|s| s.version.as_deref() == want)
            .ok_or_else(|| Error::not_found(format!("There is no version {which} of this deliverable")))
    }
}

fn body_of(conn: &Connection, project: &str, path: &str) -> Result<Option<String>> {
    Ok(query_opt(conn, "SELECT body FROM deliverables WHERE project_slug = ?1 AND path = ?2", [project, path], |r| r.get::<_, Option<String>>(0))?.flatten())
}

fn who(author: &str, run: bool) -> (&'static str, &'static str) {
    match author {
        provenance::AI if run => ("ai", "Claude Code"),
        provenance::AI => ("ai", "AI, as declared"),
        provenance::HUMAN => ("human", "You"),
        _ => ("unknown", "Origin not recorded"),
    }
}

fn prompt_title(conn: &Connection, id: Option<&str>) -> Result<Option<String>> {
    let Some(id) = id else { return Ok(None) };
    Ok(query_opt(conn, "SELECT title FROM prompts WHERE id = ?1", [id], |r| r.get::<_, String>(0))?.filter(|t| !t.trim().is_empty()))
}

/// Who wrote what the live file holds beyond the versions: a Claude Code run that changed it and published nothing, or you.
fn live_source(conn: &Connection, project: &str, base: &str) -> Result<Source> {
    let stopped = provenance::stopped_run(conn, project, base)?;
    let modified: String =
        query_opt(conn, "SELECT modified FROM deliverables WHERE project_slug = ?1 AND path = ?2", [project, base], |r| r.get(0))?.unwrap_or_default();
    let (by, who) = if stopped.is_some() { ("ai", "Claude Code") } else { ("human", "You") };
    Ok(Source { version: None, by, who: who.into(), at: modified, prompt: stopped.map(|s| s.prompt_title) })
}

fn chain(conn: &Connection, project: &str, base: &str) -> Result<Chain> {
    let base = split_version(base).0;
    if !deliverables::is_markdown(&base) {
        return Err(Error::invalid("Only a markdown deliverable can show what changed"));
    }
    let mut texts = Vec::new();
    let mut sources = Vec::new();
    for v in provenance::resolve(conn, project, &base)? {
        let Some(text) = body_of(conn, project, &deliverables::snapshot_path(&base, &v.version))? else {
            continue;
        };
        let (by, who) = who(v.author, v.run.is_some());
        texts.push(text);
        sources.push(Source { version: Some(v.version), by, who: who.into(), at: v.at, prompt: prompt_title(conn, v.prompt.as_deref())? });
    }
    if let Some(live) = body_of(conn, project, &base)?
        && texts.last() != Some(&live)
    {
        texts.push(live);
        sources.push(live_source(conn, project, &base)?);
    }
    if texts.is_empty() {
        return Err(Error::not_found("Deliverable not found"));
    }
    Ok(Chain { texts, sources })
}

/// The deliverable at `base` from `from` to `to` (each a version such as "0.3", or "live").
pub fn compare(conn: &Connection, project: &str, base: &str, from: &str, to: &str) -> Result<Comparison> {
    let chain = chain(conn, project, base)?;
    let (fi, ti) = (chain.index(from)?, chain.index(to)?);
    let docs: Vec<Doc> = chain.texts.iter().take(fi.max(ti) + 1).map(|t| Doc::new(t)).collect();
    let walk = walk(&docs[..=ti]);
    let (a, b) = (&docs[fi], &docs[ti]);
    let origin = &walk.origins[ti];
    let sources = &chain.sources;

    let mut blocks = Vec::new();
    let (mut words_added, mut words_removed, mut moved, mut changed) = (0, 0, 0, 0);
    let mut added: BTreeMap<u16, usize> = BTreeMap::new();
    for item in compare_blocks(a, b) {
        let shown = match item {
            Item::Same(n) | Item::Moved(n, _) | Item::Added(n) | Item::Changed(_, n) => b.blocks[n].kind.shown(),
            Item::Removed(o) | Item::MovedAway(o) => a.blocks[o].kind.shown(),
        };
        if !shown {
            continue;
        }
        blocks.push(match item {
            Item::Same(n) => ShownBlock { state: "same", html: markdown::render(b.block_text(&b.blocks[n])), by: None },
            Item::Added(n) => {
                let blk = b.blocks[n];
                let segs = block_segs(b, &blk, |i| Mark::Ins(origin[i]));
                let (plus, _, by) = count_added(&segs, &mut added);
                words_added += plus;
                ShownBlock { state: "added", html: html_of(&segs, blk.kind == BlockKind::Code, sources), by }
            }
            Item::Removed(o) => {
                words_removed += a.words(&a.blocks[o]);
                ShownBlock { state: "removed", html: markdown::render(a.block_text(&a.blocks[o])), by: None }
            }
            Item::Changed(o, n) => {
                let segs = changed_segs(a, &a.blocks[o], b, &b.blocks[n], origin);
                let (plus, minus, by) = count_added(&segs, &mut added);
                words_added += plus;
                words_removed += minus;
                changed += 1;
                ShownBlock { state: "changed", html: html_of(&segs, b.blocks[n].kind == BlockKind::Code, sources), by }
            }
            Item::Moved(n, o) => {
                moved += 1;
                let (x, y) = (a.blocks[o], b.blocks[n]);
                if a.block_text(&x).trim_end() == b.block_text(&y).trim_end() {
                    let by = majority((y.t0..y.t1).filter(|&i| b.toks[i].kind.counts()).map(|i| origin[i]));
                    ShownBlock { state: "moved", html: markdown::render(b.block_text(&y)), by }
                } else {
                    // Moved, and changed on the way.
                    let segs = changed_segs(a, &x, b, &y, origin);
                    let (plus, minus, by) = count_added(&segs, &mut added);
                    words_added += plus;
                    words_removed += minus;
                    ShownBlock { state: "moved", html: html_of(&segs, y.kind == BlockKind::Code, sources), by }
                }
            }
            Item::MovedAway(_) => ShownBlock { state: "movedAway", html: String::new(), by: None },
        });
    }

    Ok(Comparison {
        from: chain.sources[fi].version.clone(),
        to: chain.sources[ti].version.clone(),
        hunks: hunks(a, b, origin),
        sources: chain.sources.clone(),
        blocks,
        words_added,
        words_removed,
        moved,
        changed,
        added_by: added.into_iter().map(|(source, words)| Share { source, words }).collect(),
    })
}

/// The words a block's segments add and take out, and who added most of them; `added` counts them by who.
fn count_added(segs: &[Seg], added: &mut BTreeMap<u16, usize>) -> (usize, usize, Option<u16>) {
    let (mut plus, mut minus) = (0, 0);
    let mut by = Vec::new();
    for s in segs.iter().filter(|s| s.kind.counts()) {
        match s.mark {
            Mark::Ins(o) => {
                plus += 1;
                by.push(o);
                *added.entry(o).or_default() += 1;
            }
            Mark::Del => minus += 1,
            _ => {}
        }
    }
    (plus, minus, majority(by.into_iter()))
}

/// The line-by-line comparison, three lines of context around each change.
fn hunks(a: &Doc, b: &Doc, origin: &[u16]) -> Vec<Hunk> {
    let diff = TextDiff::configure().algorithm(Algorithm::Patience).deadline(Instant::now() + DEADLINE).diff_lines(a.text, b.text);
    // Who wrote each of the newer text's lines: who wrote most of its words.
    let mut line_of = Vec::with_capacity(b.toks.len());
    let mut line = 0;
    for t in &b.toks {
        line_of.push(line);
        if t.kind == Kind::Newline {
            line += 1;
        }
    }
    let mut by_line: HashMap<usize, Vec<u16>> = HashMap::new();
    for (i, t) in b.toks.iter().enumerate() {
        if t.kind.counts() {
            by_line.entry(line_of[i]).or_default().push(origin[i]);
        }
    }
    diff.grouped_ops(3)
        .into_iter()
        .map(|group| {
            let first = group.first();
            let mut lines = Vec::new();
            for op in &group {
                for change in diff.iter_inline_changes(op) {
                    let tag = match change.tag() {
                        ChangeTag::Equal => "same",
                        ChangeTag::Insert => "add",
                        ChangeTag::Delete => "del",
                    };
                    let parts = change
                        .iter_strings_lossy()
                        .map(|(emph, text)| Part { text: text.trim_end_matches(['\n', '\r']).to_string(), emph })
                        .filter(|p| !p.text.is_empty())
                        .collect();
                    let by = (tag == "add").then(|| change.new_index().and_then(|n| by_line.remove(&n)).and_then(|o| majority(o.into_iter()))).flatten();
                    lines.push(Line { tag, old: change.old_index().map(|i| i + 1), new: change.new_index().map(|i| i + 1), parts, by });
                }
            }
            Hunk { old_start: first.map_or(0, |op| op.old_range().start + 1), new_start: first.map_or(0, |op| op.new_range().start + 1), lines }
        })
        .collect()
}

/// Who wrote what is in the deliverable at `at` (a version, or "live"). With `focus` (a version) only what that version
/// wrote is marked, and the trace says what became of it.
pub fn authors(conn: &Connection, project: &str, base: &str, at: &str, focus: Option<&str>) -> Result<Authorship> {
    let chain = chain(conn, project, base)?;
    let ai = chain.index(at)?;
    let focus = match focus.filter(|f| !f.is_empty()) {
        Some(f) => {
            let fi = chain.index(f)?;
            if fi > ai {
                return Err(Error::invalid("That version came after the one shown"));
            }
            Some(u16::try_from(fi).unwrap_or(u16::MAX))
        }
        None => None,
    };
    let docs: Vec<Doc> = chain.texts.iter().take(ai + 1).map(|t| Doc::new(t)).collect();
    let walk = walk(&docs);
    let doc = &docs[ai];
    let origin = &walk.origins[ai];

    let mut words: BTreeMap<u16, usize> = BTreeMap::new();
    let mut blocks = Vec::new();
    for blk in doc.blocks.iter().filter(|b| b.kind.shown()) {
        let counted: Vec<u16> = (blk.t0..blk.t1).filter(|&i| doc.toks[i].kind.counts()).map(|i| origin[i]).collect();
        for &o in &counted {
            *words.entry(o).or_default() += 1;
        }
        let segs = block_segs(doc, blk, |i| match focus {
            Some(f) if origin[i] != f => Mark::None,
            _ => Mark::By(origin[i]),
        });
        let by = match focus {
            Some(f) => counted.contains(&f).then_some(f),
            None => majority(counted.into_iter()),
        };
        blocks.push(ShownBlock { state: "same", html: html_of(&segs, blk.kind == BlockKind::Code, &chain.sources), by });
    }

    let trace = focus.map(|f| {
        let fi = usize::from(f);
        let counts = |d: &Doc, o: &[u16]| (0..d.toks.len()).filter(|&i| o[i] == f && d.toks[i].kind.counts()).count();
        Trace {
            source: f,
            wrote: counts(&docs[fi], &walk.origins[fi]),
            kept: counts(doc, origin),
            removed_by: (fi + 1..=ai)
                .filter_map(|j| walk.removed[j].get(&f).map(|&w| Share { source: u16::try_from(j).unwrap_or(u16::MAX), words: w }))
                .collect(),
        }
    });

    Ok(Authorship {
        at: chain.sources[ai].version.clone(),
        sources: chain.sources,
        blocks,
        words: words.into_iter().map(|(source, words)| Share { source, words }).collect(),
        trace,
    })
}

// ---- the live file, as it is being typed ------------------------------------------------------------
// What the editor shows beside the text: each change since a version (the newest unless another is asked for), where it
// is in the text as it is now, who made it, what was there before, and the edit that undoes it. Positions are in UTF-16
// code units, as the editor counts.

/// Added text, `from..to`, by the source at `by`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub from: usize,
    pub to: usize,
    pub by: u16,
}

/// Text that was taken out, and where it was: inside a block, or a whole block (`block`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gone {
    pub at: usize,
    pub text: String,
    pub block: bool,
}

/// Replace `from..to` with `insert`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveChange {
    /// The same for the same change, while the text around it changes.
    pub id: String,
    /// "added", "changed", "removed" or "moved".
    pub kind: &'static str,
    /// The block it is in the text now; for "removed", `from == to` where the block was.
    pub from: usize,
    pub to: usize,
    /// Who made most of it (an index into `sources`); none for "removed".
    pub by: Option<u16>,
    /// What the block said before.
    pub old: String,
    pub inserts: Vec<Span>,
    pub deletes: Vec<Gone>,
    /// The edits that put back what was there, all in the coordinates of the text now.
    pub revert: Vec<Edit>,
    pub words_added: usize,
    pub words_removed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveChanges {
    /// The version compared with; none when there is no version yet.
    pub against: Option<String>,
    /// The versions, and last the text being typed.
    pub sources: Vec<Source>,
    pub changes: Vec<LiveChange>,
}

/// Byte offsets in a text as UTF-16 offsets.
struct Utf16(Vec<u32>);

impl Utf16 {
    fn new(text: &str) -> Utf16 {
        let mut table = vec![0u32; text.len() + 1];
        let mut at = 0u32;
        for (i, c) in text.char_indices() {
            table[i] = at;
            at += c.len_utf16() as u32;
        }
        table[text.len()] = at;
        Utf16(table)
    }

    fn at(&self, byte: usize) -> usize {
        self.0.get(byte).copied().unwrap_or_else(|| self.0.last().copied().unwrap_or(0)) as usize
    }
}

/// The added text among `range` of `d`'s tokens, as runs by who wrote them: a run stops at a line's start, emphasis and
/// the end of a line, and leaves out the spaces at its ends.
fn runs(d: &Doc, range: std::ops::Range<usize>, origin: &[u16], pos: &Utf16) -> Vec<Span> {
    let mut out = Vec::new();
    let mut open: Option<(usize, usize, u16)> = None;
    let close = |open: &mut Option<(usize, usize, u16)>, out: &mut Vec<Span>| {
        if let Some((from, to, by)) = open.take() {
            out.push(Span { from: pos.at(from), to: pos.at(to), by });
        }
    };
    for i in range {
        let t = d.toks[i];
        if !t.kind.markable() {
            close(&mut open, &mut out);
        } else if t.kind != Kind::Space {
            match open.as_mut() {
                Some((_, to, by)) if *by == origin[i] => *to = t.end,
                _ => {
                    close(&mut open, &mut out);
                    open = Some((t.start, t.end, origin[i]));
                }
            }
        }
    }
    close(&mut open, &mut out);
    out
}

/// What changed inside a block: the runs added, the text taken out and where, and the words of each.
fn inline_changes(a: &Doc, x: &Block, b: &Doc, y: &Block, origin: &[u16], pos: &Utf16) -> (Vec<Span>, Vec<Gone>, usize, usize, Vec<u16>) {
    let (mut inserts, mut deletes, mut plus, mut minus, mut by) = (Vec::new(), Vec::new(), 0, 0, Vec::new());
    for op in diff(Algorithm::Myers, &a.keys(x.t0, x.t1), &b.keys(y.t0, y.t1)) {
        if op.tag() == similar::DiffTag::Equal {
            continue;
        }
        let (old, new) = (op.old_range(), op.new_range());
        if !old.is_empty() {
            let text: String = old.clone().filter(|&k| a.toks[x.t0 + k].kind != Kind::Struct).map(|k| a.tok(x.t0 + k)).collect();
            minus += old.clone().filter(|&k| a.toks[x.t0 + k].kind.counts()).count();
            if !text.trim().is_empty() {
                let at = if y.t0 + new.start < y.t1 { b.toks[y.t0 + new.start].start } else { y.end };
                deletes.push(Gone { at: pos.at(at), text: text.trim().to_string(), block: false });
            }
        }
        if !new.is_empty() {
            for k in new.clone() {
                let kind = b.toks[y.t0 + k].kind;
                plus += usize::from(kind.counts());
                // Who added it goes by everything visible that was added, a full stop or an emoji too.
                if kind.markable() && kind != Kind::Space {
                    by.push(origin[y.t0 + k]);
                }
            }
            inserts.extend(runs(b, y.t0 + new.start..y.t0 + new.end, origin, pos));
        }
    }
    (inserts, deletes, plus, minus, by)
}

/// The deliverable at `base` as it is being typed (`text`) against a version (`against`, the newest when none).
pub fn live_changes(conn: &Connection, project: &str, base: &str, text: &str, against: Option<&str>) -> Result<LiveChanges> {
    let base = split_version(base).0;
    let live = live_source(conn, project, &base)?;
    let mut chain = match chain(conn, project, &base) {
        Ok(chain) => chain,
        Err(_) => return Ok(LiveChanges { against: None, sources: vec![live], changes: Vec::new() }),
    };
    if chain.sources.last().is_some_and(|s| s.version.is_none()) {
        chain.texts.pop();
        chain.sources.pop();
    }
    let published = chain.texts.len();
    if published == 0 {
        return Ok(LiveChanges { against: None, sources: vec![live], changes: Vec::new() });
    }
    let ai = match against.filter(|a| !a.is_empty() && *a != "live") {
        Some(v) => chain.index(v)?,
        None => published - 1,
    };
    let docs: Vec<Doc> = chain.texts.iter().map(|t| Doc::new(t)).collect();
    let before = published_origins(&chain.texts, &docs);
    let b = Doc::new(text);
    let here = u16::try_from(published).unwrap_or(u16::MAX);
    let (origin, _) = step(&docs[published - 1], &before[published - 1], &b, here);
    let a = &docs[ai];
    let pos = Utf16::new(text);
    let items = compare_blocks(a, &b);

    // Where a block that went would go back: before the next block that is there now, or at the end.
    let mut back: Vec<Option<usize>> = vec![None; items.len()];
    let mut next: Option<usize> = None;
    for (i, item) in items.iter().enumerate().rev() {
        back[i] = next;
        if let Item::Same(n) | Item::Added(n) | Item::Changed(_, n) | Item::Moved(n, _) = *item {
            next = Some(b.blocks[n].start);
        }
    }
    let put_back = |at: Option<usize>, old: &str| -> Edit {
        match at {
            Some(at) => Edit { from: pos.at(at), to: pos.at(at), insert: format!("{old}\n\n") },
            None => {
                let end = pos.at(text.trim_end().len());
                Edit { from: end, to: end, insert: if end == 0 { old.to_string() } else { format!("\n\n{old}") } }
            }
        }
    };
    let away: HashMap<usize, Option<usize>> =
        items.iter().enumerate().filter_map(|(i, item)| if let Item::MovedAway(o) = *item { Some((o, back[i])) } else { None }).collect();
    // Taking a block out takes the blank line after it too (or before it, for the last).
    let take_out = |n: usize| -> Edit {
        let y = b.blocks[n];
        let (from, to) = match (b.blocks.get(n + 1), n.checked_sub(1).map(|p| b.blocks[p])) {
            (Some(next), _) => (y.start, next.start),
            (None, Some(prev)) => (prev.end, y.end),
            (None, None) => (y.start, y.end),
        };
        Edit { from: pos.at(from), to: pos.at(to), insert: String::new() }
    };
    let id = |kind: &str, old: &str, new: &str| {
        let mut hasher = DefaultHasher::new();
        (kind, old, new).hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    let mut changes = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let change = match *item {
            Item::Same(_) | Item::MovedAway(_) => continue,
            Item::Added(n) => {
                let y = b.blocks[n];
                let words: Vec<u16> = (y.t0..y.t1).filter(|&k| b.toks[k].kind.counts()).map(|k| origin[k]).collect();
                LiveChange {
                    id: id("added", "", b.block_text(&y)),
                    kind: "added",
                    from: pos.at(y.start),
                    to: pos.at(y.end),
                    by: majority(words.iter().copied()),
                    old: String::new(),
                    inserts: runs(&b, y.t0..y.t1, &origin, &pos),
                    deletes: Vec::new(),
                    revert: vec![take_out(n)],
                    words_added: words.len(),
                    words_removed: 0,
                }
            }
            Item::Removed(o) => {
                let x = a.blocks[o];
                let old = a.block_text(&x).to_string();
                let edit = put_back(back[i], &old);
                LiveChange {
                    id: id("removed", &old, ""),
                    kind: "removed",
                    from: edit.from,
                    to: edit.from,
                    by: None,
                    deletes: vec![Gone { at: edit.from, text: old.clone(), block: true }],
                    old,
                    inserts: Vec::new(),
                    revert: vec![edit],
                    words_added: 0,
                    words_removed: a.words(&x),
                }
            }
            Item::Changed(o, n) | Item::Moved(n, o) => {
                let (x, y) = (a.blocks[o], b.blocks[n]);
                let old = a.block_text(&x).to_string();
                let moved = matches!(item, Item::Moved(..));
                let (inserts, deletes, plus, minus, by) = inline_changes(a, &x, &b, &y, &origin, &pos);
                // Who added most of it; a change that only took words out says nothing of who did (a block that only moved is its
                // writer's).
                let by = match (by.is_empty(), moved) {
                    (false, _) => majority(by.into_iter()),
                    (true, true) => majority((y.t0..y.t1).filter(|&k| b.toks[k].kind.counts()).map(|k| origin[k])),
                    (true, false) => None,
                };
                let in_place = Edit { from: pos.at(y.start), to: pos.at(y.end), insert: old.clone() };
                let revert = match away.get(&o).copied().filter(|_| moved) {
                    // Moved: taken out here and put back where it was, as it was, unless that is right here.
                    Some(at) => {
                        let (out, back) = (take_out(n), put_back(at, &old));
                        if back.from >= out.from && back.from <= out.to { vec![in_place] } else { vec![out, back] }
                    }
                    None => vec![in_place],
                };
                LiveChange {
                    id: id(if moved { "moved" } else { "changed" }, &old, b.block_text(&y)),
                    kind: if moved { "moved" } else { "changed" },
                    from: pos.at(y.start),
                    to: pos.at(y.end),
                    by,
                    old,
                    inserts,
                    deletes,
                    revert,
                    words_added: plus,
                    words_removed: minus,
                }
            }
        };
        changes.push(change);
    }
    chain.sources.push(live);
    Ok(LiveChanges { against: chain.sources[ai].version.clone(), sources: chain.sources, changes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_are_paragraphs_headings_items_and_code() {
        let doc = Doc::new(
            "---\ntitle: x\n---\n# Plan\n\nOne line\nand another.\n\n- a\n- b\n  more of b\n\n```js\nlet x;\n\nlet y;\n```\n| a | b |\n|---|---|\n| 1 | 2 |\n",
        );
        let kinds: Vec<BlockKind> = doc.blocks.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BlockKind::Meta, BlockKind::Heading, BlockKind::Para, BlockKind::Item, BlockKind::Item, BlockKind::Code, BlockKind::Table]);
        assert_eq!(doc.block_text(&doc.blocks[4]), "- b\n  more of b");
    }

    #[test]
    fn links_and_code_stay_whole() {
        let doc = Doc::new("See [the plan](plan.md), `a b` and [[Other note|that]].");
        let atoms: Vec<&str> = (0..doc.toks.len()).filter(|&i| doc.toks[i].kind == Kind::Atom).map(|i| doc.tok(i)).collect();
        assert_eq!(atoms, vec!["[the plan](plan.md)", "`a b`", "[[Other note|that]]"]);
    }

    #[test]
    fn who_wrote_what_follows_the_words_through_the_versions() {
        let v1 = "Intro by me.\n\nA paragraph the AI will move around later on.\n";
        let v2 = "Intro by me, with more.\n\nA paragraph the AI will move around later on.\n\nWritten by the AI.\n";
        let v3 = "Written by the AI.\n\nIntro by me, with more.\n\nA paragraph the AI will move around later on.\n";
        let docs = [Doc::new(v1), Doc::new(v2), Doc::new(v3)];
        let w = walk(&docs);
        let d = &docs[2];
        let of = |word: &str| (0..d.toks.len()).find(|&i| d.tok(i) == word).map(|i| w.origins[2][i]);
        assert_eq!(of("Written"), Some(1), "moved, still from v2");
        assert_eq!(of("Intro"), Some(0));
        assert_eq!(of("more"), Some(1));
    }

    #[test]
    fn marks_become_tags_and_never_break_the_html() {
        let sources = vec![Source { version: Some("0.1".into()), by: "ai", who: "Claude Code".into(), at: String::new(), prompt: None }];
        let doc = Doc::new("A [link](x.md) here\n");
        let segs = block_segs(&doc, &doc.blocks[0], |_| Mark::Ins(0));
        let html = html_of(&segs, false, &sources);
        assert!(
            html.contains("<ins class=\"dv-ins\" data-by=\"ai\" data-o=\"0\" title=\"Claude Code, v0.1\">A <a href=\"x.md\">link</a> here</ins>"),
            "{html}"
        );
        assert_eq!(expand("<p>\u{E001}x\u{E002}y</p>", &sources), "<p>x<del class=\"dv-del\">y</del></p>");
    }
}
