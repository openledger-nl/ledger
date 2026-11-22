// [[links]]: what a typed target points at, and what to offer while one is being typed. The rules live here, so the
// editor (its live preview, its Ctrl/⌘+click, its completion) and the rendered page agree; links.rs follows the same
// rules to keep links working when a thread or deliverable is renamed. The list of things a link can point at comes
// from the backend (`list_links`, links.rs): every project's threads, deliverables and feedback, the days of the
// meeting notes and the vault's people. Plain TypeScript, with nothing of the app in it.
//
// A link is read from where it is written: its own project (the project of the page, or a meeting's only project).
// Names there need nothing more; another project's things start with that project's slug.
//
//   [[Pricing model]]                a thread, by its title (or its slug)
//   [[pricing-brief]]                a deliverable, by file name; `[[plans/pricing-brief.md]]` by path
//   [[feedback:1730…-ab12cd34]]      a piece of feedback, by id
//   [[thrym-player/pricing-brief]]   any of those in another project, after its slug (only for another project's)
//   [[people/frank]]                 a person, by id: their file in shared/people
//   [[2026-11-22]]                   a day's meeting notes, by its date (as Obsidian links a daily note)
//   [[target|words to show]]         any of them, shown as other words
//   [[thread:Name]]                  the kind spelled out, for when a thread and a deliverable have the same name

import type { LinkTarget } from './types';

export type LinkKind = LinkTarget['kind'];
type ProjectKind = 'thread' | 'deliverable' | 'feedback';

const KINDS: LinkKind[] = ['thread', 'deliverable', 'feedback', 'meeting', 'person'];
const PROJECT_KINDS: ProjectKind[] = ['thread', 'deliverable', 'feedback'];

/** What a link to a person starts with: their file is `shared/people/<id>.md`. */
export const PEOPLE = 'people/';

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

const inProject = (t: LinkTarget): t is LinkTarget & { kind: ProjectKind } => (PROJECT_KINDS as string[]).includes(t.kind);

export interface Suggestion {
	target: LinkTarget;
	/** What goes between the brackets. */
	insert: string;
	/** What it is called in the list. */
	label: string;
	/** What tells it from the rest: its kind, and where it is. */
	detail: string;
}

/** The names of one project's things: the first target to claim a name keeps it. */
class Names {
	readonly all = new Map<string, LinkTarget>();
	readonly byKind: Record<ProjectKind, Map<string, LinkTarget>> = { thread: new Map(), deliverable: new Map(), feedback: new Map() };

	add(t: LinkTarget & { kind: ProjectKind }) {
		const names =
			t.kind === 'thread'
				? [fold(t.title), fold(t.id)]
				: t.kind === 'deliverable'
					? [fold(t.id), fold(withoutExtension(t.id)), fold(fileName(t.id)), fold(withoutExtension(fileName(t.id)))]
					: [fold(t.id)];
		for (const name of names) {
			if (!this.all.has(name)) this.all.set(name, t);
			if (!this.byKind[t.kind].has(name)) this.byKind[t.kind].set(name, t);
		}
	}

	/** `feedback:` and the other kinds restrict a name to that kind. */
	find(text: string): LinkTarget | null {
		const kind = PROJECT_KINDS.find((k) => text.startsWith(`${k}:`));
		// A title may itself start like a kind ("Feedback: what next"): if nothing of that kind is meant, the whole text is the name.
		const named = kind ? this.byKind[kind].get(fold(text.slice(kind.length + 1))) : undefined;
		return named ?? this.all.get(text) ?? null;
	}
}

export class LinkIndex {
	private readonly projects = new Map<string, Names>();
	private readonly days = new Map<string, LinkTarget>();
	private readonly people = new Map<string, LinkTarget>();

	/** `home` is the project links are written in (null outside one, or for a meeting of several projects or none). */
	constructor(
		readonly targets: LinkTarget[],
		readonly home: string | null = null
	) {
		for (const t of targets) {
			if (inProject(t)) {
				const names = this.projects.get(t.project) ?? new Names();
				this.projects.set(t.project, names);
				names.add(t);
			} else if (t.kind === 'meeting') {
				if (!this.days.has(fold(t.id))) this.days.set(fold(t.id), t);
			} else if (!this.people.has(fold(t.id))) {
				this.people.set(fold(t.id), t);
			}
		}
	}

	/** Whether `t` is in a project other than the one links are written in, so a link to it names that project. */
	private foreign(t: LinkTarget): boolean {
		return inProject(t) && t.project !== this.home;
	}

	/** The thing a typed target means, if there is one. */
	resolve(typed: string): LinkTarget | null {
		const direct = this.lookup(typed);
		if (direct) return direct;
		// `[[Page#Heading]]`: the part after # is for a place within the page, which these pages don't have yet.
		const hash = typed.indexOf('#');
		return hash > 0 ? this.lookup(typed.slice(0, hash)) : null;
	}

