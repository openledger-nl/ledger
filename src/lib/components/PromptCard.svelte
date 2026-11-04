<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Trash from '@lucide/svelte/icons/trash';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import { baseName, deliverableUrl, promptName } from '$lib/actions';
	import { time } from '$lib/format';
	import { attempt, confirmDialog, refresh } from '$lib/ui.svelte';
	import type { Feedback, Prompt } from '$lib/types';
	import ClaudeRunPanel from './ClaudeRunPanel.svelte';
	import Markdown from './Markdown.svelte';
	import Menu from './Menu.svelte';

	/**
	 * In a thread, a card shows the deliverables the prompt is about (they are changed in Edit). On a deliverable's own page
	 * (`onDeliverable`) it shows instead the feedback the prompt should resolve (the page's `feedback`, by id), and can add
	 * more (`onaddfeedback`). `onfeedback` goes to a piece of feedback.
	 */
	let {
		project,
		thread,
		prompt,
		onedit,
		onDeliverable = false,
		feedback = [],
		onfeedback,
		onaddfeedback
	}: {
		project: string;
		thread: string;
		prompt: Prompt;
		onedit: (prompt: Prompt) => void;
		onDeliverable?: boolean;
		feedback?: Feedback[];
		onfeedback?: (id: string) => void;
		onaddfeedback?: () => void;
	} = $props();

	/** The feedback this prompt covers that is on this deliverable. */
	const covered = $derived(prompt.feedback.map((id) => feedback.find((f) => f.id === id)).filter((f): f is Feedback => !!f));
	let expanded = $state(false);
	const long = $derived(prompt.body.length > 420);

	async function detach(id: string) {
		if (await attempt(() => api.detachPromptFeedback(project, thread, prompt.id, id))) await refresh();
	}

	async function remove() {
		const ok = await confirmDialog({
			title: `Delete “${promptName(prompt)}”?`,
			message: 'The deliverables it produced are kept.',
			confirmLabel: 'Delete',
			danger: true
		});
		if (!ok) return;
		if ((await attempt(() => api.deletePrompt(project, thread, prompt.id))) !== undefined) await refresh();
	}
</script>

<div class="card prompt-card" id="pr-{prompt.id}">
	<div class="meta">
		<span class="card-kind">Prompt</span>
		<span class="time">{time(prompt.createdAt)}</span>
		{#if prompt.templateName}<span class="badge accent">{prompt.templateName}</span>{/if}
		{#if prompt.updatedAt !== prompt.createdAt}<span class="file">edited</span>{/if}
	</div>
	<div class="card-title">{promptName(prompt)}</div>
	{#if prompt.body.trim()}
		<div class="pc-body" class:clamped={onDeliverable && long && !expanded}><Markdown html={prompt.html} /></div>
		{#if onDeliverable && long}<button class="link accent" onclick={() => (expanded = !expanded)}>{expanded ? 'Show less' : 'Show all'}</button>{/if}
	{/if}
	{#if prompt.base.trim()}
		<details class="pc-base">
			<summary><span class="disclosure"><ChevronRight size={13} /></span>Base prompt</summary>
			<Markdown html={prompt.baseHtml} />
		</details>
	{/if}

	{#if onDeliverable}
		<div class="field">
			<span>Feedback it should resolve</span>
			<div class="chips">
				{#each covered as f (f.id)}
					<span class="chip" class:plain={f.status === 'resolved'}>
						<button class="chip-go" title="Show the feedback" onclick={() => onfeedback?.(f.id)}>
							{f.givenOn ? `v${f.givenOn}` : 'live'} · {f.quote ?? f.body}
						</button>
						<button class="x" aria-label="Take this feedback out of the prompt" onclick={() => detach(f.id)}><X size={12} /></button>
					</span>
				{:else}
					<span class="hint">None yet</span>
				{/each}
			</div>
		</div>
	{:else}
		<div class="field">
			<span>Deliverables</span>
			<div class="chips">
				{#each prompt.deliverables as d (d.path)}
					<span class="chip" class:plain={!d.exists}>
						{#if d.exists}
							<a href={deliverableUrl(project, d.path)} style="color:inherit;text-decoration:none">{baseName(d.path)}</a>
						{:else}
							<span title="This file is no longer in the project">{baseName(d.path)} (missing)</span>
						{/if}
					</span>
				{:else}
					<span class="hint">None yet</span>
				{/each}
			</div>
		</div>
	{/if}

	<div class="actions">
		{#if onDeliverable}<button class="link accent" onclick={() => onaddfeedback?.()}>Add feedback…</button>{/if}
		<button class="link" onclick={() => onedit(prompt)}>Edit</button>
		<Menu class="link more" label="More about this prompt" items={[{ id: 'delete', label: 'Delete prompt', icon: Trash, danger: true }]} onpick={() => remove()} align="end">
			<Ellipsis size={16} />
		</Menu>
	</div>
	<ClaudeRunPanel {project} {thread} {prompt} />
</div>
