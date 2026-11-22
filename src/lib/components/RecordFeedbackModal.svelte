<script lang="ts">
	// Feedback written down in Ledger: your own note (for yourself or for Claude), or feedback someone gave you in
	// person, in a meeting (one in a daily note), by mail or by phone, with who and in what role. It lands on a
	// deliverable (or a version of it), a user story, or the project as a whole, and is kept in one of the project's
	// threads like all feedback. Wireframe E2; requirements: Feedback.
	import { api } from '$lib/api';
	import { attempt } from '$lib/ui.svelte';
	import { GIVEN_WAYS, personName, ROLES } from '$lib/v1';
	import FileText from '@lucide/svelte/icons/file-text';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import MessagesSquare from '@lucide/svelte/icons/messages-square';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import { dayLabel } from '$lib/semesters';
	import type { DailyNote, DeliverableSummary, Feedback, FeedbackKind, FeedbackRole, ProjectPerson, ProjectSummary, Story } from '$lib/types';
	import Dropdown from './Dropdown.svelte';
	import TagPicker, { type TagOption } from './TagPicker.svelte';
	import DateInput from './DateInput.svelte';
	import TimeInput from './TimeInput.svelte';
	import { amsterdamMoment, time, todayKey } from '$lib/format';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import Modal from './Modal.svelte';

	let {
		projects,
		peopleOptions,
		people,
		notes,
		project = '',
		onclose,
		onsaved
	}: {
		/** The vault's projects, with their threads. */
		projects: ProjectSummary[];
		/** The indexed person tags ($lib/tagIndex). */
		peopleOptions: TagOption[];
		/** The people of the projects, for the name and role a picked tag stands for. */
		people: ProjectPerson[];
		/** Daily notes, for feedback given in one of their meetings. */
		notes: DailyNote[];
		project?: string;
		onclose: () => void;
		onsaved: (item: Feedback) => void;
	} = $props();

	let kind = $state<FeedbackKind>('someone');
	/** Who gave it: one person tag. */
	let fromTags = $state<string[]>([]);
	const from = $derived(fromTags[0] ?? '');
	let role = $state<FeedbackRole>('coach');
	let given = $state('in person');
	/** The daily note (its day) of the meeting it was given in. */
	let daily = $state('');
	// svelte-ignore state_referenced_locally
	let onProject = $state(project || (projects[0]?.slug ?? ''));
	let on = $state<'deliverable' | 'story' | 'project'>('deliverable');
	let deliverable = $state('');
	let version = $state('');
	let story = $state('');
	let thread = $state('');
	let body = $state('');
	// When it was given, in Amsterdam time: a day and a 24-hour time.
	let atDay = $state(todayKey());
	let atTime = $state(time(new Date().toISOString()));
	let busy = $state(false);

	const chosen = $derived(projects.find((p) => p.slug === onProject));
	// The project's most recently active thread holds it unless another is picked; a project without threads gets one.
	const threads = $derived([...(chosen?.threads ?? [])].sort((a, b) => b.lastActivity.localeCompare(a.lastActivity)));
	$effect(() => {
		const options = threads.map((t) => t.slug);
		if (!options.includes(thread)) thread = options[0] ?? '';
	});

	// The chosen project's deliverables and stories.
	let deliverables = $state<DeliverableSummary[]>([]);
	let stories = $state<Story[]>([]);
	$effect(() => {
		const slug = onProject;
		deliverables = [];
		stories = [];
		if (!slug) return;
		let stale = false;
		api.listDeliverables(slug).then((list) => !stale && (deliverables = list), () => {});
		api.listStories(slug).then((list) => !stale && (stories = list), () => {});
		return () => (stale = true);
	});
	const versions = $derived(deliverables.find((d) => d.path === deliverable)?.versions ?? []);
	// The meetings coupled to the chosen project, newest first: picking one records its day.
	const meetingOptions = $derived([
		{ value: '', label: 'Not linked' },
		...notes.flatMap((n) =>
			n.meetings
				.filter((m) => m.projects.includes(onProject))
				.map((m) => ({ value: n.date, label: `${m.title || 'Meeting'}`, hint: `${dayLabel(n.date)}${m.time ? `, ${m.time}` : ''}`, icon: NotebookPen }))
		)
	]);
	const roleOptions = ROLES.map((r) => ({ value: r.role, label: r.name }));
	const givenOptions = GIVEN_WAYS.map((g) => ({ value: g.given, label: g.name }));
	const projectOptions = $derived(projects.map((p) => ({ value: p.slug, label: p.name, icon: FolderOpen })));
	const onOptions = [
		{ value: 'deliverable', label: 'A deliverable' },
		{ value: 'story', label: 'A user story' },
		{ value: 'project', label: 'The whole project' }
	];
	const deliverableOptions = $derived(deliverables.map((d) => ({ value: d.path, label: d.name, icon: FileText })));
	const versionOptions = $derived([{ value: '', label: 'The live file' }, ...versions.map((v) => ({ value: v, label: `v${v}` }))]);
	const storyOptions = $derived(stories.map((s) => ({ value: s.id, label: `${s.id} ${s.summary}` })));
	const threadOptions = $derived(threads.map((t) => ({ value: t.slug, label: t.title, icon: MessagesSquare })));

	// Someone of the project picked by their tag (`coach/frank`) brings their own name and role.
	const person = $derived(people.find((p) => p.tag === from && p.project === onProject) ?? people.find((p) => p.tag === from));
	$effect(() => {
		if (person) {
			role = person.role;
			return;
		}
		const first = from.split('/')[0];
		const known = ROLES.find((r) => r.role === first);
		if (known) role = known.role;
	});
	const fromName = $derived(person?.name ?? peopleOptions.find((o) => o.value === from)?.label ?? (from ? personName(from) : ''));

	const ready = $derived(
		body.trim() !== '' &&
			onProject !== '' &&
			(on === 'project' || (on === 'deliverable' ? deliverable.trim() !== '' : story !== '')) &&
			(kind === 'own' || from.trim() !== '')
	);

	/** The deliverable's path as feedback names it: the live file, or the version picked. */
	function target(): string | null {
		if (on !== 'deliverable') return null;
		if (!version) return deliverable;
		const dot = deliverable.lastIndexOf('.');
		return dot > deliverable.lastIndexOf('/') ? `${deliverable.slice(0, dot)}@v${version}${deliverable.slice(dot)}` : `${deliverable}@v${version}`;
	}

	async function save() {
		if (!ready || busy) return;
		busy = true;
		const someone = kind === 'someone';
		const saved = await attempt(() =>
			api.recordFeedback({
				project: onProject,
				thread: thread || null,
				deliverable: target(),
				story: on === 'story' ? story : null,
				kind,
				from: someone
					? { name: fromName, portflowUser: person?.portflowUser ?? null, role, given }
					: { name: null, portflowUser: null, role: null, given: null },
				daily: someone && given === 'meeting' ? daily || null : null,
				body: body.trim(),
				at: amsterdamMoment(atDay, atTime)
			})
		);
		busy = false;
		if (saved) onsaved(saved);
	}
