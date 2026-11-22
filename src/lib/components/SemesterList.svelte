<script lang="ts">
	import CalendarRange from '@lucide/svelte/icons/calendar-range';
	import Flag from '@lucide/svelte/icons/flag';
	import Plus from '@lucide/svelte/icons/plus';
	import TreePalm from '@lucide/svelte/icons/tree-palm';
	import Dropdown, { type DropdownOption } from '$lib/components/Dropdown.svelte';
	import SemesterModal from '$lib/components/SemesterModal.svelte';
	import SprintTimeline from '$lib/components/SprintTimeline.svelte';
	import { api, portflowApi } from '$lib/api';
	import { portflow } from '$lib/portflow.svelte';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import { currentSprint, draftOf, length, phaseOf, spanLabel, today, workDays } from '$lib/semesters';
	import type { PortflowSection, Semester, SemesterDraft, SemesterTemplate } from '$lib/types';

	// The semesters the dashboard's contribution graph follows: a list, and a + that adds one from a semester template
	// (shared/templates/semesters) or from scratch. The page (/semesters) loads both, so they stay current when one is
	// saved.

	let { semesters, templates }: { semesters: Semester[]; templates: SemesterTemplate[] } = $props();

	const about = (t: SemesterTemplate) => `${spanLabel(t.start, t.end)} | ${t.sprints.length} sprints`;

	/** Keeps a semester's days, vacations and sprints as a template, to start the next one from. */
	async function saveAsTemplate(s: Semester) {
		const saved = await attempt(() => api.saveSemesterTemplate(null, draftOf(s)));
		if (!saved) return;
		toast(`${saved.name} is a semester template now (Shared, Templates)`);
		await refresh();
	}

	// Which Portflow section (a semester in the portfolio) each semester is: only while Portflow is on.
	let sections = $state<PortflowSection[]>([]);
	$effect(() => {
		void portflow.status?.lastSyncAt;
		if (!portflow.enabled) {
			sections = [];
			return;
		}
		let stale = false;
		portflowApi.sections().then(
			(all) => !stale && (sections = all),
			() => {}
		);
		return () => (stale = true);
	});
	const sectionOf = (semester: string) => sections.find((x) => x.semester === semester)?.id ?? null;
	async function linkSection(semester: string, value: string) {
		const all = await attempt(() => portflowApi.linkSection(semester, value ? Number(value) : null));
		if (all) sections = all;
	}
	const sectionOptions = (semester: string): DropdownOption[] => [
		{ value: '', label: 'None' },
		...sections.map((x) => ({
			value: String(x.id),
			label: `${x.name}${x.semester && x.semester !== semester ? ` (now ${x.semesterName ?? x.semester})` : ''}`
		}))
	];

	/** The editor: undefined when closed; otherwise the semester it changes, or null for a new one (from `preset`, if given). */
	let editor = $state<{ semester: Semester | null; preset?: SemesterDraft } | undefined>(undefined);

	let adding = $state(false);
	let menu: HTMLElement | undefined = $state();

	function add(preset?: SemesterDraft) {
		adding = false;
		editor = { semester: null, preset };
	}

	// A click anywhere else, or Escape, closes the menu.
	$effect(() => {
		if (!adding) return;
		const away = (e: PointerEvent) => {
			if (menu && !menu.contains(e.target as Node)) adding = false;
		};
		const escape = (e: KeyboardEvent) => e.key === 'Escape' && (adding = false);
		document.addEventListener('pointerdown', away);
		document.addEventListener('keydown', escape);
		return () => {
			document.removeEventListener('pointerdown', away);
			document.removeEventListener('keydown', escape);
		};
	});

	const now = today();
	/** The current semester first, then what's coming (soonest first), then what's past (latest first). */
	const groups = $derived.by(() => {
		const by = (phase: string) => semesters.filter((s) => phaseOf(s, now) === phase);
		return [
			{ label: 'Now', items: by('current') },
			{ label: 'Coming up', items: by('upcoming') },
			{ label: 'Past', items: by('past').reverse() }
		].filter((g) => g.items.length);
	});

	const PHASE = { current: 'Current', upcoming: 'Upcoming', past: 'Past' } as const;

	function progress(s: Semester) {
		const total = length(s.start, s.end);
		const done = Math.min(total, Math.max(0, length(s.start, now)));
		return { week: Math.ceil(done / 7), weeks: Math.ceil(total / 7) };
	}
</script>

