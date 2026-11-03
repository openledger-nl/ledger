// Calendar days and the sprints of a semester, for the screens. A calendar day is "2026-09-01" (how it is stored)
// and is shown the European way, "01-09-2026" or "1 Sep 2026". The sprint plan mirrors the backend's
// (src-tauri/src/semesters.rs), so the editor can show it while you type.

import { LOCALE } from './format';
import type { Semester, SemesterDraft, Sprint, Vacation } from './types';

const pad = (n: number) => String(n).padStart(2, '0');

/** "2026-09-01" → that day at local midnight; null when it isn't a real day. */
export function parseDay(day: string): Date | null {
	const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
	if (!m) return null;
	const d = new Date(+m[1], +m[2] - 1, +m[3]);
	return d.getMonth() === +m[2] - 1 && d.getDate() === +m[3] ? d : null;
}

/** A date → its calendar day, "2026-09-01". */
export const toDay = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

export const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);

export const today = () => toDay(new Date());

/** "2026-09-01" → "01-09-2026". */
export function european(day: string): string {
	const d = parseDay(day);
	return d ? `${pad(d.getDate())}-${pad(d.getMonth() + 1)}-${d.getFullYear()}` : day;
}

/** What someone typed — "1-9-2026", "01/09/2026", "1.9.26" — → "2026-09-01"; null when it isn't a day. */
export function parseEuropean(text: string): string | null {
	const m = /^\s*(\d{1,2})[-/.\s](\d{1,2})[-/.\s](\d{2}|\d{4})\s*$/.exec(text);
	if (!m) return null;
	const year = m[3].length === 2 ? 2000 + +m[3] : +m[3];
	const day = `${year}-${pad(+m[2])}-${pad(+m[1])}`;
	return parseDay(day) ? day : null;
}

const shortFormat = new Intl.DateTimeFormat(LOCALE, { day: 'numeric', month: 'short' });
const fullFormat = new Intl.DateTimeFormat(LOCALE, { day: 'numeric', month: 'short', year: 'numeric' });

/** "1 Sep 2026". */
export const dayLabel = (day: string) => (parseDay(day) ? fullFormat.format(parseDay(day) as Date) : day);

/** "1 Sep – 21 Sep 2026", or with both years when they differ. */
export function spanLabel(start: string, end: string): string {
	const a = parseDay(start);
	const b = parseDay(end);
	if (!a || !b) return `${start} – ${end}`;
	return a.getFullYear() === b.getFullYear() ? `${shortFormat.format(a)} – ${fullFormat.format(b)}` : `${fullFormat.format(a)} – ${fullFormat.format(b)}`;
}

/** Days from `start` to `end`, both counted. */
export function length(start: string, end: string): number {
	const a = parseDay(start);
	const b = parseDay(end);
	return a && b ? Math.round((b.getTime() - a.getTime()) / 86_400_000) + 1 : 0;
}

/** Whether a day falls in one of the vacations. */
export const isOff = (day: string, vacations: Vacation[]) => vacations.some((v) => day >= v.start && day <= v.end);

/** Days of work from `start` to `end`, both counted: the days not in a vacation. */
export function workDays(start: string, end: string, vacations: Vacation[]): number {
	let n = 0;
	for (let d = parseDay(start), until = parseDay(end); d && until && d <= until; d = addDays(d, 1)) if (!isOff(toDay(d), vacations)) n++;
	return n;
}

/**
 * Sprints from `start` to `end`, each `weeks(i)` weeks of work long (or `weeks` for all of them), counted around the
 * vacations: the clock stops during one, so a sprint that runs into it carries on after it. Less than a week of
 * work left at the end joins the last sprint. Cuts the same way as the backend's `plan`.
 */
