<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import X from '@lucide/svelte/icons/x';
	import { onMount } from 'svelte';
	import { api, claudeCodeApi } from '$lib/api';
	import { baseName } from '$lib/actions';
	import { autoRunNewPrompt } from '$lib/claudeRuns.svelte';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { DeliverableSummary, Feedback, Prompt, PromptDeliverable, Template } from '$lib/types';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import Modal from './Modal.svelte';

	/**
	 * The one dialog for a prompt, wherever it is made or edited. A new prompt (`prompt` null) starts with the deliverable it
	 * is about: an existing one, which the dialog opens on when it is given a `deliverable` (the one being edited, say), or a
	 * new one. Then what follows depends on that: for a new deliverable, the template to start from; for an existing one,
	 * the feedback on it to resolve (`feedback` are the pieces already selected). The base prompt is filled in from the template
	 * or the feedback and stays folded away until it is wanted; what the person adds goes in their own box. A prompt has no
	 * title: Claude Code names it afterwards. Editing one shows the same two texts, and the deliverables it is about.
	 * The dialog is mounted fresh each time, so it only reads the initial values.
	 */
	let {
		project,
		thread,
		prompt,
		deliverable = '',
		feedback = [],
		onsaved,
		onclose
	}: {
		project: string;
		thread: string;
		prompt: Prompt | null;
		deliverable?: string;
		feedback?: string[];
		onsaved?: (saved: Prompt) => void;
		onclose: () => void;
	} = $props();

	function start() {
		return {
			kind: deliverable ? ('existing' as const) : ('new' as const),
			chosen: deliverable,
			selected: [...feedback],
			base: prompt?.base ?? '',
			body: prompt?.body ?? '',
			linked: prompt?.deliverables ?? []
		};
	}
	const initial = start();

	// ---- what the prompt is about (new) ------------------------------------------------------------

	let kind = $state<'new' | 'existing'>(initial.kind);
	let existing = $state<DeliverableSummary[]>([]);
	let chosen = $state(initial.chosen);
	/** For a new deliverable: its file name, when the person gives one. Blank leaves the naming to whatever produces it. */
	let fileName = $state('');
	let templates = $state<Template[]>([]);
	let templateId = $state('');
	/** The open feedback on the chosen deliverable, and the pieces selected. */
	let open = $state<Feedback[]>([]);
	let selected = $state<string[]>(initial.selected);
	const template = $derived(templates.find((t) => t.id === templateId));

	// ---- the two texts ------------------------------------------------------------------------------

	let base = $state(initial.base);
	/** The person has written in the base: it is no longer filled in again when the template or the feedback changes. */
	let baseDirty = $state(false);
	let body = $state(initial.body);

	let saving = $state(false);
	// Whether saving a new prompt starts it in Claude Code by itself, and whether Claude Code names it: both are settings.
	let autoRun = $state(false);
	let naming = $state(false);

	onMount(async () => {
		if (prompt) {
			existing = ((await attempt(() => api.listDeliverables(project))) ?? []).filter((d) => d.exists);
			return;
		}
		try {
			const settings = await claudeCodeApi.settings();
			autoRun = settings.autoRun;
			naming = settings.namePrompts && !!settings.resolvedPath;
		} catch {
			// The lines about them are only a courtesy.
		}
		templates = (await attempt(() => api.effectiveTemplates(project))) ?? [];
		existing = ((await attempt(() => api.listDeliverables(project))) ?? []).filter((d) => d.exists);
		if (kind === 'existing' && !existing.some((d) => d.path === chosen)) chosen = existing[0]?.path ?? '';
	});

	// The open feedback of the chosen deliverable.
	$effect(() => {
		const path = kind === 'existing' ? chosen : '';
		if (prompt || !path) {
			open = [];
			return;
		}
		let stale = false;
		void api.getDeliverable(project, path).then(
			(d) => {
				if (stale) return;
				open = d.feedback.filter((f) => f.status === 'open');
				selected = selected.filter((id) => open.some((f) => f.id === id));
			},
			() => {
				if (!stale) open = [];
			}
		);
		return () => (stale = true);
	});

	// The base is filled in from the template (a new deliverable) or from the feedback selected (an existing one), until the
	// person has written in it.
	$effect(() => {
		if (prompt) return;
		const from = kind === 'new' ? templateId : '';
		const ids = kind === 'existing' ? [...selected] : [];
		if (baseDirty) return;
		let stale = false;
		void api.suggestPromptBase(project, from || null, ids).then(
			(text) => {
				if (!stale && !baseDirty) base = text;
			},
			() => {}
		);
		return () => (stale = true);
	});

	async function resetBase() {
		const text = await attempt(() => api.suggestPromptBase(project, kind === 'new' ? templateId || null : null, kind === 'existing' ? selected : []));
		if (text === undefined) return;
		base = text;
		baseDirty = false;
	}

	const toggle = (id: string) => (selected = selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id]);
	const allSelected = $derived(open.length > 0 && selected.length === open.length);
	const baseNote = $derived(
		!base.trim()
			? 'nothing yet'
			: baseDirty || prompt
				? `${base.trim().split(/\s+/).length} words`
				: kind === 'existing'
					? `filled in from ${selected.length} piece${selected.length === 1 ? '' : 's'} of feedback`
					: `filled in from the template “${template?.name ?? ''}”`
	);

	// ---- the deliverables of a prompt being edited ----------------------------------------------------

	let linked = $state<PromptDeliverable[]>(initial.linked);
	let adding = $state('');
	let newName = $state('');
	const linkable = $derived(existing.filter((d) => !linked.some((l) => l.path === d.path)));

	async function change(action: () => Promise<Prompt>) {
		const done = await attempt(action);
		if (!done) return;
		linked = done.deliverables;
		adding = '';
		newName = '';
		await refresh();
	}
	const link = () => prompt && adding && change(() => api.linkPromptDeliverable(project, thread, prompt.id, adding));
	const unlink = (path: string) => prompt && change(() => api.unlinkPromptDeliverable(project, thread, prompt.id, path));
	const startNew = () => prompt && newName.trim() && change(() => api.createPromptDeliverable(project, thread, prompt.id, newName));

	// ---- saving -------------------------------------------------------------------------------------

	const empty = $derived(!base.trim() && !body.trim());

	async function save() {
		saving = true;
		const saved = await attempt(() =>
			prompt
				? api.updatePrompt(project, thread, prompt.id, base, body)
				: api.createPrompt(project, thread, {
						base,
						body,
						templateId: kind === 'new' ? templateId || null : null,
						deliverable: kind === 'existing' ? chosen : null,
						newDeliverable: kind === 'new' ? fileName.trim() || null : null,
						feedback: kind === 'existing' ? selected : []
					})
		);
		saving = false;
		if (!saved) return;
		if (!prompt && kind === 'new' && template?.questions.length) {
			toast(`Added ${template.questions.length} guiding question${template.questions.length === 1 ? '' : 's'} to the thread`);
		}
		await refresh();
		// Not awaited: the prompt is made, and the run reports for itself.
		if (!prompt) void autoRunNewPrompt(project, thread, saved);
		onsaved?.(saved);
		onclose();
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) void save();
	}
