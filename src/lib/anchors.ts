// Feedback on a passage: how a selection becomes something that can be found again, in any of the three ways a
// document is shown (the markdown itself in source mode and live preview, the rendered page in reading mode).
//
// What is kept is the selected text as plain text — the marks of markdown gone, white space collapsed — and a few words
// before and after it. Plain text is what the three views have in common: the rendered page never shows `**` or `[](…)`,
// so the markdown is boiled down to what it would show, with a note of where each letter came from.
// Kept free of Svelte and Tauri imports so it can be exercised on its own.

export interface Anchor {
	/** The selected text, plain. */
	text: string;
	/** The words just before it, and just after it: they tell apart a phrase that occurs more than once. */
	prefix: string;
	suffix: string;
}

/** A passage to show in the document: feedback's, or the one being commented on (`new`). `tone` says which. */
export interface Mark {
	id: string;
	anchor: Anchor;
	tone: 'open' | 'resolved' | 'new';
}

/** The most that can be commented on in one go (the backend refuses more). */
export const MAX_QUOTE = 2000;
/** How many characters of context are kept on each side. */
const CONTEXT = 40;

/** Text boiled down to plain, and for each of its letters the place in the original it came from. */
export interface Plain {
	text: string;
	/** `at[i]` is where `text[i]` stands in the original; one more at the end, the original's length. */
	at: number[];
	/** The original, to see what lies between the letters. */
	source: string;
}

const word = /[\p{L}\p{N}]/u;

/** A match of a global regular expression, whose place in the text is always known. */
type Hit = RegExpMatchArray & { index: number };

