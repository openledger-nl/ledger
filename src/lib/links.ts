// [[links]]: what a typed target points at, and what to offer while one is being typed. The rules live here and nowhere
// else, so the editor (its live preview, its Ctrl/⌘+click, its completion) and the rendered page agree. The list of
// things a link can point at comes from the backend (`list_links`, links.rs): the project's threads, deliverables and
// feedback. Plain TypeScript, with nothing of the app in it.
//
//   [[Pricing model]]            a thread, by its title (or its slug)
//   [[pricing-brief]]            a deliverable, by file name; `[[plans/pricing-brief.md]]` by path
//   [[feedback:1730…-ab12cd34]]  a piece of feedback, by id
//   [[target|words to show]]     any of them, shown as other words
//   [[thread:Name]]              the kind spelled out, for when a thread and a deliverable have the same name

import type { LinkTarget } from './types';

export type LinkKind = LinkTarget['kind'];

const KINDS: LinkKind[] = ['thread', 'deliverable', 'feedback'];

/** A link in the text as the editor reads it: `[[target]]` or `[[target|shown]]` (a pipe may be escaped, in a table cell). */
export const WIKI_LINK = /\[\[([^[\]|\n\\]+)(?:\\?\|([^[\]\n]*))?\]\]/g;

/** What is typed between the brackets, split into the target and the words shown instead (if any). */
export function parseWikiLink(inner: string): { target: string; alias: string | null } {
	const bar = inner.search(/\\?\|/);
	if (bar < 0) return { target: inner.trim(), alias: null };
	return { target: inner.slice(0, bar).trim(), alias: inner.slice(inner.indexOf('|', bar) + 1).trim() || null };
}

/** How targets are compared: no difference made for case or for runs of white space. */
const fold = (text: string) => text.trim().replace(/\s+/g, ' ').toLowerCase();

const isMarkdownPath = (path: string) => /\.(md|markdown)$/i.test(path);
const withoutExtension = (path: string) => (isMarkdownPath(path) ? path.replace(/\.[^./]+$/, '') : path);
const fileName = (path: string) => path.slice(path.lastIndexOf('/') + 1);

/** Whether text can stand between the brackets of a link as it is. */
const usable = (text: string) => text.trim() !== '' && !/[[\]|\\\n]/.test(text);

/** Squeezes text into something that can be shown in a link: one line, nothing that would end it. */
const shown = (text: string) => text.replace(/[[\]|\\]/g, ' ').replace(/\s+/g, ' ').trim();

export interface Suggestion {
	target: LinkTarget;
	/** What goes between the brackets. */
	insert: string;
	/** What it is called in the list. */
	label: string;
	/** What tells it from the rest: its kind, and where it is. */
	detail: string;
}

export class LinkIndex {
	private readonly names = new Map<string, LinkTarget>();
	private readonly byKind: Record<LinkKind, Map<string, LinkTarget>> = { thread: new Map(), deliverable: new Map(), feedback: new Map() };

	constructor(readonly targets: LinkTarget[]) {
		for (const target of targets) {
			for (const name of this.namesOf(target)) {
				if (!this.names.has(name)) this.names.set(name, target);
				if (!this.byKind[target.kind].has(name)) this.byKind[target.kind].set(name, target);
			}
		}
	}

	/** Every way a target can be written, folded. The first target to claim a name keeps it. */
	private namesOf(t: LinkTarget): string[] {
		switch (t.kind) {
			case 'thread':
				return [fold(t.title), fold(t.id)];
			case 'deliverable':
				return [fold(t.id), fold(withoutExtension(t.id)), fold(fileName(t.id)), fold(withoutExtension(fileName(t.id)))];
			case 'feedback':
				return [fold(t.id)];
		}
	}

	/** The thing a typed target means, if there is one. `feedback:` and the other kinds restrict it to that kind. */
	resolve(typed: string): LinkTarget | null {
		const direct = this.lookup(typed);
		if (direct) return direct;
		// `[[Page#Heading]]`: the part after # is for a place within the page, which these pages don't have yet.
		const hash = typed.indexOf('#');
		return hash > 0 ? this.lookup(typed.slice(0, hash)) : null;
	}

	private lookup(typed: string): LinkTarget | null {
		const text = fold(typed);
		const kind = KINDS.find((k) => text.startsWith(`${k}:`));
		// A title may itself start like a kind ("Feedback: what next"): if nothing of that kind is meant, the whole text is the name.
		const named = kind ? this.byKind[kind].get(fold(text.slice(kind.length + 1))) : undefined;
		return named ?? this.names.get(text) ?? null;
	}

	/** What to write for a target: the plainest form that comes back to it, else with the kind spelled out. */
	nameFor(t: LinkTarget): string {
		const candidates =
			t.kind === 'thread'
				? [t.title, t.id]
				: t.kind === 'deliverable'
					? [withoutExtension(fileName(t.id)), fileName(t.id), withoutExtension(t.id), t.id]
					: [];
		for (const name of candidates) if (usable(name) && this.resolve(name) === t) return name.trim();
		if (t.kind === 'feedback') return `feedback:${t.id}`;
		return `${t.kind}:${t.id}`;
	}

	/** What the editor offers after `[[` and `query`: the best matches first, and with nothing typed the newest of each kind. */
	suggest(query: string, limit = 40): Suggestion[] {
		const typed = query.trim();
		const kind = KINDS.find((k) => fold(typed).startsWith(`${k}:`));
		const needle = fold(kind ? typed.slice(kind.length + 1) : typed);
		const pool = this.targets.filter((t) => !kind || t.kind === kind);

		let chosen: LinkTarget[];
		if (!needle) {
			const taken: Record<LinkKind, number> = { thread: 0, deliverable: 0, feedback: 0 };
			chosen = pool.filter((t) => taken[t.kind]++ < 10);
		} else {
			chosen = pool
				.map((t, order) => ({ t, order, score: this.score(t, needle) }))
				.filter((m) => m.score < Infinity)
				.sort((a, b) => a.score - b.score || a.order - b.order)
				.map((m) => m.t);
		}
		return chosen.slice(0, limit).map((t) => this.suggestion(t));
	}

	private suggestion(t: LinkTarget): Suggestion {
		const name = this.nameFor(t);
		if (t.kind === 'feedback') {
			// Feedback has no name of its own: the link says what it says.
			return { target: t, insert: `${name}|${shown(t.title)}`, label: t.title, detail: `feedback ${t.detail}` };
		}
		const kind = t.kind === 'thread' ? 'thread' : 'deliverable';
		return { target: t, insert: name, label: t.title, detail: t.detail && t.kind === 'deliverable' ? `${kind} in ${t.detail}` : kind };
	}

	/** How well a target matches what is typed: lower is better, infinity is no match. */
	private score(t: LinkTarget, needle: string): number {
		const own = fold(t.title);
		const where = t.kind === 'deliverable' ? fold(t.id) : fold(t.detail);
		if (own === needle || where === needle) return 0;
		if (own.startsWith(needle)) return 1;
		if (own.split(/[\s\-_./]+/).some((word) => word.startsWith(needle))) return 2;
		if (own.includes(needle)) return 3;
		if (t.kind === 'deliverable' && where.includes(needle)) return 4;
		if (t.kind === 'feedback' && where.includes(needle)) return 5;
		return subsequence(own, needle) ? 6 : Infinity;
	}
}

/** Whether the letters of `needle` occur in `text` in order, not necessarily next to each other. */
function subsequence(text: string, needle: string): boolean {
	let at = 0;
	for (const c of needle) {
		at = text.indexOf(c, at) + 1;
		if (!at) return false;
	}
	return true;
}