</script>

<svelte:window {onkeydown} />

<Modal title={prompt ? 'Edit prompt' : 'New prompt'} wide {onclose}>
	<p class="hint" style="margin:0">
		A prompt asks for a deliverable. Whatever produces it — you, an assistant — lands the file in the project’s deliverables.
		{#if !prompt}{naming ? 'Claude Code gives the prompt a name from what it asks.' : 'The prompt is called by the start of its text.'}{/if}
	</p>

	{#if prompt}
		<div class="field">
			Deliverables it is about
			<div class="chips">
				{#each linked as d (d.path)}
					<span class="chip" class:plain={!d.exists}>
						<span title={d.exists ? '' : 'This file is no longer in the project'}>{baseName(d.path)}{d.exists ? '' : ' (missing)'}</span>
						<button class="x" aria-label="Take {baseName(d.path)} off the prompt" onclick={() => unlink(d.path)}><X size={12} /></button>
					</span>
				{:else}
					<span class="hint">None: whatever produces the prompt names the file</span>
				{/each}
			</div>
			<div class="form-row" style="flex-wrap:nowrap; align-items:stretch">
				<select bind:value={adding} aria-label="Add an existing deliverable" style="flex:1">
					<option value="">Add an existing deliverable…</option>
					{#each linkable as d (d.path)}<option value={d.path}>{d.path}</option>{/each}
				</select>
				<button class="btn small" disabled={!adding} onclick={link}>Add</button>
			</div>
			<div class="form-row" style="flex-wrap:nowrap; align-items:stretch">
				<input bind:value={newName} placeholder="Or start a new one, named…" aria-label="Name of a new deliverable" spellcheck="false" maxlength="120" style="flex:1" />
				<button class="btn small" disabled={!newName.trim()} onclick={startNew}>Create</button>
			</div>
		</div>
	{:else}
		<div class="field">
			Deliverable
			<label class="check">
				<input type="radio" bind:group={kind} value="new" />
				A new deliverable: the prompt asks for a file that doesn’t exist yet
			</label>
			{#if kind === 'new'}
				<input
					bind:value={fileName}
					placeholder="File name (optional), e.g. pricing-brief.md"
					aria-label="File name of the new deliverable"
					spellcheck="false"
					maxlength="120"
				/>
				<span class="hint">Leave it blank and the AI names the file; give a name and the file is started now, for the prompt to fill in.</span>
			{/if}
			<label class="check">
				<input type="radio" bind:group={kind} value="existing" disabled={!existing.length} />
				An existing deliverable: the prompt asks for changes to it{existing.length ? '' : ' (there are none in this project yet)'}
			</label>
			{#if kind === 'existing'}
				<select bind:value={chosen} aria-label="The existing deliverable">
					{#each existing as d (d.path)}
						<option value={d.path}>{d.path}{d.versions.length ? ` · latest version v${d.versions.at(-1)}` : ' · not published yet'}</option>
					{/each}
				</select>
			{/if}
		</div>

		{#if kind === 'new' && templates.length}
			<label class="field">
				Start from a template
				<select bind:value={templateId}>
					<option value="">No template</option>
					{#each templates as t (t.scope + '/' + t.id)}
						<option value={t.id}>{t.name}{t.scope ? '' : ' (all projects)'}</option>
					{/each}
				</select>
			</label>
			{#if template}
				<div class="notice info">
					{#if template.trigger}<b>Use when:</b> {template.trigger}<br />{/if}
					{#if template.questions.length}
						{template.questions.length} guiding question{template.questions.length === 1 ? '' : 's'} will be added to this thread to answer first.
					{:else}
						This template has no guiding questions.
					{/if}
				</div>
			{/if}
		{/if}

		{#if kind === 'existing'}
			<div class="field">
				<span class="pick-title">
					Feedback to resolve
					{#if open.length}
						<span class="pick-actions">
							<button class="link accent" onclick={() => (selected = allSelected ? [] : open.map((f) => f.id))}>
								{allSelected ? 'Clear' : `Add all feedback (${open.length})`}
							</button>
						</span>
					{/if}
				</span>
				{#if open.length}
					<div class="pick-list">
						{#each open as f (f.id)}
							<label class="pick-row">
								<input type="checkbox" checked={selected.includes(f.id)} onchange={() => toggle(f.id)} />
								<span class="pick-text">
									<span class="pick-head">{f.givenOn ? `v${f.givenOn}` : 'Live file'}</span>
									{#if f.quote}<span class="pick-quote">“{f.quote}”</span>{/if}
									<span class="pick-body">{f.body}</span>
								</span>
							</label>
						{/each}
					</div>
				{:else}
					<span class="hint">There is no open feedback on this deliverable.</span>
				{/if}
			</div>
		{/if}
	{/if}

	<details class="base-field">
		<summary>
			<span class="disclosure"><ChevronRight size={14} /></span>
			Base prompt
			<span class="hint">{baseNote}</span>
		</summary>
		<div class="field" style="margin-top:8px">
			<MarkdownEditor bind:value={base} rows={8} label="Base prompt" placeholder="Filled in from the template or the feedback. Write here to change it." oninput={() => (baseDirty = true)} />
			{#if baseDirty && !prompt}
				<div><button class="link accent" onclick={resetBase}>Fill it in again</button></div>
			{/if}
		</div>
	</details>

	<div class="field">
		What do you want?
		<MarkdownEditor bind:value={body} rows={5} label="What do you want?" placeholder="Anything to add: audience, scope, what good looks like. Markdown works." />
	</div>

	{#snippet footer()}
		<span class="spacer-h"></span>
		{#if !prompt && autoRun}<span class="hint">Claude Code starts on it as soon as it is created</span>{/if}
		<span class="hint">Ctrl+Enter saves</span>
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={save} disabled={saving || empty || (!prompt && kind === 'existing' && !chosen)}>{prompt ? 'Save' : 'Create prompt'}</button>
	{/snippet}
</Modal>
