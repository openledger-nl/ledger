<script lang="ts">
	// Meeting notes: one note per day, newest first with today on top. A day holds its meetings, each under its own
	// heading and coupled to its projects and people. Stored as the day's note in Daily/, so it opens in Obsidian too.
	// Requirements: Meetings, Obsidian (daily notes).
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import MeetingNoteEditor from '$lib/components/MeetingNoteEditor.svelte';
	import DateInput from '$lib/components/DateInput.svelte';
	import { personTagOptions, projectTagOptions } from '$lib/tagIndex';
	import { dayLabel, today } from '$lib/semesters';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const projectOptions = $derived(projectTagOptions(data.projects, data.meta, data.notes, data.feedback));
	const peopleOptions = $derived(personTagOptions(data.notes, data.feedback));
	const day = $derived(page.url.searchParams.get('day') ?? today());
	const days = $derived([...new Set([today(), ...data.notes.map((n) => n.date)])].sort().reverse());
	const noteOf = (d: string) => data.notes.find((n) => n.date === d) ?? null;

	function open(d: string) {
		if (d) goto(`/meetings?day=${d}`, { replaceState: true, keepFocus: true, noScroll: true });
	}
</script>

<svelte:head><title>Meeting notes · Ledger</title></svelte:head>

<div class="header-wrap flat">
	<div class="header-top">
		<div>
			<h1>Meeting notes</h1>
			<div class="sub">One note a day, with each meeting under its own heading and coupled to its projects and people.</div>
		</div>
		<div class="header-actions">
			<span class="dn-goto"><DateInput bind:value={() => day, (d) => open(d)} label="Go to a day" /></span>
		</div>
	</div>
</div>

<div class="page">
	<div class="page-inner dn-wrap">
		<div class="dn-split">
			<ul class="dn-days" aria-label="Days">
				{#each days as d (d)}
					{@const n = noteOf(d)}
					<li>
						<button class="mt-row" class:on={d === day} onclick={() => open(d)}>
							<span class="mt-title">{d === today() ? 'Today' : dayLabel(d)}</span>
							<span class="mt-sub">
								{#if n?.meetings.length}{n.meetings.map((m) => m.title || 'Meeting').join(' · ')}{:else}No meetings yet{/if}
							</span>
						</button>
					</li>
				{/each}
			</ul>
			{#key day}
				<MeetingNoteEditor date={day} note={noteOf(day)} {projectOptions} {peopleOptions} />
			{/key}
		</div>
	</div>
</div>