/** Which characters of markdown `source` stand for no text on the page: the marks around it. */
function marks(source: string): boolean[] {
	const gone: boolean[] = new Array(source.length).fill(false);
	const drop = (from: number, to: number) => {
		for (let i = from; i < to; i++) gone[i] = true;
	};
	// A backslash escapes the character after it, which then is text whatever it looks like.
	const literal = new Set<number>();
	for (const m of source.matchAll(/\\(?=[!-/:-@[-`{-~])/g) as IterableIterator<Hit>) {
		drop(m.index, m.index + 1);
		literal.add(m.index + 1);
	}
	const hit = (re: RegExp, each: (m: Hit) => void) => {
		for (const m of source.matchAll(re)) each(m as Hit);
	};

	// Whole lines that hold no text: code fences, rules, setext underlines, table separators.
	hit(/^[ \t]*(?:```|~~~).*$/gm, (m) => drop(m.index, m.index + m[0].length));
	hit(/^[ \t]*(?:[-*_][ \t]*){3,}$|^[ \t]*(?:=+|-{2,})[ \t]*$/gm, (m) => drop(m.index, m.index + m[0].length));
	hit(/^[ \t]*\|?[ \t]*:?-{2,}:?(?:[ \t]*\|[ \t]*:?-{2,}:?)*[ \t]*\|?[ \t]*$/gm, (m) => drop(m.index, m.index + m[0].length));
	// A picture shows no text; a link shows its words and not the address.
	hit(/!\[[^\]\n]*\]\([^)\n]*\)/g, (m) => drop(m.index, m.index + m[0].length));
	hit(/\[([^\]\n]*)\]\(([^)\n]*)\)/g, (m) => {
		drop(m.index, m.index + 1);
		drop(m.index + 1 + m[1].length, m.index + m[0].length);
	});
	// What starts a line: a heading's #, a quote's >, a list's bullet or number, a task's box.
	hit(/^[ \t]*(?:#{1,6}[ \t]+|(?:>[ \t]?)+|[-*+][ \t]+(?:\[[ xX]\][ \t]+)?|\d{1,9}[.)][ \t]+)/gm, (m) => drop(m.index, m.index + m[0].length));
	// The pipes of a table row.
	hit(/^[ \t]*\|.*\|[ \t]*$/gm, (m) => {
		for (let i = m.index; i < m.index + m[0].length; i++) if (source[i] === '|') gone[i] = true;
	});
	// Emphasis, code and strike-through. An underscore inside a word (snake_case) is text.
	for (let i = 0; i < source.length; i++) {
		if (literal.has(i)) continue;
		const c = source[i];
		if (c === '*' || c === '`') gone[i] = true;
		else if (c === '~' && source[i + 1] === '~') drop(i, i + 2);
		else if (c === '_' && !(word.test(source[i - 1] ?? '') && word.test(source[i + 1] ?? ''))) gone[i] = true;
	}
	return gone;
}

/**
 * `text` as plain text. With `markdown`, it is markdown, and what the page would not show is left out; without, it is
 * text already (what a rendered page holds). Either way runs of white space become one space.
 */
export function plain(text: string, markdown: boolean): Plain {
	const gone = markdown ? marks(text) : [];
	let out = '';
	const at: number[] = [];
	for (let i = 0; i < text.length; i++) {
		if (gone[i]) continue;
		if (/\s/.test(text[i])) {
			if (out.endsWith(' ')) continue;
			out += ' ';
		} else out += text[i];
		at.push(i);
	}
	at.push(text.length);
	return { text: out, at, source: text };
}

/** The first index of `at` that is `offset` or more. */
function lower(at: number[], offset: number): number {
	let lo = 0;
	let hi = at.length;
	while (lo < hi) {
		const mid = (lo + hi) >> 1;
		if (at[mid] < offset) lo = mid + 1;
		else hi = mid;
	}
	return lo;
}

/** The anchor for the part of `text` between `from` and `to` (offsets in `text`), or null when it holds no text. */
export function anchorOf(text: string, from: number, to: number, markdown: boolean): Anchor | null {
	const p = plain(text, markdown);
	let a = lower(p.at, from);
	let b = Math.min(lower(p.at, to), p.text.length);
	while (a < b && p.text[a] === ' ') a++;
	while (b > a && p.text[b - 1] === ' ') b--;
	if (b <= a) return null;
	return { text: p.text.slice(a, b), prefix: p.text.slice(Math.max(0, a - CONTEXT), a), suffix: p.text.slice(b, b + CONTEXT) };
}

const sameEnd = (a: string, b: string) => {
	let n = 0;
	while (n < a.length && n < b.length && a[a.length - 1 - n] === b[b.length - 1 - n]) n++;
	return n;
};
const sameStart = (a: string, b: string) => {
	let n = 0;
	while (n < a.length && n < b.length && a[n] === b[n]) n++;
	return n;
};

/**
 * Where the passage is now, as offsets in the text `p` was made from, or null when it is no longer there. A phrase that
 * occurs more than once is told apart by the words around it.
 */
export function locate(p: Plain, anchor: Anchor): { from: number; to: number } | null {
	const want = anchor.text.replace(/\s+/g, ' ').trim();
	if (!want) return null;
	let best = -1;
	let bestScore = -1;
	for (let i = p.text.indexOf(want); i !== -1; i = p.text.indexOf(want, i + 1)) {
		const score = sameEnd(p.text.slice(Math.max(0, i - anchor.prefix.length), i), anchor.prefix) + sameStart(p.text.slice(i + want.length), anchor.suffix);
		if (score > bestScore) {
			best = i;
			bestScore = score;
		}
	}
	if (best < 0) return null;
	// The marks that touch the passage belong to it: **bold**, a link's [ and ](address), a list's bullet.
	const kept = (i: number) => p.at[lower(p.at, i)] === i;
	const mark = (i: number) => !/\s/.test(p.source[i]) && !kept(i);
	let from = p.at[best];
	let to = p.at[best + want.length - 1] + 1;
	while (from > 0 && mark(from - 1)) from--;
	while (to < p.source.length && mark(to)) to++;
	return { from, to };
}

// ---- the rendered page ---------------------------------------------------------------------------------
// Reading mode shows HTML, whose text is the text nodes in order. Offsets into it are counted the way a Range counts
// its text, so a selection can be turned into offsets, and offsets back into a Range.

/** The text of everything under `root`, text node after text node. */
export function textOf(root: Node): string {
	let text = '';
	const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
	for (let n = walk.nextNode(); n; n = walk.nextNode()) text += n.nodeValue ?? '';
	return text;
}

/** How many characters of text come before the point (`node`, `offset`) under `root`. */
export function offsetIn(root: Node, node: Node, offset: number): number {
	const range = document.createRange();
	range.selectNodeContents(root);
	range.setEnd(node, offset);
	return range.toString().length;
}

/** The Range over characters `from` to `to` of the text under `root`. */
export function rangeIn(root: Node, from: number, to: number): Range | null {
	const range = document.createRange();
	let seen = 0;
	let started = false;
	const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
	for (let n = walk.nextNode(); n; n = walk.nextNode()) {
		const length = (n.nodeValue ?? '').length;
		if (!started && from < seen + length) {
			range.setStart(n, from - seen);
			started = true;
		}
		if (started && to <= seen + length) {
			range.setEnd(n, to - seen);
			return range;
		}
		seen += length;
	}
	return null;
}
