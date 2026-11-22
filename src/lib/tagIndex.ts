// The index of `#` tags the pickers offer: every project tag and person tag in use, from the vault's projects, the
// meeting notes and the feedback, each with how it reads and how often it is used; and the vault's people, for who was
// at a meeting.

import type { DailyNote, InboxFeedback, Person, ProjectMeta, ProjectPerson, ProjectSummary } from './types';
import type { TagOption } from './components/TagPicker.svelte';
import { feedbackTag, personName, personRole, roleName } from './v1';
import { plural } from './format';

const words = (tag: string) => tag.split('-').map((w, i) => (i === 0 ? w.charAt(0).toUpperCase() + w.slice(1) : w)).join(' ');

/** Project tags (`thrym-player`, shown as `#project/thrym-player`): the vault's projects first, by name. */
export function projectTagOptions(projects: ProjectSummary[], meta: ProjectMeta[], notes: DailyNote[] = [], feedback: InboxFeedback[] = []): TagOption[] {
	const uses = new Map<string, number>();
	const count = (t: string) => uses.set(t, (uses.get(t) ?? 0) + 1);
	for (const n of notes) for (const m of n.meetings) m.projects.forEach(count);
	for (const f of feedback) count(f.project);

	const names = new Map<string, string>();
	for (const m of meta) names.set(m.tag, words(m.tag));
	for (const t of uses.keys()) if (!names.has(t)) names.set(t, words(t));
	const inVault = new Set(projects.map((p) => p.slug));
	for (const p of projects) names.set(p.slug, p.name);

	return [...names]
		.map(([value, label]) => ({
			value,
			label,
			hint: [uses.get(value) ? plural(uses.get(value) ?? 0, 'use') : '', inVault.has(value) ? '' : 'not in this vault'].filter(Boolean).join(' | ')
		}))
		.sort((a, b) => Number(inVault.has(b.value)) - Number(inVault.has(a.value)) || a.label.localeCompare(b.label));
}

/** Person tags (`people/frank`): the people of the projects first (by their own name and role in each), then the others
 *  from the meetings and the feedback, most used first. */
export function personTagOptions(notes: DailyNote[] = [], feedback: InboxFeedback[] = [], people: ProjectPerson[] = []): TagOption[] {
	const uses = new Map<string, number>();
	const count = (t: string) => uses.set(t, (uses.get(t) ?? 0) + 1);
	for (const n of notes) for (const m of n.meetings) for (const id of m.people) count(`people/${id}`);
	const named = new Map<string, string>();
	for (const f of feedback) {
		const tag = feedbackTag(f);
		if (!tag || !f.from) continue;
		count(tag);
		named.set(tag, f.from);
	}
	const listed = new Map<string, ProjectPerson[]>();
	for (const p of people) listed.set(p.tag, [...(listed.get(p.tag) ?? []), p]);
	const own = [...listed].map(([value, of]) => ({
		value,
		label: of[0].name,
		hint: `${of.map((p) => `${roleName(p.role)} in ${p.project}`).join(', ')}${uses.get(value) ? ` | ${plural(uses.get(value) ?? 0, 'use')}` : ''}`
	}));
	const others = [...uses]
		.filter(([value]) => !listed.has(value))
		.sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
		.map(([value, n]) => ({ value, label: named.get(value) ?? personName(value), hint: [personRole(value), plural(n, 'use')].filter(Boolean).join(' | ') }));
	return [...own, ...others];
}

/** The vault's people, by id, for who was at a meeting: each with the roles they have in their projects, the people of
 *  the projects first, then everyone else, by name. */
export function personOptions(everyone: Person[], people: ProjectPerson[] = []): TagOption[] {
	const roles = (id: string) => people.filter((p) => p.person === id).map((p) => `${roleName(p.role)} in ${p.project}`);
	return [...everyone]
		.sort((a, b) => Number(roles(b.id).length > 0) - Number(roles(a.id).length > 0) || a.name.localeCompare(b.name))
		.map((p) => ({ value: p.id, label: p.name, hint: [roles(p.id).join(', '), p.organisation].filter(Boolean).join(' | ') || undefined }));
}
