// Timestamps arrive as UTC ISO strings; everything shown to people is in Central European time (Amsterdam), written
// the European way: day-month-year in numbers ("22-2-2022", no leading zeros) and a 24-hour clock ("14:05"), whatever
// the system's own locale or time zone says.

import type { ThreadSummary } from './types';

/** The locale every date and number is shown in. */
export const LOCALE = 'en-GB';

/** Every moment is shown in this time zone: Central European time, summer time included. */
export const TIME_ZONE = 'Europe/Amsterdam';

const timeFormat = new Intl.DateTimeFormat(LOCALE, { hour: '2-digit', minute: '2-digit', hourCycle: 'h23', timeZone: TIME_ZONE });
const partsFormat = new Intl.DateTimeFormat('en-CA', { year: 'numeric', month: '2-digit', day: '2-digit', timeZone: TIME_ZONE });
const weekdayFormat = new Intl.DateTimeFormat(LOCALE, { weekday: 'long', timeZone: 'UTC' });

/** "2026-02-22" → "22-2-2026": the one way a calendar day is written. */
export function numericDay(day: string): string {
	const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
	return m ? `${+m[3]}-${+m[2]}-${m[1]}` : day;
}

/** The weekday of a calendar day ("2026-11-04" → "Wednesday"), the same in every time zone. */
export function weekday(day: string): string {
	const [y, m, d] = day.split('-').map(Number);
	return weekdayFormat.format(new Date(Date.UTC(y, m - 1, d)));
}

/** A moment from an ISO string or a Date; null when it isn't one (a missing or broken date never breaks a page). */
function moment(iso: string | Date): Date | null {
	const d = typeof iso === 'string' ? new Date(iso) : iso;
	return Number.isNaN(d.getTime()) ? null : d;
}

/** "14:05", in Amsterdam; "" for something that isn't a moment. */
export const time = (iso: string) => {
	const d = moment(iso);
	return d ? timeFormat.format(d) : '';
};

/** "4-11-2026", the day it was in Amsterdam. */
export const date = (iso: string) => numericDay(dayKey(iso));

export const dateTime = (iso: string) => (moment(iso) ? `${date(iso)}, ${time(iso)}` : '');

/** The calendar day in Amsterdam, "2026-10-16": what posts are grouped under; "" for something that isn't a moment. */
export function dayKey(iso: string | Date): string {
	const d = moment(iso);
	return d ? partsFormat.format(d) : '';
}

/** A day and a 24-hour time as they are in Amsterdam ("2026-11-09", "14:05"), as a moment (ISO 8601, UTC). */
export function amsterdamMoment(day: string, clock: string): string {
	const [y, m, d] = day.split('-').map(Number);
	const [hh, mm] = (clock || '00:00').split(':').map(Number);
	const wall = Date.UTC(y, m - 1, d, hh || 0, mm || 0);
	// How far Amsterdam's clock is ahead of UTC at a moment (an hour, or two in summer time).
	const ahead = (t: number) => {
		const parts = Object.fromEntries(
			new Intl.DateTimeFormat('en-GB', { timeZone: TIME_ZONE, hourCycle: 'h23', year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric' })
				.formatToParts(new Date(t))
				.map((p) => [p.type, Number(p.value)])
		);
		return Date.UTC(parts.year, parts.month - 1, parts.day, parts.hour, parts.minute) - t;
	};
	return new Date(wall - ahead(wall - ahead(wall))).toISOString();
}

/** Today in Amsterdam, "2026-11-09". */
export const todayKey = () => dayKey(new Date());

/** A day heading: "Today", "Yesterday", or "Wednesday 4-11-2026". */
export function dayLabel(key: string): string {
	const [y, m, d] = key.split('-').map(Number);
	const [ty, tm, td] = todayKey().split('-').map(Number);
	const diff = Math.round((Date.UTC(ty, tm - 1, td) - Date.UTC(y, m - 1, d)) / 86_400_000);
	if (diff === 0) return 'Today';
	if (diff === 1) return 'Yesterday';
	return `${weekday(key)} ${numericDay(key)}`;
}

/** Markdown flattened to one line of plain text, for quoting a post. */
export function plainText(markdown: string, max = 140): string {
	const text = markdown
		.replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
		.replace(/^\s*[-+*]\s+/gm, '')
		.replace(/[*_`~>#]+/g, '')
		.replace(/\s+/g, ' ')
		.trim();
	return text.length > max ? `${text.slice(0, max).trimEnd()}...` : text;
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
	return parts.join(' | ');
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