	private lookup(typed: string): LinkTarget | null {
		const text = fold(typed);
		if (text.startsWith(PEOPLE)) return this.people.get(text.slice(PEOPLE.length)) ?? null;
		if (text.startsWith('person:')) return this.people.get(fold(text.slice('person:'.length))) ?? null;
		const own = this.home ? this.projects.get(this.home)?.find(text) : null;
		if (own) return own;
		const day = this.days.get(text.startsWith('meeting:') ? fold(text.slice('meeting:'.length)) : text);
		if (day) return day;
		// Another project's: its slug, a slash, and the name as that project writes it.
		const slash = text.indexOf('/');
		return slash > 0 ? (this.projects.get(text.slice(0, slash))?.find(text.slice(slash + 1)) ?? null) : null;
	}

	/** What to write for a target: the plainest form that comes back to it, else with the kind spelled out; another
	 *  project's after its slug. */
	nameFor(t: LinkTarget): string {
		if (t.kind === 'person') return `${PEOPLE}${t.id}`;
		if (t.kind === 'meeting') return t.id;
		const candidates =
			t.kind === 'thread' ? [t.title, t.id] : t.kind === 'deliverable' ? [withoutExtension(fileName(t.id)), fileName(t.id), withoutExtension(t.id), t.id] : [];
		const prefix = this.foreign(t) ? `${t.project}/` : '';
		for (const name of [...candidates, `${t.kind}:${t.id}`]) {
			if (!usable(name)) continue;
			const written = prefix + name.trim();
			if (this.resolve(written) === t) return written;
		}
		return `${prefix}${t.kind}:${t.id}`;
	}

	/** What the editor offers after `[[` and `query`: the best matches first, the link's own project before the others,
	 *  and with nothing typed the newest of each kind. `people/` and a project's slug and `/` narrow it to those. */
	suggest(query: string, limit = 40): Suggestion[] {
		let typed = query.trim();
		let pool: LinkTarget[];
		let narrowed = false;
		const slash = fold(typed).indexOf('/');
		const project = slash > 0 ? fold(typed).slice(0, slash) : null;
		if (fold(typed).startsWith(PEOPLE)) {
			pool = [...this.people.values()];
			typed = typed.slice(PEOPLE.length);
			narrowed = true;
		} else if (project && project !== this.home && this.projects.has(project)) {
			pool = this.targets.filter((t) => inProject(t) && t.project === project);
			typed = typed.slice(slash + 1);
			narrowed = true;
		} else {
			// The link's own project first, then the meeting days and the people, then everything else.
			const rank = (t: LinkTarget) => (inProject(t) ? (this.foreign(t) ? 2 : 0) : 1);
			pool = this.targets.filter((t) => t.kind !== 'meeting' || this.days.get(fold(t.id)) === t).sort((a, b) => rank(a) - rank(b));
		}
		const kind = KINDS.find((k) => fold(typed).startsWith(`${k}:`));
		const needle = fold(kind ? typed.slice(kind.length + 1) : typed);
		pool = pool.filter((t) => !kind || t.kind === kind);

		let chosen: LinkTarget[];
		if (!needle) {
			// Another project's things come once its name or theirs is typed, unless there is no project of its own.
			const near = narrowed || !this.home ? pool : pool.filter((t) => !this.foreign(t));
			const taken: Record<LinkKind, number> = { thread: 0, deliverable: 0, feedback: 0, meeting: 0, person: 0 };
			chosen = near.filter((t) => taken[t.kind]++ < 10);
		} else {
			chosen = pool
				.map((t, order) => ({ t, order, score: this.score(t, needle) }))
				.filter((m) => m.score < Infinity)
				.sort((a, b) => a.score - b.score || Number(this.foreign(a.t)) - Number(this.foreign(b.t)) || a.order - b.order)
				.map((m) => m.t);
		}
		return chosen.slice(0, limit).map((t) => this.suggestion(t));
	}

	private suggestion(t: LinkTarget): Suggestion {
		const name = this.nameFor(t);
		const elsewhere = this.foreign(t) ? ` in ${t.project}` : '';
		if (t.kind === 'feedback') {
			// Feedback has no name of its own: the link says what it says.
			return { target: t, insert: `${name}|${shown(t.title)}`, label: t.title, detail: `feedback ${t.detail}${elsewhere}` };
		}
		if (t.kind === 'meeting') return { target: t, insert: name, label: t.id, detail: t.title ? `meeting notes: ${t.title}` : 'meeting notes' };
		if (t.kind === 'person') return { target: t, insert: name, label: t.title, detail: t.detail ? `person | ${t.detail}` : 'person' };
		if (t.kind === 'thread') return { target: t, insert: name, label: t.title, detail: `thread${elsewhere}` };
		const where = [this.foreign(t) ? t.project : '', t.detail].filter(Boolean).join('/');
		return { target: t, insert: name, label: t.title, detail: where ? `deliverable in ${where}` : 'deliverable' };
	}

	/** How well a target matches what is typed: lower is better, infinity is no match. */
	private score(t: LinkTarget, needle: string): number {
		const own = fold(t.title);
		const where = t.kind === 'deliverable' || t.kind === 'person' ? fold(t.id) : fold(t.detail);
		if (own === needle || where === needle) return 0;
		if (own.startsWith(needle)) return 1;
		if (own.split(/[\s\-_./]+/).some((word) => word.startsWith(needle))) return 2;
		if (own.includes(needle)) return 3;
		if ((t.kind === 'deliverable' || t.kind === 'meeting' || t.kind === 'person') && where.includes(needle)) return 4;
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