{#snippet addMenu()}
	<div class="dd" bind:this={menu}>
		<button class="btn primary small" aria-haspopup="menu" aria-expanded={adding} aria-label="Add a semester" title="Add a semester" onclick={() => (adding = !adding)}>
			<Plus size={16} />
		</button>
		{#if adding}
			<div class="dd-menu end" role="menu" aria-label="Add a semester">
				{#each templates as t (t.id)}
					<button type="button" class="dd-option menu-item" role="menuitem" onclick={() => add(draftOf(t))}>
						<span class="menu-title">From {t.name}</span>
						<span class="menu-sub">{about(t)}</span>
					</button>
				{/each}
				<button type="button" class="dd-option menu-item" role="menuitem" onclick={() => add()}>
					<span class="menu-title">New semester</span>
					<span class="menu-sub">Your own days, vacations and sprints</span>
				</button>
			</div>
		{/if}
	</div>
{/snippet}

<div class="panel">
	<div class="header-top">
		<div>
			<h2>Semesters</h2>
			<div class="sub">Half years, cut into sprints of three weeks of work, shared by every project. The contribution graph on the dashboard follows them.</div>
		</div>
		{#if semesters.length}{@render addMenu()}{/if}
	</div>
	{#if semesters.length === 0}
		<div class="sem-empty">
			<span class="sem-empty-ico"><CalendarRange size={28} /></span>
			<h3>No semester set up yet</h3>
			<p class="sub">
				{templates.length ? 'Start from a semester template, or give' : 'Give'} a semester a first and a last day (half a year) and Ledger cuts it into
				sprints. Name them, set a goal for each, or move their dates. The dashboard's contribution graph then follows the semester, day by day.
			</p>
			<div class="sem-empty-actions">
				{#each templates as t, i (t.id)}
					<button class="btn" class:primary={i === 0} onclick={() => add(draftOf(t))} title={about(t)}><Plus size={16} />From {t.name}</button>
				{/each}
				<button class="btn" class:primary={!templates.length} onclick={() => add()}>New semester</button>
			</div>
		</div>
	{/if}
</div>

{#each groups as g (g.label)}
	<section class="sem-group">
		<h2 class="proj-group-head">{g.label}</h2>
		{#each g.items as s (s.id)}
			{@const phase = phaseOf(s, now)}
			{@const at = currentSprint(s, now)}
			{@const p = progress(s)}
			<article class="panel sem-card" class:current={phase === 'current'}>
				<div class="sem-card-head">
					<div class="sem-card-title">
						<h3>{s.name}</h3>
						<span class="badge" class:accent={phase === 'current'}>{PHASE[phase]}</span>
					</div>
					<div class="row-end">
						<button class="btn small ghost" onclick={() => saveAsTemplate(s)} title="Start the next semester from its days, vacations and sprints">Save as template</button>
						<button class="btn small" onclick={() => (editor = { semester: s })}>Edit</button>
					</div>
				</div>
				<div class="sem-meta">
					<span>{spanLabel(s.start, s.end)}</span>
					<span>{s.sprints.length} sprints</span>
					{#if s.vacations.length}<span>{s.vacations.length} {s.vacations.length === 1 ? 'vacation' : 'vacations'}</span>{/if}
					{#if phase === 'current'}
						<span><b>Week {p.week}</b> of {p.weeks}{#if at >= 0} | <b>{s.sprints[at].name}</b>{/if}</span>
					{/if}
				</div>

				{#if portflow.enabled && sections.length}
					<div class="sem-section">
						Portflow section
						<!-- Section ids are numbers; the empty string is "None". Only a different pick links anew. -->
						<Dropdown
							bind:value={() => String(sectionOf(s.id) ?? ''), (v) => v !== String(sectionOf(s.id) ?? '') && linkSection(s.id, v)}
							options={sectionOptions(s.id)}
							label="Portflow section"
						/>
					</div>
				{/if}

				<SprintTimeline start={s.start} end={s.end} sprints={s.sprints} vacations={s.vacations} />

				<ol class="sem-sprints">
					{#each s.sprints as sp, i (i)}
						{@const sphase = phaseOf(sp, now)}
						<li class={sphase}>
							<span class="n">{i + 1}</span>
							<span class="name">{sp.name}</span>
							<span class="when">{spanLabel(sp.start, sp.end)} | {Math.round(workDays(sp.start, sp.end, s.vacations) / 7)} wk</span>
							{#if sp.goal}<span class="goal"><Flag size={12} />{sp.goal}</span>{:else}<span class="goal"></span>{/if}
							{#if sphase === 'current'}<span class="badge accent">Now</span>{/if}
						</li>
					{/each}
				</ol>

				{#if s.vacations.length}
					<div class="sem-vacations">
						{#each s.vacations as v, i (i)}
							<span class="chip plain"><TreePalm size={12} />{v.name} | {spanLabel(v.start, v.end)}</span>
						{/each}
					</div>
				{/if}

				{#if s.notes}<p class="sem-notes">{s.notes}</p>{/if}
			</article>
		{/each}
	</section>
{/each}

{#if editor}
	<SemesterModal semester={editor.semester} preset={editor.preset} existing={semesters} onclose={() => (editor = undefined)} />
{/if}
