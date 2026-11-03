<script lang="ts">
	import Minus from '@lucide/svelte/icons/minus';
	import Palmtree from '@lucide/svelte/icons/tree-palm';
	import Plus from '@lucide/svelte/icons/plus';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import { addDays, halfYearFrom, length, parseDay, planSprints, spanLabel, suggestDraft, toDay, workDays } from '$lib/semesters';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import type { Semester, Vacation } from '$lib/types';
	import DateInput from './DateInput.svelte';
	import Modal from './Modal.svelte';
	import SprintTimeline from './SprintTimeline.svelte';

	// Making or changing a semester. The sprints are never typed in: they follow from the first and last day, the
	// vacations and how many weeks of work each sprint is (the semester's length unless a sprint has its own), so
	// they always fit. Only their names and goals are written by hand.

	/** `semester` is null when making one; `existing` gives a new one a sensible first guess. */
	let { semester, existing, onclose }: { semester: Semester | null; existing: Semester[]; onclose: () => void } = $props();

	/** The form's starting values, read once (the modal is mounted fresh each time); edits stay local until Save. */
	function opening() {
		const draft = semester ? structuredClone($state.snapshot(semester)) : suggestDraft(existing);
		const weeks = draft.sprintWeeks || 3;
		// A sprint's own length, where it differs from the semester's; the last one is whatever is left.
		const lengths = draft.sprints.map((s, i) => {
			const own = Math.max(1, Math.round(workDays(s.start, s.end, draft.vacations) / 7));
			return i < draft.sprints.length - 1 && own !== weeks ? own : undefined;
		});
		return { ...draft, sprintWeeks: weeks, lengths, names: draft.sprints.map((s) => s.name), goals: draft.sprints.map((s) => s.goal) };
	}
	const initial = opening();

	let name = $state(initial.name);
	let start = $state(initial.start);
	let end = $state(initial.end);
	let weeks = $state(initial.sprintWeeks);
	let notes = $state(initial.notes);
	let vacations = $state<Vacation[]>(initial.vacations);
	/** Per sprint: its own length in weeks, or undefined for the semester's. */
	let lengths = $state<(number | undefined)[]>(initial.lengths);
	let names = $state<string[]>(initial.names);
	let goals = $state<string[]>(initial.goals);
	let saving = $state(false);

	/** Vacations that make sense yet; the half-typed ones are left out of the plan until they do. */
	const usable = $derived(vacations.filter((v) => parseDay(v.start) && parseDay(v.end) && v.end >= v.start));
	const sprints = $derived(
		planSprints(start, end, (i) => lengths[i] ?? weeks, usable).map((s, i) => ({ ...s, name: names[i]?.trim() || s.name, goal: goals[i] ?? '' }))
	);
	const lengthOf = (i: number) => lengths[i] ?? weeks;
	const custom = $derived(lengths.some((l, i) => l !== undefined && i < sprints.length));

	const days = $derived(length(start, end));
	const problem = $derived.by(() => {
		if (!name.trim()) return 'Give the semester a name.';
		if (!parseDay(start) || !parseDay(end)) return 'Fill in both days.';
		if (end < start) return 'The semester ends before it starts.';
		if (days > 367) return 'A semester is at most a year long.';
		const sorted = [...vacations].sort((a, b) => a.start.localeCompare(b.start));
		for (const [i, v] of sorted.entries()) {
			const label = v.name.trim() || 'A vacation';
			if (!parseDay(v.start) || !parseDay(v.end)) return `${label} needs both days.`;
			if (v.end < v.start) return `${label} ends before it starts.`;
			if (v.start < start || v.end > end) return `${label} falls outside the semester.`;
			if (i > 0 && v.start <= sorted[i - 1].end) return `${label} overlaps ${sorted[i - 1].name.trim() || 'another vacation'}.`;
		}
		if (!sprints.length) return 'The semester has no days outside its vacations.';
		return '';
	});

	function setLength(i: number, n: number) {
		lengths[i] = Math.min(12, Math.max(1, n)) === weeks ? undefined : Math.min(12, Math.max(1, n));
	}

	async function resetLengths() {
		const ok = await confirmDialog({ title: 'Make every sprint the same length?', message: `Each sprint goes back to ${weeks} weeks of work. Names and goals stay.`, confirmLabel: 'Reset' });
		if (ok) lengths = [];
	}

	function addVacation() {
		// A week, starting on the Monday seven weeks after the semester's start or the last vacation's end.
		const after = parseDay(vacations.at(-1)?.end ?? start) ?? new Date();
		const base = addDays(after, 7 * 7);
		const monday = addDays(base, -((base.getDay() + 6) % 7));
		const from = toDay(monday) > end ? start : toDay(monday);
		const until = toDay(addDays(parseDay(from) as Date, 6));
		vacations.push({ name: '', start: from, end: until > end ? end : until });
	}

	async function save() {
		saving = true;
		const draft = { name, start, end, sprintWeeks: weeks, notes, sprints, vacations };
		const saved = await attempt(() => (semester ? api.updateSemester(semester.id, draft) : api.createSemester(draft)));
		saving = false;
		if (!saved) return;
		toast(semester ? 'Semester saved' : `${saved.name} added, with ${saved.sprints.length} sprints`);
		await refresh();
		onclose();
	}

	async function remove() {
		if (!semester) return;
		const ok = await confirmDialog({ title: `Delete “${semester.name}”?`, message: 'Its sprints and vacations go with it. Nothing you made in it is touched.', confirmLabel: 'Delete semester', danger: true });
		if (!ok || (await attempt(() => api.deleteSemester(semester.id))) === undefined) return;
		await refresh();
		onclose();
	}
