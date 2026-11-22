<script lang="ts">
	// One day's meeting note: its meetings, each under its own heading (its title; its time and projects go in the note's
	// frontmatter) with the people who were there, as links to their files in shared/people, and your notes. Saved as
	// you type, as the day's note `meetings/<year>/<month>/<day>.md`. No goal and no template: a meeting is a heading and
	// what you write under it. The people of a meeting's projects come first in its "With", "Whole team" adds every
	// teammate at once, and someone typed who isn't in the vault yet is added to it when the note is saved.
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { tick, untrack } from 'svelte';
	import { api } from '$lib/api';
	import { meetingStart } from '$lib/commands.svelte';
	import { time } from '$lib/format';
	import { dayLabel, today } from '$lib/semesters';
	import { attempt, confirmDialog, refresh } from '$lib/ui.svelte';
	import { PEOPLE } from '$lib/links';
	import { teamOf } from '$lib/v1';
	import type { DailyNote, LinkTarget, MeetingSection, ProjectPerson } from '$lib/types';
	import type { MentionOption } from '$lib/markdownEditor';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import TagPicker, { type TagOption, type TagQuickPick } from './TagPicker.svelte';
	import TimeInput from './TimeInput.svelte';

	let {
		date,
		note,
		projectOptions,
		peopleOptions,
		people = [],
		links = []
	}: {
		date: string;
		/** The day's note as saved; null when the day has none yet. */
		note: DailyNote | null;
		/** The indexed project tags, and the vault's people by id ($lib/tagIndex). */
		projectOptions: TagOption[];
		peopleOptions: TagOption[];
		/** The projects' people: the meeting's own come first, and make up "Whole team". */
		people?: ProjectPerson[];
		/** What [[links]] can point at (list_links). */
		links?: LinkTarget[];
	} = $props();

	/** The project a meeting's [[links]] are written in: its only one. With several or none, a link to a project's
	 *  thread or deliverable names the project (`[[thrym-player/pricing-brief]]`). */
	const homeOf = (m: MeetingSection) => (m.projects.length === 1 ? (m.projects[0] ?? null) : null);

	/** Who can be picked for a meeting: the people of its projects first, then everyone else in the vault. */
	function peopleFor(m: MeetingSection): TagOption[] {
		const own = new Set(people.filter((p) => m.projects.includes(p.project)).map((p) => p.person));
		return [...peopleOptions.filter((o) => own.has(o.value)), ...peopleOptions.filter((o) => !own.has(o.value))];
	}

	/** Who `@` in a meeting's notes offers: the same people as "With", in the same order, each written as a link. */
	function mentionsFor(m: MeetingSection): MentionOption[] {
		return peopleFor(m).map((o) => ({ id: o.value, label: o.label, detail: o.hint ?? o.value, insert: `[[${PEOPLE}${o.value}]]` }));
	}

	/** The people a meeting's notes link to (`[[people/frank]]`), by id. */
	const mentionedIn = (body: string) =>
		[...body.matchAll(/\[\[people\/([^[\]|#\n]+)/gi)].map((match) => (match[1] ?? '').trim().toLowerCase()).filter(Boolean);

	/** New notes: whoever is newly mentioned in them was at the meeting too, and joins "With". */
	function setBody(m: MeetingSection, body: string) {
		const before = new Set(mentionedIn(m.body));
		m.body = body;
		const added = mentionedIn(body).filter((tag) => !before.has(tag) && !m.people.includes(tag));
		if (added.length) m.people = [...m.people, ...added];
	}

	/** "Whole team": every teammate of the meeting's projects, in one click. */
	function quickFor(m: MeetingSection): TagQuickPick[] {
		const team = teamOf(people, m.projects);
		if (!team.length) return [];
		const names = team.map((id) => people.find((p) => p.person === id)?.name ?? id).join(', ');
		return [{ label: 'Whole team', tags: team, title: `Adds ${names}` }];
	}

	// The meetings as edited here, from the saved note (the page keys this component by day).
	let meetings = $state<MeetingSection[]>(untrack(() => structuredClone(note?.meetings ?? [])));
	/** The meetings as this editor and the vault last agreed on them: what was loaded, or last saved. */
	let agreed = untrack(() => JSON.stringify(note?.meetings ?? []));

	// Saved a moment after the last change; a day without meetings has no file.
	let skip = true;
	$effect(() => {
		const snapshot = JSON.stringify(meetings);
		if (skip) {
			skip = false;
			return;
		}
		const timer = setTimeout(async () => {
			const list = JSON.parse(snapshot) as MeetingSection[];
			// A day without meetings (and nothing else in its note) has no file: the backend takes it away.
			const saved = await attempt(() => api.saveDailyNote({ date, meetings: list }));
			if (saved !== undefined) agreed = snapshot;
			await refresh();
		}, 700);
		return () => clearTimeout(timer);
	});

	// The note changed somewhere else (Obsidian, a sync from another machine): it comes in, unless there is something
	// typed here that isn't saved yet, which then wins and is saved over it.
	$effect(() => {
		const incoming = JSON.stringify(note?.meetings ?? []);
		untrack(() => {
			if (incoming === agreed || JSON.stringify(meetings) !== agreed) return;
			agreed = incoming;
			skip = true;
			meetings = structuredClone(note?.meetings ?? []);
		});
	});

	/** The time now in Amsterdam, rounded down to the half hour ("14:30"), as the app shows every time (format.ts). */
	function now(): string {
		const [hours, minutes] = time(new Date().toISOString()).split(':');
		return `${hours}:${Number(minutes) < 30 ? '00' : '30'}`;
	}

	let root = $state<HTMLElement>();

	/** Adds a meeting (or, when the last one is still empty, keeps that one) and puts the cursor in its title. */
	async function add() {
		const last = meetings.at(-1);
		const blank = last && !last.title.trim() && !last.body.trim();
		if (!blank) meetings = [...meetings, { time: now(), title: '', projects: last?.projects ?? [], people: [], body: '' }];
		await tick();
		const titles = root?.querySelectorAll<HTMLInputElement>('.dn-title');
		const input = titles?.[titles.length - 1];
		input?.scrollIntoView({ block: 'center', behavior: 'smooth' });
		input?.focus();
	}

	// "Start a meeting note" (Ctrl+Shift+D unless changed) opens today and asks for a meeting here.
	$effect(() => {
		if (!meetingStart.pending || date !== today()) return;
		meetingStart.pending = false;
		void untrack(add);
	});

	async function remove(i: number) {
		const m = meetings[i];
		if (m.body.trim() || m.title.trim()) {
			const ok = await confirmDialog({
				title: `Delete "${m.title.trim() || 'this meeting'}"?`,
				message: 'Its heading and your notes under it are taken out of the daily note.',
				confirmLabel: 'Delete',
				danger: true
			});
			if (!ok) return;
		}
		meetings = meetings.filter((_, k) => k !== i);
	}
</script>

<section class="dn-day" aria-label="Meeting notes {dayLabel(date)}" bind:this={root}>
	<header class="dn-head">
		<div>
			<h2>{dayLabel(date)}</h2>
		</div>
		<div class="header-actions">
			<button class="btn small primary" onclick={add}><Plus size={14} />Add meeting</button>
		</div>
	</header>

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
						<TagPicker bind:value={m.people} options={peopleFor(m)} quick={quickFor(m)} label="With" placeholder="Who was there" prefix={PEOPLE} />
					</div>
				</div>
				<MarkdownEditor
					bind:value={() => m.body, (v) => setBody(m, v)}
					rows={4}
					label="Notes"
					placeholder="What was said, decided, to do... Type @ to mention someone, [[ to link a document."
					people={mentionsFor(m)}
					linkTargets={links}
					linkHome={homeOf(m)}
				/>
			</article>
		{:else}
			<div class="empty-state">
				<p>No meetings on this day yet.</p>
				<button class="btn primary" onclick={add}><Plus size={14} />Add a meeting</button>
				<p class="hint">Each meeting is a heading in the day's note: one note per day, however many meetings.</p>
			</div>
		{/each}
</section>
