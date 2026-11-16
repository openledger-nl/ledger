<script lang="ts">
	// Feedback written down in Ledger: your own note (for yourself or for Claude), or feedback someone gave you in
	// person, by mail or in a meeting (one in a daily note), with who and in what role. It lands on a deliverable (or a version of it) or on
	// the project. Wireframe E2; requirements: Feedback.
	import { api } from '$lib/api';
	import { mockApi } from '$lib/mock/api';
	import { attempt } from '$lib/ui.svelte';
	import { ROLES } from '$lib/v1';
	import FileText from '@lucide/svelte/icons/file-text';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import { dayLabel } from '$lib/semesters';
	import type { DailyNote, DeliverableSummary, FeedbackItem, FeedbackKind, FeedbackRole } from '$lib/types';
	import Dropdown from './Dropdown.svelte';
	import TagPicker, { type TagOption } from './TagPicker.svelte';
	import DateInput from './DateInput.svelte';
	import TimeInput from './TimeInput.svelte';
	import { time, todayKey } from '$lib/format';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import Modal from './Modal.svelte';
	import PreviewBadge from './PreviewBadge.svelte';

	let {
		projectTags,
		projectName,
		vaultProjects,
		peopleOptions,
		notes,
		project = '',
		onclose,
		onsaved
	}: {
		projectTags: string[];
		projectName: (tag: string) => string;
		/** The projects in this vault, by slug: their deliverables can be picked. */
		vaultProjects: string[];
		/** The indexed person tags ($lib/tagIndex). */
		peopleOptions: TagOption[];
		/** Daily notes, for feedback given in one of their meetings. */
		notes: DailyNote[];
		project?: string;
		onclose: () => void;
		onsaved: (item: FeedbackItem) => void;
	} = $props();

	const GIVEN = ['in person', 'in a meeting', 'by mail', 'by phone', 'other'];

	let kind = $state<FeedbackKind>('someone');
	/** Who gave it: one person tag. */
	let fromTags = $state<string[]>([]);
	const from = $derived(fromTags[0] ?? '');
	let role = $state<FeedbackRole>('coach');
	let given = $state('in person');
	/** The daily note (its day) of the meeting it was given in. */
	let daily = $state('');
	// svelte-ignore state_referenced_locally
	let onProject = $state(project);
	let on = $state<'deliverable' | 'project'>('deliverable');
	let deliverable = $state('');
	let version = $state('');
	let body = $state('');
	// When it was given, in Amsterdam time: a day and a 24-hour time, stored as "2026-11-09T14:05".
	let atDay = $state(todayKey());
	let atTime = $state(time(new Date().toISOString()));
	const at = $derived(`${atDay}T${atTime || '00:00'}`);
	let busy = $state(false);

	// The chosen project's deliverables, when it is one of this vault's.
	let deliverables = $state<DeliverableSummary[]>([]);
	$effect(() => {
		const slug = onProject;
		deliverables = [];
		if (!vaultProjects.includes(slug)) return;
		let stale = false;
		api.listDeliverables(slug).then((list) => !stale && (deliverables = list), () => {});
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
	const givenOptions = GIVEN.map((g) => ({ value: g, label: g.charAt(0).toUpperCase() + g.slice(1) }));
	const projectOptions = $derived(projectTags.map((t) => ({ value: t, label: projectName(t), icon: FolderOpen })));
	const onOptions = [
		{ value: 'deliverable', label: 'A deliverable' },
		{ value: 'project', label: 'The whole project' }
	];
	const deliverableOptions = $derived(deliverables.map((d) => ({ value: d.path, label: d.name, icon: FileText })));
	const versionOptions = $derived([{ value: '', label: 'The live file' }, ...versions.map((v) => ({ value: v, label: `v${v}` }))]);

	// A person tagged with a role and a name (`expert/sander`) sets the role too.
	$effect(() => {
		const first = from.split('/')[0];
		const known = ROLES.find((r) => r.role === first);
		if (known) role = known.role;
	});

	const ready = $derived(
		body.trim() !== '' && onProject !== '' && (on === 'project' || deliverable.trim() !== '') && (kind === 'own' || from.trim() !== '')
	);

	async function save() {
		if (!ready || busy) return;
		busy = true;
		const saved = await attempt(() =>
			mockApi.recordFeedback({
				kind,
				from: kind === 'someone' ? from.trim().toLowerCase() : null,
				role: kind === 'someone' ? role : null,
				given: kind === 'someone' ? given : null,
				daily: kind === 'someone' && given === 'in a meeting' ? daily || null : null,
				target: on === 'project' ? { on: 'project', project: onProject } : { on: 'deliverable', project: onProject, deliverable: deliverable.trim(), version: version || null },
				body: body.trim(),
				at
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
		{#if given === 'in a meeting'}
			<div class="field">
				<span>Which meeting</span>
				<Dropdown bind:value={daily} options={meetingOptions} label="Meeting" full placeholder="Pick a project first" />
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
			<Dropdown bind:value={() => on, (v) => (on = v as 'deliverable' | 'project')} options={onOptions} label="On" full />
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
					<input bind:value={deliverable} placeholder="e.g. 01 Project plan.md" />
				{/if}
			</div>
			<div class="field" style="flex:0 0 170px">
				<span>Version</span>
				{#if versions.length}
					<Dropdown bind:value={version} options={versionOptions} label="Version" full />
				{:else}
					<input bind:value={version} placeholder="e.g. 1.0, or empty" />
				{/if}
			</div>
		</div>
	{/if}

	<div class="field">
		<span>Feedback</span>
		<MarkdownEditor bind:value={body} rows={5} label="Feedback" placeholder={kind === 'own' ? 'What you want to remember' : 'What they said, in their words where you can'} />
	</div>

	{#snippet footer()}
		<PreviewBadge />
		<span class="spacer-h"></span>
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={save} disabled={!ready || busy}>Record</button>
	{/snippet}
</Modal>