export function planSprints(start: string, end: string, weeks: number | ((index: number) => number), vacations: Vacation[] = []): Sprint[] {
	const from = parseDay(start);
	const until = parseDay(end);
	if (!from || !until || until < from) return [];
	const lengthOf = typeof weeks === 'number' ? () => weeks : weeks;
	const sprints: Sprint[] = [];
	let day = from;
	for (;;) {
		while (day <= until && isOff(toDay(day), vacations)) day = addDays(day, 1);
		if (day > until) break;
		const first = day;
		const need = Math.max(1, lengthOf(sprints.length)) * 7;
		let worked = 0;
		let last = day;
		for (; day <= until && worked < need; day = addDays(day, 1)) {
			if (!isOff(toDay(day), vacations)) {
				worked++;
				last = day;
			}
		}
		const previous = sprints.at(-1);
		if (previous && worked < 7) previous.end = toDay(last);
		else sprints.push({ name: `Sprint ${sprints.length + 1}`, start: toDay(first), end: toDay(last), goal: '' });
		day = addDays(last, 1);
	}
	return sprints;
}

/** The last day of the half year that starts on `start`: six months on, less a day (1 Feb → 31 Jul). */
export function halfYearFrom(start: string): string {
	const d = parseDay(start);
	return d ? toDay(new Date(d.getFullYear(), d.getMonth() + 6, d.getDate() - 1)) : start;
}

/** A first guess for a new semester: the half year after the last one, or the one we're in (September and
 *  February start them, as Dutch schools do). */
export function suggestDraft(existing: Semester[]): SemesterDraft {
	const last = existing.at(-1);
	let start: string;
	if (last) {
		start = toDay(addDays(parseDay(last.end) as Date, 1));
	} else {
		const now = new Date();
		const autumn = now.getMonth() >= 7 || now.getMonth() === 0;
		const year = now.getMonth() === 0 ? now.getFullYear() - 1 : now.getFullYear();
		start = autumn ? `${year}-09-01` : `${now.getFullYear()}-02-01`;
	}
	const s = parseDay(start) as Date;
	const firstHalf = s.getMonth() >= 6;
	const schoolYear = firstHalf ? `${s.getFullYear()}–${s.getFullYear() + 1}` : `${s.getFullYear() - 1}–${s.getFullYear()}`;
	const end = firstHalf ? `${s.getFullYear() + 1}-01-31` : `${s.getFullYear()}-07-31`;
	return {
		name: `Semester ${firstHalf ? 1 : 2} · ${schoolYear}`,
		start,
		end: parseDay(end) && end > start ? end : halfYearFrom(start),
		sprintWeeks: 3,
		notes: '',
		sprints: [],
		vacations: []
	};
}

/**
 * NJ26, the autumn semester of 2026, as its calendar has it: a kick-off week, a shorter sprint 0, five sprints of three
 * weeks, a week off in autumn and two at Christmas. It ends with the last sprint: the weeks after it (the end talks
 * and the graduation conference) are events, not sprints. The name keeps its X for whoever adds it to fill in.
 */
export function nj26Draft(): SemesterDraft {
	const start = '2026-08-31';
	const end = '2027-01-24';
	const vacations: Vacation[] = [
		{ name: 'Autumn break', start: '2026-10-19', end: '2026-10-25' },
		{ name: 'Christmas break', start: '2026-12-21', end: '2027-01-03' }
	];
	const plan: [name: string, weeks: number][] = [['Kick-off', 1], ['Sprint 0', 2], ['Sprint 1', 3], ['Sprint 2', 3], ['Sprint 3', 3], ['Sprint 4', 3], ['Sprint 5', 3]];
	const sprints = planSprints(start, end, (i) => plan[i]?.[1] ?? 3, vacations).map((s, i) => ({ ...s, name: plan[i]?.[0] ?? s.name }));
	return { name: 'NJ26 - Semester X', start, end, sprintWeeks: 3, notes: '', sprints, vacations };
}

export type Phase = 'current' | 'upcoming' | 'past';

export function phaseOf(s: { start: string; end: string }, day = today()): Phase {
	return day < s.start ? 'upcoming' : day > s.end ? 'past' : 'current';
}

/** The semester today falls in, if any. */
export const currentSemester = (semesters: Semester[], day = today()) => semesters.find((s) => phaseOf(s, day) === 'current');

/** The index of the sprint today falls in, or -1. */
export const currentSprint = (s: Semester, day = today()) => s.sprints.findIndex((p) => phaseOf(p, day) === 'current');
