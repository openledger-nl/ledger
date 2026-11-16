<script lang="ts">
	// One day's meeting note: its meetings, each under its own heading (time, title, the projects it belongs to) with
	// the people who were there and your notes. Saved as you type, as the day's note `Daily/<day>.md`, with frontmatter
	// that gathers the meetings' tags. No goal and no template: a meeting is a heading and what you write under it.
	// The people of a meeting's projects come first in its "With", and "Whole team" adds every teammate at once.
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { untrack } from 'svelte';
	import { dailyTags } from '$lib/daily';
	import { api } from '$lib/api';
	import { dayLabel } from '$lib/semesters';
	import { attempt, confirmDialog, refresh } from '$lib/ui.svelte';
	import { teamTags } from '$lib/v1';
	import type { DailyNote, MeetingSection, ProjectPerson } from '$lib/types';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import TagPicker, { type TagOption, type TagQuickPick } from './TagPicker.svelte';
	import TimeInput from './TimeInput.svelte';

	let {
		date,
		note,
		projectOptions,
		peopleOptions,
		people = []
	}: {
		date: string;
		/** The day's note as saved; null when the day has none yet. */
		note: DailyNote | null;
		/** The indexed project and person tags ($lib/tagIndex). */
		projectOptions: TagOption[];
		peopleOptions: TagOption[];
		/** The projects' people: the meeting's own come first, and make up "Whole team". */
		people?: ProjectPerson[];
	} = $props();

	/** Who can be picked for a meeting: the people of its projects first, then everyone else in the index. */
	function peopleFor(m: MeetingSection): TagOption[] {
		const own = new Set(people.filter((p) => m.projects.includes(p.project)).map((p) => p.tag));
		return [...peopleOptions.filter((o) => own.has(o.value)), ...peopleOptions.filter((o) => !own.has(o.value))];
	}

	/** "Whole team": every teammate of the meeting's projects, in one click. */
	function quickFor(m: MeetingSection): TagQuickPick[] {
		const tags = teamTags(people, m.projects);
		if (!tags.length) return [];
		const names = tags.map((t) => people.find((p) => p.tag === t)?.name ?? t).join(', ');
		return [{ label: 'Whole team', tags, title: `Adds ${names}` }];
	}

	// The meetings as edited here, from the saved note once per day (the page keys this component by day).
	let meetings = $state<MeetingSection[]>(untrack(() => structuredClone(note?.meetings ?? [])));
	const id = untrack(() => note?.id ?? `daily-${date}`);
	let saveState = $state<'saved' | 'saving' | 'unsaved'>('saved');
	const tags = $derived(dailyTags({ meetings }));
	const nameOf = (tag: string) =>
		tag.startsWith('project/')
			? (projectOptions.find((o) => o.value === tag.slice(8))?.label ?? tag.slice(8))
			: (peopleOptions.find((o) => o.value === tag)?.label ?? tag);

	// Saved a moment after the last change; a day without meetings has no file.
	let first = true;
	$effect(() => {
		const snapshot = JSON.stringify(meetings);
		if (first) {
			first = false;
			return;
		}
		saveState = 'unsaved';
		const timer = setTimeout(async () => {
			saveState = 'saving';
			const list = JSON.parse(snapshot) as MeetingSection[];
			// A day without meetings (and nothing else in its note) has no file: the backend takes it away.
			const saved = await attempt(() => api.saveDailyNote({ date, id, meetings: list }));
			saveState = saved === undefined ? 'unsaved' : 'saved';
			await refresh();
		}, 700);
		return () => clearTimeout(timer);
	});

	function now(): string {
		const d = new Date();
		return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes() < 30 ? 0 : 30).padStart(2, '0')}`;
	}

	function add() {
		const last = meetings.at(-1);
		meetings = [...meetings, { time: now(), title: '', projects: last?.projects ?? [], people: [], body: '' }];
	}

	async function remove(i: number) {
		const m = meetings[i];
		if (m.body.trim() || m.title.trim()) {
			const ok = await confirmDialog({
				title: `Delete “${m.title.trim() || 'this meeting'}”?`,
				message: 'Its heading and your notes under it are taken out of the daily note.',
				confirmLabel: 'Delete',
				danger: true
			});
			if (!ok) return;
		}
		meetings = meetings.filter((_, k) => k !== i);
	}
</script>

<section class="dn-day" aria-label="Meeting notes {dayLabel(date)}">
	<header class="dn-head">
		<div>
			<h2>{dayLabel(date)}</h2>
			<div class="dn-path"><code>Daily/{date}.md</code> · {saveState === 'saved' ? 'Saved' : saveState === 'saving' ? 'Saving…' : 'Unsaved changes'}</div>
		</div>
		<div class="header-actions">
			<button class="btn small primary" onclick={add}><Plus size={14} />Add meeting</button>
		</div>
	</header>

	{#if tags.length}
		<div class="dn-tags" title="The frontmatter's tags: gathered from the meetings">
			<span class="hint">tags</span>
			{#each tags as t (t)}<span class="chip plain" title="#{t}">{nameOf(t)}</span>{/each}
		</div>
	{/if}

	{#each meetings as m, i (i)}
			<article class="dn-meeting">
				<div class="dn-heading">
					<TimeInput bind:value={() => m.time ?? '', (v) => (m.time = v || null)} label="Time" />
					<input class="dn-title" bind:value={m.title} placeholder="Title, e.g. Coach meeting" aria-label="Title" />
					<button class="tb-icon" title="Delete this meeting" aria-label="Delete this meeting" onclick={() => remove(i)}><Trash2 size={15} /></button>
				</div>
				<div class="dn-couple">
					<div class="field">
						<span>Projects</span>
						<TagPicker bind:value={m.projects} options={projectOptions} label="Projects" placeholder="Couple to a project" prefix="project/" />
					</div>
					<div class="field">
						<span>With</span>
						<TagPicker bind:value={m.people} options={peopleFor(m)} quick={quickFor(m)} label="With" placeholder="Who was there" />
					</div>
				</div>
				<MarkdownEditor bind:value={m.body} rows={4} label="Notes" placeholder="What was said, decided, to do…" />
			</article>
		{:else}
			<div class="empty-state">
				<p>No meetings on this day yet.</p>
				<button class="btn primary" onclick={add}><Plus size={14} />Add a meeting</button>
				<p class="hint">Each meeting is a heading in the day's note: one note per day, however many meetings.</p>
			</div>
		{/each}
</section>
