<script lang="ts">
	import Wrench from '@lucide/svelte/icons/wrench';
	import Dropdown, { type DropdownOption } from '$lib/components/Dropdown.svelte';
	import { dayKey, LOCALE, plural } from '$lib/format';
	import { nearestSemester, parseDay } from '$lib/semesters';
	import type { ActivityEvent, ActivityKind, ProjectSummary, Semester } from '$lib/types';

	// A stretch of days, a square each, shaded by how much was made that day: like GitHub's contribution graph.
	// Weeks run down the columns, Monday first. Days are local days, so an evening's work lands on that evening.
	// It opens on a semester and draws all of it: today has a ring and the days still to come are dashed and empty, so what
	// is left of the semester shows next to what was made.

	let { events, projects, semesters }: { events: ActivityEvent[]; projects: ProjectSummary[]; semesters: Semester[] } = $props();

	/** '' is every project. */
	let project = $state('');
	/** 'sem:<id>' is a semester ('sem:' when there is none yet); otherwise a year, "2026". It opens on the semester
	 *  we're in, or the nearest one. */
	const opening = () => `sem:${nearestSemester(semesters)?.id ?? ''}`;
	let period = $state(opening());

	const KIND: Record<ActivityKind, [string, string]> = {
		post: ['post', 'posts'],
		prompt: ['prompt', 'prompts'],
		feedback: ['feedback', 'feedback'],
		thread: ['thread', 'threads'],
		upload: ['upload', 'uploads'],
		version: ['published version', 'published versions'],
		run: ['Claude Code run', 'Claude Code runs'],
		story: ['story', 'stories'],
		task: ['task', 'tasks']
	};
	const ORDER = Object.keys(KIND) as ActivityKind[];

	const DAY = 86_400_000;
	const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
	const mondayOf = (d: Date) => addDays(d, -((d.getDay() + 6) % 7));
	const keyOf = (d: Date) => dayKey(d.toISOString());
	const today = addDays(new Date(), 0);
	const monthFormat = new Intl.DateTimeFormat(LOCALE, { month: 'short' });
	const longDate = new Intl.DateTimeFormat(LOCALE, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
	const shortDate = new Intl.DateTimeFormat(LOCALE, { day: 'numeric', month: 'short' });

	type Day = { date: Date; key: string; kinds: Partial<Record<ActivityKind, number>>; total: number; level: number; today: boolean; future: boolean };

	const mine = $derived(project ? events.filter((e) => e.projectSlug === project) : events);

	/** What was made each local day: "2026-11-03" → the counts by kind. */
	const byDay = $derived.by(() => {
		const days = new Map<string, Partial<Record<ActivityKind, number>>>();
		for (const e of mine) {
			const key = dayKey(e.at);
			const kinds = days.get(key) ?? {};
			kinds[e.kind] = (kinds[e.kind] ?? 0) + 1;
			days.set(key, kinds);
		}
		return days;
	});
	const totalOf = (key: string) => Object.values(byDay.get(key) ?? {}).reduce((a, b) => a + b, 0);

	/** The years something was made in, the latest first: a year with nothing in it isn't offered. */
	const years = $derived([...new Set(events.map((e) => new Date(e.at).getFullYear()))].sort((a, b) => b - a));

	const bySemester = $derived(period.startsWith('sem:'));
	const semester = $derived(bySemester ? semesters.find((s) => `sem:${s.id}` === period) : undefined);

	/** The days shown; null while there is no semester to look at. */
	const range = $derived.by(() => {
		if (semester) return { from: parseDay(semester.start) as Date, to: parseDay(semester.end) as Date };
		return bySemester ? null : { from: new Date(+period, 0, 1), to: new Date(+period, 11, 31) };
	});
	/** Squares grow a little for a semester, which is shorter than a year. */
	const size = $derived(semester ? 'mid' : '');

	const projectOptions = $derived<DropdownOption[]>([
		{ value: '', label: 'All projects' },
		...projects.map((p) => ({ value: p.slug, label: p.name, group: 'Projects' }))
	]);
	const periodOptions = $derived<DropdownOption[]>([
		...(semesters.length
			? [...semesters].reverse().map((s) => ({ value: `sem:${s.id}`, label: s.name, group: 'Semesters' }))
			: [{ value: 'sem:', label: 'Semester' }]),
		...years.map((y) => ({ value: String(y), label: String(y), group: 'Year' }))
	]);

	/** The weeks shown, each seven days from Monday; null where a day falls outside the period. A year stops at today,
	 *  a semester goes on to its last day: those days are drawn, but empty. */
	const weeks = $derived.by(() => {
		const out: (Day | null)[][] = [];
		if (!range) return out;
		for (let monday = mondayOf(range.from); monday <= range.to; monday = addDays(monday, 7)) {
			const week: (Day | null)[] = [];
			for (let i = 0; i < 7; i++) {
				const date = addDays(monday, i);
				if (date < range.from || date > range.to || (date > today && !semester)) {
					week.push(null);
					continue;
				}
				const key = keyOf(date);
				week.push({ date, key, kinds: byDay.get(key) ?? {}, total: totalOf(key), level: 0, today: date.getTime() === today.getTime(), future: date > today });
			}
			out.push(week);
		}
		// Shade by quartiles of the active days, so one huge day doesn't wash the rest out.
		const active = out
			.flat()
			.filter((d): d is Day => !!d && d.total > 0)
			.map((d) => d.total)
			.sort((a, b) => a - b);
		const cut = [0.25, 0.5, 0.75].map((q) => active[Math.floor(q * (active.length - 1))] ?? 0);
		for (const d of out.flat()) {
			if (d && d.total > 0) d.level = 1 + cut.filter((c) => d.total > c).length;
		}
		return out;
	});

	const days = $derived(weeks.flat().filter((d): d is Day => !!d));

	/** Every vacation day of every semester: "2026-12-24" → "Christmas break". */
	const offDays = $derived.by(() => {
		const off = new Map<string, string>();
		for (const s of semesters)
			for (const v of s.vacations)
				for (let d = parseDay(v.start) as Date; d <= (parseDay(v.end) as Date); d = addDays(d, 1)) off.set(keyOf(d), v.name || 'Vacation');
		return off;
	});
	const total = $derived(days.reduce((n, d) => n + d.total, 0));
	const hasToday = $derived(days.some((d) => d.today));

	/** A month's name above the first week in which it starts; dropped when it would crowd the one before. */
	const months = $derived.by(() => {
		const labels: { column: number; label: string }[] = [];
		weeks.forEach((week, column) => {
			const first = week.find((d) => d && (column === 0 || d.date.getDate() === 1));
			if (!first) return;
			if (column > 0 && labels.length && column - labels.at(-1)!.column < (size ? 2 : 3)) labels.pop();
			labels.push({ column, label: monthFormat.format(first.date) });
		});
		return labels;
	});

	const stats = $derived.by(() => {
		let longest = 0;
		let run = 0;
		let busiest: Day | null = null;
		for (const d of days) {
			run = d.total > 0 ? run + 1 : 0;
			longest = Math.max(longest, run);
			if (d.total > (busiest?.total ?? 0)) busiest = d;
		}
		// The current streak runs back from today, or from yesterday while nothing has been made yet today.
		let current = 0;
		let day = totalOf(keyOf(today)) > 0 ? today : addDays(today, -1);
		while (totalOf(keyOf(day)) > 0) {
			current++;
			day = addDays(day, -1);
		}
		return { active: days.filter((d) => d.total > 0).length, longest, current, busiest, left: days.filter((d) => d.future).length };
	});

	const breakdown = (d: Day) =>
		ORDER.filter((k) => d.kinds[k])
			.map((k) => {
				const n = d.kinds[k] as number;
				return `${n} ${KIND[k][n === 1 ? 0 : 1]}`;
			})
			.join(' · ');

	// The hover card: placed over the hovered day, inside the graph's box.
	let box: HTMLElement | undefined = $state();
	let tip = $state<{ day: Day; x: number; y: number } | null>(null);
	const lookup = $derived(new Map(days.map((d) => [d.key, d])));

	function hover(e: PointerEvent) {
		const cell = (e.target as HTMLElement).closest<HTMLElement>('[data-key]');
		const day = cell && lookup.get(cell.dataset.key ?? '');
		if (!cell || !day || !box) return (tip = null);
		const a = cell.getBoundingClientRect();
		const b = box.getBoundingClientRect();
		// Kept inside the box, so the first and last weeks' cards don't hang over its edges.
		const x = Math.min(Math.max(a.left + a.width / 2 - b.left, 140), b.width - 140);
		tip = { day, x, y: a.top - b.top };
	}

	// When the graph is wider than its box, start with today in the middle; a stretch without it starts at the newest
	// weeks, on the right.
	let scroller: HTMLElement | undefined = $state();
	$effect(() => {
		void weeks;
		if (!scroller) return;
		const cell = scroller.querySelector<HTMLElement>('.contrib-day.today');
		if (!cell) return void (scroller.scrollLeft = scroller.scrollWidth);
		const at = cell.getBoundingClientRect().left - scroller.getBoundingClientRect().left + scroller.scrollLeft;
		scroller.scrollLeft = at - (scroller.clientWidth - cell.offsetWidth) / 2;
	});

	const periodLabel = $derived(semester ? `in ${semester.name}` : `in ${period}`);
</script>

<section class="panel contrib" aria-label="Contributions" bind:this={box}>
	<div class="header-top">
		<h2>
			{#if range}{total.toLocaleString(LOCALE)} {total === 1 ? 'contribution' : 'contributions'}
				{periodLabel}{:else}Contributions{/if}{#if project}<span class="contrib-in"> · {projects.find((p) => p.slug === project)?.name}</span>{/if}
		</h2>
		<div class="contrib-filters">
			<Dropdown bind:value={project} options={projectOptions} label="Project" align="end" />
			<Dropdown bind:value={period} options={periodOptions} label="Period" align="end" />
			<a class="btn small" href="/settings/semesters" title="Set up the semesters this graph follows"><Wrench size={14} />Semesters</a>
		</div>
	</div>

	{#if !range}
		<div class="contrib-empty">
			<b>No semester set up yet</b>
			<p class="sub">Set one up to follow it here, day by day, or pick a year above.</p>
			<a class="btn primary small" href="/settings/semesters">Set up a semester</a>
		</div>
	{:else}
		<div class="contrib-scroll" bind:this={scroller}>
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="contrib-grid"
				class:mid={size === 'mid'}
				style="--weeks: {weeks.length}"
				role="group"
				aria-label="{plural(total, 'contribution')} on {plural(stats.active, 'day')} {periodLabel}"
				onpointerover={hover}
				onpointerleave={() => (tip = null)}
			>
				{#each months as m (m.column)}
					<span class="contrib-month" style="grid-column: {m.column + 2}">{m.label}</span>
				{/each}
				<span class="contrib-wd" style="grid-row: 2">Mon</span>
				<span class="contrib-wd" style="grid-row: 4">Wed</span>
				<span class="contrib-wd" style="grid-row: 6">Fri</span>
				{#each weeks as week, w (w)}
					{#each week as d, i (i)}
						{#if d}
							<span class="contrib-day l{d.level}" class:today={d.today} class:future={d.future} class:vac={offDays.has(d.key)} style="grid-column: {w + 2}; grid-row: {i + 2}" data-key={d.key}></span>
						{/if}
					{/each}
				{/each}
			</div>
		</div>

		<div class="contrib-foot">
			<div class="contrib-stats">
				<span><b>{stats.active}</b> active {stats.active === 1 ? 'day' : 'days'}</span>
				<span><b>{stats.current}</b> {stats.current === 1 ? 'day' : 'days'} current streak</span>
				<span><b>{stats.longest}</b> {stats.longest === 1 ? 'day' : 'days'} longest streak</span>
				{#if stats.left}<span><b>{stats.left}</b> {stats.left === 1 ? 'day' : 'days'} left</span>{/if}
				{#if stats.busiest}
					<span>Busiest: <b>{shortDate.format(stats.busiest.date)}</b> ({stats.busiest.total})</span>
				{/if}
			</div>
			<div class="contrib-legend" aria-hidden="true">
				{#if hasToday}<span class="key"><span class="contrib-day today"></span>Today</span>{/if}
				{#if stats.left}<span class="key"><span class="contrib-day future"></span>Still to come</span>{/if}
				<span class="key scale">
					Less
					{#each [0, 1, 2, 3, 4] as l (l)}<span class="contrib-day l{l}"></span>{/each}
					More
				</span>
			</div>
		</div>
	{/if}

	{#if tip}
		<div class="contrib-tip" style="left: {tip.x}px; top: {tip.y}px" role="tooltip">
			{#if tip.day.future}
				<b>Still to come</b> · {longDate.format(tip.day.date)}
			{:else}
				<b>{tip.day.total ? plural(tip.day.total, 'contribution') : tip.day.today ? 'No contributions yet' : 'No contributions'}</b>
				{tip.day.today ? 'today,' : 'on'} {longDate.format(tip.day.date)}
				{#if tip.day.total}<span>{breakdown(tip.day)}</span>{/if}
			{/if}
			{#if offDays.has(tip.day.key)}<span>Vacation: {offDays.get(tip.day.key)}</span>{/if}
		</div>
	{/if}
</section>
