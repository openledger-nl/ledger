// The tags a daily note's frontmatter gathers from its meetings, as the backend writes them (src-tauri/src/daily.rs).
// Free of Svelte and Tauri, like mmm.ts.

import type { DailyNote } from './types';

/** Every tag of the note's meetings, once each: projects first (`project/x`), then people. */
export function dailyTags(note: Pick<DailyNote, 'meetings'>): string[] {
	const projects = new Set(note.meetings.flatMap((m) => m.projects.map((p) => `project/${p}`)));
	const people = new Set(note.meetings.flatMap((m) => m.people));
	return [...projects, ...people];
}