</script>

<Modal title="Record feedback" wide {onclose}>
	<div class="seg" role="group" aria-label="Kind of feedback" style="align-self:flex-start">
		<button class:on={kind === 'someone'} aria-pressed={kind === 'someone'} onclick={() => (kind = 'someone')}>From someone else</button>
		<button class:on={kind === 'own'} aria-pressed={kind === 'own'} onclick={() => (kind = 'own')}>Your own note</button>
	</div>
	<p class="hint" style="margin:-4px 0 0">
		{kind === 'own' ? 'A note for yourself, or for Claude when it next works on the deliverable.' : 'Who gave it, in what role and how: feedback can be filtered per person.'}
	</p>

	{#if kind === 'someone'}
		<div class="form-row">
			<div class="field grow">
				<span>From</span>
				<TagPicker bind:value={fromTags} options={peopleOptions} label="From" placeholder="Who gave it, e.g. coach/frank" max={1} />
			</div>
			<div class="field" style="flex:0 0 150px">
				<span>Role</span>
				<Dropdown bind:value={() => role, (v) => (role = v as FeedbackRole)} options={roleOptions} label="Role" full />
			</div>
			<div class="field" style="flex:0 0 150px">
				<span>Given</span>
				<Dropdown bind:value={given} options={givenOptions} label="How it was given" full />
			</div>
		</div>
		{#if given === 'meeting'}
			<div class="field">
				<span>Which meeting</span>
				<Dropdown bind:value={daily} options={meetingOptions} label="Meeting" full />
			</div>
		{/if}
	{/if}

	<div class="form-row">
		<div class="field grow">
			<span>Project</span>
			<Dropdown bind:value={onProject} options={projectOptions} label="Project" full placeholder="Pick a project" />
		</div>
		<div class="field" style="flex:0 0 170px">
			<span>On</span>
			<Dropdown bind:value={() => on, (v) => (on = v as 'deliverable' | 'story' | 'project')} options={onOptions} label="On" full />
		</div>
		<div class="field" style="flex:0 0 290px">
			<span>When</span>
			<span class="fb-when-input"><DateInput bind:value={atDay} label="Day" /><TimeInput bind:value={atTime} label="Time" /></span>
		</div>
	</div>
	{#if on === 'deliverable'}
		<div class="form-row">
			<div class="field grow">
				<span>Deliverable</span>
				{#if deliverables.length}
					<Dropdown bind:value={deliverable} options={deliverableOptions} label="Deliverable" full placeholder="Pick a deliverable" />
				{:else}
					<span class="hint">This project has no deliverables yet: put it on a story or on the whole project.</span>
				{/if}
			</div>
			{#if versions.length}
				<div class="field" style="flex:0 0 170px">
					<span>Version</span>
					<Dropdown bind:value={version} options={versionOptions} label="Version" full />
				</div>
			{/if}
		</div>
	{:else if on === 'story'}
		<div class="field">
			<span>User story</span>
			{#if stories.length}
				<Dropdown bind:value={story} options={storyOptions} label="User story" full placeholder="Pick a story" />
			{:else}
				<span class="hint">This project has no user stories yet.</span>
			{/if}
		</div>
	{/if}

	<div class="field">
		<span>Kept in the thread</span>
		{#if threads.length}
			<Dropdown bind:value={thread} options={threadOptions} label="Thread" full />
		{:else}
			<span class="hint">This project has no threads yet: a thread named "Feedback" is started for it.</span>
		{/if}
	</div>

	<div class="field">
		<span>Feedback</span>
		<MarkdownEditor bind:value={body} rows={5} label="Feedback" placeholder={kind === 'own' ? 'What you want to remember' : 'What they said, in their words where you can'} />
	</div>

	{#snippet footer()}
		<span class="spacer-h"></span>
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={save} disabled={!ready || busy}>Record</button>
	{/snippet}
</Modal>
