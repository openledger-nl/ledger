<script lang="ts">
	// A new deliverable starts with its goal, in your own words; the template, the question it answers and the label
	// follow from it. Ledger numbers it (01, 02, …) in the order deliverables are made. Wireframe D1; requirements:
	// Deliverables.
	import { slugify } from '$lib/v1';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api';
	import { deliverableUrl } from '$lib/actions';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { MmmLevel, ProjectMeta, Template } from '$lib/types';
	import Dropdown from './Dropdown.svelte';
	import GuidingQuestions from './GuidingQuestions.svelte';
	import FileText from '@lucide/svelte/icons/file-text';
	import CircleHelp from '@lucide/svelte/icons/circle-help';
	import Modal from './Modal.svelte';
	import MmmPicker from './MmmPicker.svelte';

	let {
		project,
		answers = null,
		onclose
	}: {
		project: string;
		answers?: string | null;
		onclose: () => void;
	} = $props();

	let goal = $state('');
	let name = $state('');
	let template = $state('');
	// svelte-ignore state_referenced_locally
	let answer = $state(answers ?? '');
	let mmm = $state<MmmLevel | ''>('');
	let firstDraft = $state<'me' | 'claude'>('me');
	let busy = $state(false);

	let meta = $state<ProjectMeta | null>(null);
	let templates = $state<Template[]>([]);
	let number = $state(1);
	$effect(() => {
		const slug = project;
		let stale = false;
		Promise.all([api.getProjectMeta(slug), api.effectiveTemplates(slug).catch(() => []), api.nextDeliverableNumber(slug).catch(() => 1)]).then(([m, t, next]) => {
			if (stale) return;
			meta = m;
			templates = t;
			if (m?.intendedMmm && !mmm) mmm = m.intendedMmm;
			number = next;
		});
		return () => (stale = true);
	});

	const chosen = $derived(templates.find((t) => t.id === template));
	/** The answers to the template's guiding questions, one per question; they start blank when the template changes. */
	let guided = $state<string[]>([]);
	$effect(() => {
		guided = (chosen?.questions ?? []).map(() => '');
	});
	/** The questions answered, as a %% note for whoever writes the first draft: seen in source mode, and by Claude. */
	const answersNote = $derived.by(() => {
		const done = (chosen?.questions ?? []).map((q, i) => [q, guided[i]?.trim() ?? ''] as const).filter(([, a]) => a);
		return done.length ? `%% Answers to the guiding questions:\n\n${done.map(([q, a]) => `**${q}**\n${a}`).join('\n\n')}\n%%\n\n` : '';
	});
	const templateOptions = $derived([
		{ value: '', label: 'None: an empty file', hint: 'Start from a blank page.' },
		...templates.map((t) => ({ value: t.id, label: t.name, hint: t.trigger ? `Use when ${t.trigger}` : undefined, icon: FileText, group: t.scope ? 'This project' : 'Every project' }))
	]);
	const answerOptions = $derived([
		{ value: '', label: 'No question' },
		...(meta?.mainQuestion ? [{ value: 'main', label: 'The main question', hint: meta.mainQuestion, icon: CircleHelp }] : []),
		...(answer && answer !== 'main' ? [{ value: answer, label: answer, icon: CircleHelp }] : [])
	]);
	// The file as the backend names it: the number and the name, as a slug ("01-project-plan.md").
	const fileName = $derived(`${slugify(`${String(number).padStart(2, '0')} ${name.trim().replace(/\.md$/i, '')}`)}${name.trim() ? '' : '-...'}.md`);
	const manMade = $derived(mmm === 'man-made');
	$effect(() => {
		if (manMade) firstDraft = 'me';
	});
	// The name follows the template until you type one.
	let named = $state(false);
	$effect(() => {
		if (!named && chosen) name = chosen.name;
	});
	const ready = $derived(goal.trim().length >= 10 && name.trim() !== '');

	async function create() {
		if (!ready || busy) return;
		busy = true;
		// The template's skeleton, with its readers noted under its title.
		const skeleton = chosen ? ((await attempt(() => api.templateSkeleton(project, chosen.id))) ?? chosen.body) : '';
		const body = `# ${name.trim().replace(/\.md$/i, '')}\n\n> **Goal:** ${goal.trim()}\n\n${answersNote}${skeleton}`.trimEnd() + '\n';
		const made = await attempt(() => api.createDeliverable(project, fileName, body));
		if (made) {
			await attempt(() =>
				api.saveDeliverableMeta({ project, path: made.path, number, goal: goal.trim(), mmm: mmm || null, answers: answer || null, template: template || null })
			);
		}
		busy = false;
		if (!made) return;
		onclose();
		await refresh();
		toast(firstDraft === 'claude' ? 'Deliverable created: start a prompt from its thread to let Claude write the first draft' : 'Deliverable created');
		await goto(deliverableUrl(project, made.path));
	}
</script>

<Modal title="New deliverable" wide {onclose}>
	<label class="field">
		<span>Goal <span class="req">required</span></span>
		<textarea
			bind:value={goal}
			rows="3"
			data-autofocus
			placeholder="In your own words, what must it do? e.g. This presentation must make my ERP migration clear to the client, and show the study that I do it in phases."
		></textarea>
		<span class="hint">Goes onto the deliverable and into every prompt for it. Ledger never writes it for you.</span>
	</label>

	{#if meta?.goal}
		<p class="dm-project-goal"><strong>Project goal</strong> {meta.goal}</p>
	{/if}

	<div class="form-row">
		<div class="field grow">
			<span>Template</span>
			<Dropdown bind:value={template} options={templateOptions} label="Template" full />
		</div>
		<label class="field grow">
			<span>Name</span>
			<input bind:value={name} oninput={() => (named = true)} placeholder="e.g. Project plan" />
		</label>
	</div>
	{#if chosen?.questions.length}
		<GuidingQuestions questions={chosen.questions} bind:answers={guided} template={chosen.name} />
	{/if}
	<p class="hint" style="margin:-6px 0 0">Saved as <code>deliverables/{fileName}</code>. The number stays with it: deleting another deliverable doesn't renumber, and a file renamed outside Ledger keeps it while its name starts with it.</p>

	<div class="form-row">
		<div class="field grow">
			<span>Answers</span>
			<Dropdown bind:value={answer} options={answerOptions} label="The question it answers" full />
		</div>
		<div class="field grow">
			<span>Its AI-use label</span>
			<MmmPicker bind:value={mmm} label="Its AI-use label" />
		</div>
	</div>
	{#if manMade}<p class="hint" style="margin:-6px 0 0">Man-Made: AI writing is switched off for this deliverable. You can change the label at any time.</p>{/if}

	<div class="field">
		<span>First draft</span>
		<label class="check"><input type="radio" bind:group={firstDraft} value="me" /> I write it</label>
		<label class="check" class:disabled={manMade}>
			<input type="radio" bind:group={firstDraft} value="claude" disabled={manMade} /> Claude Code writes a first draft from a prompt with this goal
		</label>
		<span class="hint">Off by default: Ledger keeps you in control and never does the research for you.</span>
	</div>

	{#snippet footer()}
		<span class="spacer-h"></span>
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={create} disabled={!ready || busy}>Create deliverable</button>
	{/snippet}
</Modal>
