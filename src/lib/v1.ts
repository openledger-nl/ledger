// Words and small sums for the v1 screens: the names of project kinds, post kinds, labels and roles, how a
// person tag reads, and the days a project has left. Free of Svelte and Tauri, like mmm.ts.

import type { EpistemicLabel, FeedbackRole, PersonTag, PostKind, ProjectKind, ProjectMeta, ProjectPerson, Semester } from './types';
import { parseDay, today } from './semesters';

export const PROJECT_KINDS: { kind: ProjectKind; name: string; hint: string }[] = [
	{ kind: 'study', name: 'Study', hint: 'A personal or group project in Open Learning: semesters, sprints and Portflow when switched on.' },
	{ kind: 'hobby', name: 'Hobby', hint: 'Something of your own: no study fields, an end date only if you want one.' },
	{ kind: 'company', name: 'Company', hint: 'Work for a client or your own business: client feedback, agreements, versions.' }
];
export const kindName = (kind: ProjectKind) => PROJECT_KINDS.find((k) => k.kind === kind)?.name ?? kind;

export const POST_KINDS: { kind: PostKind; name: string }[] = [
	{ kind: 'thought', name: 'Thought' },
	{ kind: 'devlog', name: 'Devlog' },
	{ kind: 'research', name: 'Research' },
	{ kind: 'reflection', name: 'Reflection' }
];
export const postKindName = (kind: PostKind) => POST_KINDS.find((k) => k.kind === kind)?.name ?? kind;

/** Set by hand only: Ledger never labels a post (you could fool yourself). */
export const LABELS: { label: EpistemicLabel; name: string; hint: string }[] = [
	{ label: 'fact', name: 'Fact', hint: 'Checked, with a source.' },
	{ label: 'to-check', name: 'To check', hint: 'Probably true, still to be checked.' },
	{ label: 'assumption', name: 'Assumption', hint: 'What you take for true for now.' }
];
export const labelName = (label: EpistemicLabel) => LABELS.find((l) => l.label === label)?.name ?? label;

export const ROLES: { role: FeedbackRole; name: string }[] = [
	{ role: 'coach', name: 'Coach' },
	{ role: 'expert', name: 'Expert' },
	{ role: 'teammate', name: 'Teammate' },
	{ role: 'client', name: 'Client' },
	{ role: 'colleague', name: 'Colleague' },
	{ role: 'other', name: 'Someone else' }
];
export const roleName = (role: FeedbackRole | null) => (role ? (ROLES.find((r) => r.role === role)?.name ?? role) : 'You');

/** How feedback was given, as kept ("meeting") and as said ("In a meeting"); Portflow's own is set by Portflow. */
export const GIVEN_WAYS: { given: string; name: string }[] = [
	{ given: 'in person', name: 'In person' },
	{ given: 'meeting', name: 'In a meeting' },
	{ given: 'email', name: 'By email' },
	{ given: 'phone', name: 'By phone' },
	{ given: 'other', name: 'Otherwise' }
];
export const givenName = (given: string | null) => (given === 'portflow' ? 'In Portflow' : (GIVEN_WAYS.find((g) => g.given === given)?.name ?? null));

/** `expert/sander` → `Sander`; `client/police/ries` → `Ries`. */
export function personName(tag: PersonTag): string {
	const last = tag.split('/').pop() ?? tag;
	return last.charAt(0).toUpperCase() + last.slice(1);
}

/** The tags of the whole team of these projects: every teammate, each once, in the order they are listed. */
export function teamTags(people: ProjectPerson[], projects: string[]): PersonTag[] {
	return [...new Set(people.filter((p) => p.role === 'teammate' && projects.includes(p.project)).map((p) => p.tag))];
}

/** A name as the backend makes it a file or tag name (ids.rs `slugify`): lowercase ASCII letters and digits, the rest
 *  one dash. "01 Project plan" → "01-project-plan". */
export const slugify = (name: string) =>
	name
		.trim()
		.toLowerCase()
		.split(/[^a-z0-9]+/)
		.filter(Boolean)
		.join('-');

/** The tag someone goes by, as the backend forms it: `people/frank-de-vries` (their name as a slug, which is the id a
 *  person of the vault gets). */
export const personTag = (name: string): PersonTag => `people/${slugify(name) || 'someone'}`;

/** The person tag of whoever gave a piece of feedback; null for your own notes and feedback that names nobody. */
export const feedbackTag = (f: { from: string | null }) => (f.from ? personTag(f.from) : null);

/** What a person tag says besides the name: `client/police/ries` reads `client | police`; `people/ries` nothing. */
export const personRole = (tag: PersonTag) =>
	tag
		.split('/')
		.slice(0, -1)
		.filter((part, i) => !(i === 0 && part === 'people'))
		.join(' | ');

/** A tag as typed: lowercase, no `#`, words joined by dashes, levels by slashes. */
export const cleanTag = (text: string) =>
	text
		.trim()
		.replace(/^#/, '')
		.toLowerCase()
		.split('/')
		.map((part) => part.trim().replace(/\s+/g, '-'))
		.filter(Boolean)
		.join('/');

/** The day a project ends: its own day, or the end of its semester; null when it has none. */
export function endDay(meta: ProjectMeta, semesters: Semester[]): string | null {
	if (meta.end === 'semester') return semesters.find((s) => s.name === meta.semester)?.end ?? null;
	return meta.end;
}

/** Calendar days from today to the end, inclusive; negative once it has passed. */
export function daysLeft(end: string | null, day = today()): number | null {
	const a = parseDay(day);
	const b = end ? parseDay(end) : null;
	if (!a || !b) return null;
	return Math.round((b.getTime() - a.getTime()) / 86_400_000);
}

export function daysLeftText(n: number | null): string {
	if (n === null) return 'No end date';
	if (n < 0) return `Ended ${-n} ${-n === 1 ? 'day' : 'days'} ago`;
	if (n === 0) return 'Ends today';
	return `${n} ${n === 1 ? 'day' : 'days'} left`;
}
