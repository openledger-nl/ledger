// A daily note as a file: frontmatter, then a `##` heading per meeting. Written here the way the backend will write
// `Daily/<day>.md`, and read back leniently, so the preview shows the real file and an Obsidian-made note reads in.
// Free of Svelte and Tauri, like mmm.ts.

import type { DailyNote, MeetingSection } from './types';

const TAG = /#([\p{L}\p{N}_\-/]+)/gu;

/** Every tag of the note's meetings, once each: projects first (`project/x`), then people. */
export function dailyTags(note: Pick<DailyNote, 'meetings'>): string[] {
	const projects = new Set(note.meetings.flatMap((m) => m.projects.map((p) => `project/${p}`)));
	const people = new Set(note.meetings.flatMap((m) => m.people));
	return [...projects, ...people];
}

export function renderDaily(note: DailyNote): string {
	const tags = dailyTags(note);
	const head = ['---', 'type: daily', `id: ${note.id}`, `date: ${note.date}`, `tags: [${tags.join(', ')}]`, '---', ''];
	const sections = note.meetings.map((m) => {
		const heading = ['##', m.time, m.title.trim() || 'Meeting', ...m.projects.map((p) => `#project/${p}`)].filter(Boolean).join(' ');
		const lines = [heading];
		if (m.people.length) lines.push(`With: ${m.people.map((p) => `#${p}`).join(' ')}`);
		lines.push('', m.body.trim());
		return lines.join('\n').trimEnd();
	});
	return `${head.join('\n')}\n${sections.join('\n\n')}\n`;
}

/** Reads a daily note's text: every `##` heading is a meeting; what is before the first one is not a meeting and is left out. */
export function parseDaily(date: string, text: string): DailyNote {
	const body = text.replace(/^---\n[\s\S]*?\n---\n?/, '');
	const id = /^id:\s*(.+)$/m.exec(text.slice(0, text.length - body.length))?.[1]?.trim() ?? `daily-${date}`;
	const meetings: MeetingSection[] = [];
	for (const part of body.split(/^(?=## )/m)) {
		if (!part.startsWith('## ')) continue;
		const [first, ...rest] = part.split('\n');
		const heading = first.slice(3);
		const projects = [...heading.matchAll(TAG)].map((m) => m[1]).filter((t) => t.startsWith('project/')).map((t) => t.slice(8));
		const words = heading.replace(TAG, '').trim();
		const time = /^(\d{1,2}:\d{2})\s*/.exec(words);
		let lines = rest;
		let people: string[] = [];
		const withLine = lines.findIndex((l) => /^With:/i.test(l.trim()));
		if (withLine !== -1 && lines.slice(0, withLine).every((l) => !l.trim())) {
			people = [...lines[withLine].matchAll(TAG)].map((m) => m[1]);
			lines = lines.slice(withLine + 1);
		}
		meetings.push({ time: time?.[1] ?? null, title: words.slice(time?.[0].length ?? 0).trim(), projects, people, body: lines.join('\n').trim() });
	}
	return { date, id, meetings, updatedAt: new Date().toISOString() };
}
