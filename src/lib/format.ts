// Timestamps arrive as UTC ISO strings; everything shown to people is in local time, written the European way:
// day before month ("3 Nov 2026", "03-11-2026") and a 24-hour clock, whatever the system's own locale says.

import type { ThreadSummary } from './types';

/** The locale every date and number is shown in. */
export const LOCALE = 'en-GB';

const timeFormat = new Intl.DateTimeFormat(LOCALE, { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' });
const dateFormat = new Intl.DateTimeFormat(LOCALE, { day: 'numeric', month: 'short', year: 'numeric' });
const longDayFormat = new Intl.DateTimeFormat(LOCALE, { weekday: 'long', day: 'numeric', month: 'long' });

export const time = (iso: string) => timeFormat.format(new Date(iso));

export const date = (iso: string) => dateFormat.format(new Date(iso));

export const dateTime = (iso: string) => `${date(iso)}, ${time(iso)}`;

/** Local calendar day, "2026-10-16": what posts are grouped under. */
export function dayKey(iso: string): string {
	const d = new Date(iso);
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function dayLabel(key: string): string {
	const [y, m, d] = key.split('-').map(Number);
	const day = new Date(y, m - 1, d);
	const today = new Date();
	const startOfToday = new Date(today.getFullYear(), today.getMonth(), today.getDate());
	const diff = Math.round((startOfToday.getTime() - day.getTime()) / 86_400_000);
	if (diff === 0) return 'Today';
	if (diff === 1) return 'Yesterday';
	return longDayFormat.format(day) + (y !== today.getFullYear() ? ` ${y}` : '');
}

/** Markdown flattened to one line of plain text, for quoting a post. */
export function plainText(markdown: string, max = 140): string {
	const text = markdown
		.replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
		.replace(/^\s*[-+*]\s+/gm, '')
		.replace(/[*_`~>#]+/g, '')
		.replace(/\s+/g, ' ')
		.trim();
	return text.length > max ? `${text.slice(0, max).trimEnd()}…` : text;
}

/** "just now", "5 min ago", "3 h ago", or the date for anything older than a day. */
export function ago(iso: string): string {
	const seconds = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 1000));
	if (seconds < 45) return 'just now';
	if (seconds < 3600) return `${Math.max(1, Math.round(seconds / 60))} min ago`;
	if (seconds < 86_400) return `${Math.round(seconds / 3600)} h ago`;
	return date(iso);
}

/** The moment a deliverable is dated by: when its version went to Portflow, else when its newest version was published,
 *  else when the file last changed (a file's time also changes when it is synced or checked out, so it comes last). */
export function deliverableTime(d: { modified: string; latestAt?: string | null; delivered?: { number: number; at: string } | null }): {
	at: string;
	title: string;
} {
	if (d.delivered) return { at: d.delivered.at, title: `v${d.delivered.number} delivered to Portflow ${ago(d.delivered.at)}` };
	if (d.latestAt) return { at: d.latestAt, title: `Newest version published ${ago(d.latestAt)}` };
	return { at: d.modified, title: `Changed ${ago(d.modified)}` };
}

/** A timestamp as milliseconds, for comparing; 0 when it isn't one. */
export const stamp = (iso: string) => new Date(iso).getTime() || 0;

/** The items, the one whose time is latest first. */
export const newest = <T,>(items: readonly T[], at: (item: T) => string) => [...items].sort((a, b) => stamp(at(b)) - stamp(at(a)));

/** "3 posts", "1 post". */
export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** "4 posts · 2 prompts · 1 feedback": what a thread holds, leaving out what it has none of. */
export function threadMeta(t: ThreadSummary): string {
	const parts = [plural(t.postCount, 'post')];
	if (t.promptCount) parts.push(plural(t.promptCount, 'prompt'));
	if (t.feedbackCount) parts.push(`${t.feedbackCount} feedback`);
	if (t.uploadCount) parts.push(plural(t.uploadCount, 'upload'));
	return parts.join(' · ');
}

/** Escapes text and wraps case-insensitive matches of `query` in <mark>, for search snippets. */
export function highlight(text: string, query: string): string {
	const escape = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
	const needle = query.trim();
	if (!needle) return escape(text);
	const pattern = new RegExp(needle.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi');
	let out = '';
	let last = 0;
	for (const match of text.matchAll(pattern)) {
		out += escape(text.slice(last, match.index)) + `<mark>${escape(match[0])}</mark>`;
		last = match.index + match[0].length;
	}
	return out + escape(text.slice(last));
}
