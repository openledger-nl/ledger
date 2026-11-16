// The project tags the preview screens offer (`#project/<tag>`), and how each reads: the vault's projects by their
// slug and name, plus the tags the mock data uses for projects that aren't in this vault (shown by their tag, in words).

import type { DailyNote, ProjectMeta, ProjectSummary } from './types';

const words = (tag: string) => tag.split('-').map((w, i) => (i === 0 ? w.charAt(0).toUpperCase() + w.slice(1) : w)).join(' ');

export function projectTagNames(projects: ProjectSummary[], meta: ProjectMeta[], notes: DailyNote[] = []) {
	const names = new Map<string, string>();
	for (const m of meta) names.set(m.tag, words(m.tag));
	for (const t of notes.flatMap((n) => n.meetings.flatMap((m) => m.projects))) if (!names.has(t)) names.set(t, words(t));
	for (const p of projects) names.set(p.slug, p.name);
	return {
		list: [...names.keys()].sort((a, b) => (names.get(a) ?? a).localeCompare(names.get(b) ?? b)),
		name: (tag: string) => names.get(tag) ?? words(tag)
	};
}