</script>

<Modal title={semester ? 'Edit semester' : 'New semester'} wide {onclose}>
	<label class="field">
		Name
		<input bind:value={name} placeholder="e.g. Semester 1 · 2026–2027" />
	</label>

	<div class="sem-dates">
		<div class="field">
			First day
			<DateInput bind:value={start} label="First day" range={{ start, end }} />
		</div>
		<div class="field">
			Last day
			<DateInput bind:value={end} label="Last day" min={start} range={{ start, end }} />
		</div>
		<div class="sem-length">
			<button type="button" class="btn small ghost" onclick={() => (end = halfYearFrom(start))} title="Six months from the first day">Half a year</button>
			<span class="hint">{days > 0 ? `${Math.floor(days / 7)} weeks${days % 7 ? ` ${days % 7} d` : ''}` : ''}</span>
		</div>
	</div>

	<SprintTimeline {start} {end} {sprints} vacations={usable} />

	<div class="sem-section">
		<div class="sem-section-head">
			<div>
				<h3>Vacations</h3>
				<span class="hint">The sprints stop for these and carry on after them.</span>
			</div>
			<button type="button" class="btn small" onclick={addVacation}><Plus size={14} />Add a vacation</button>
		</div>
		{#each vacations as v, i (i)}
			<div class="vac-row">
				<span class="vac-ico"><Palmtree size={14} /></span>
				<input class="vac-name" bind:value={v.name} aria-label="Name of vacation {i + 1}" placeholder="e.g. Christmas break" />
				<DateInput bind:value={v.start} label="First day of vacation {i + 1}" min={start} max={end} range={{ start: v.start, end: v.end }} />
				<DateInput bind:value={v.end} label="Last day of vacation {i + 1}" min={v.start} max={end} range={{ start: v.start, end: v.end }} />
				<button type="button" class="sprint-x" aria-label="Remove vacation {i + 1}" title="Remove" onclick={() => vacations.splice(i, 1)}><X size={14} /></button>
			</div>
		{:else}
			<p class="hint" style="margin:0">No vacations: the sprints run back to back.</p>
		{/each}
	</div>

	<div class="sem-section">
		<div class="sem-section-head">
			<div>
				<h3>Sprints</h3>
				<span class="hint">{sprints.length} {sprints.length === 1 ? 'sprint' : 'sprints'}, counted in weeks of work</span>
			</div>
			<div class="sem-section-tools">
				{#if custom}<button type="button" class="link" onclick={resetLengths}><RotateCcw size={12} /> All {weeks} weeks</button>{/if}
				<div class="sem-weeks" role="group" aria-label="Length of every sprint">
					<span class="lbl">Each</span>
					<button type="button" class="step" aria-label="Shorter sprints" disabled={weeks <= 1} onclick={() => (weeks = Math.max(1, weeks - 1))}><Minus size={14} /></button>
					<span class="weeks"><b>{weeks}</b> {weeks === 1 ? 'week' : 'weeks'}</span>
					<button type="button" class="step" aria-label="Longer sprints" disabled={weeks >= 12} onclick={() => (weeks = Math.min(12, weeks + 1))}><Plus size={14} /></button>
				</div>
			</div>
		</div>

		<div class="sprint-list">
			{#each sprints as s, i (i)}
				{@const isLast = i === sprints.length - 1}
				{@const worked = workDays(s.start, s.end, usable)}
				<div class="sprint-row" class:own={lengths[i] !== undefined}>
					<span class="sprint-n">{i + 1}</span>
					<input class="sprint-name" bind:value={() => names[i] ?? '', (v) => (names[i] = v)} aria-label="Name of sprint {i + 1}" placeholder="Sprint {i + 1}" />
					<div class="sem-weeks small" role="group" aria-label="Length of sprint {i + 1}">
						<button type="button" class="step" aria-label="Shorter" disabled={lengthOf(i) <= 1} onclick={() => setLength(i, lengthOf(i) - 1)}><Minus size={13} /></button>
						<span class="weeks"><b>{lengthOf(i)}</b> wk</span>
						<button type="button" class="step" aria-label="Longer" disabled={lengthOf(i) >= 12} onclick={() => setLength(i, lengthOf(i) + 1)}><Plus size={13} /></button>
					</div>
					<span class="sprint-span">
						{spanLabel(s.start, s.end)}
						{#if worked !== length(s.start, s.end)}<span class="vac-note"> · incl. vacation</span>{/if}
						{#if isLast && worked !== lengthOf(i) * 7}<span class="vac-note"> · {worked} days of work</span>{/if}
					</span>
					<input class="sprint-goal" bind:value={() => goals[i] ?? '', (v) => (goals[i] = v)} aria-label="Goal of sprint {i + 1}" placeholder="Goal (optional)" />
				</div>
			{/each}
		</div>
	</div>

	<label class="field">
		Notes
		<textarea bind:value={notes} rows="2" placeholder="Exam weeks, deadlines, what this half is about…"></textarea>
	</label>

	{#snippet footer()}
		{#if semester}<button class="btn ghost danger" onclick={remove}>Delete</button>{/if}
		<span class="spacer-h"></span>
		{#if problem}<span class="hint error-text">{problem}</span>{/if}
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={save} disabled={saving || !!problem}>{semester ? 'Save' : 'Add semester'}</button>
	{/snippet}
</Modal>
