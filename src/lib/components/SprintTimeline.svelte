<script lang="ts">
	import { length, parseDay, phaseOf, spanLabel, today } from '$lib/semesters';
	import type { Sprint, Vacation } from '$lib/types';

	// A semester as one bar, cut into its sprints, each as wide as it is long. The sprint today falls in is drawn in
	// the accent, past ones a step deeper than those still to come; vacations are hatched over it, and a line marks
	// today.

	let { start, end, sprints, vacations = [] }: { start: string; end: string; sprints: Sprint[]; vacations?: Vacation[] } = $props();

	const total = $derived(Math.max(1, length(start, end)));
	const offset = (day: string) => (length(start, day) - 1) / total;

	const now = today();
	const marker = $derived(now >= start && now <= end ? (length(start, now) - 0.5) / total : null);

	/** What a sprint is called on the bar: "Sprint 2" is S2, the same sprint as in the list below it; another name
	 *  ("Kick-off") is shown as it is, cut short when it doesn't fit. */
	const short = (name: string) => {
		const n = name.trim().match(/^sprint\s*(\d+)$/i);
		return n ? `S${n[1]}` : name.trim();
	};

	const parts = $derived(
		sprints
			.filter((s) => parseDay(s.start) && parseDay(s.end) && s.end >= s.start)
			.map((s, i) => ({ ...s, index: i, left: offset(s.start), width: length(s.start, s.end) / total, phase: phaseOf(s, now) }))
	);
</script>

<div class="timeline">
	{#each parts as p (p.index)}
		<div
			class="tl-sprint {p.phase}"
			style="left: {p.left * 100}%; width: {p.width * 100}%"
			title="{p.name} · {spanLabel(p.start, p.end)}{p.goal ? ` · ${p.goal}` : ''}"
		>
			<span>{p.width * total >= 10 ? short(p.name) : ''}</span>
		</div>
	{/each}
	{#each vacations as v, i (i)}
		<div
			class="tl-vacation"
			style="left: {offset(v.start) * 100}%; width: {(length(v.start, v.end) / total) * 100}%"
			title="{v.name || 'Vacation'} · {spanLabel(v.start, v.end)}"
		></div>
	{/each}
	{#if marker !== null}<span class="tl-today" style="left: {marker * 100}%" title="Today"></span>{/if}
</div>
