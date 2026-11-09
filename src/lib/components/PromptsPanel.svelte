<script lang="ts">
	import { api } from '$lib/api';
	import { promptName, threadUrl } from '$lib/actions';
	import { orderPrompts } from '$lib/order';
	import { listOrder } from '$lib/order.svelte';
	import { dayKey, date } from '$lib/format';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { Deliverable, Prompt, ThreadSummary } from '$lib/types';
	import FeedbackPicker from './FeedbackPicker.svelte';
	import PromptCard from './PromptCard.svelte';
	import PromptModal from './PromptModal.svelte';

	// The prompts about this deliverable: made here for it, or from its feedback, run with Claude Code, and kept in their
	// threads. A prompt belongs to the deliverable, not to a version of it. New ones go to the page's thread (`thread`).
	let {
		project,
		doc,
		prompts,
		threads,
		thread,
		threadTitle,
		onfeedback
	}: {
		project: string;
		doc: Deliverable;
		prompts: Prompt[];
		threads: ThreadSummary[];
		thread: string;
		threadTitle: string;
		/** Show a piece of feedback (by id) in the Feedback window. */
		onfeedback: (id: string) => void;
	} = $props();

	const titleOf = (slug: string) => threads.find((t) => t.slug === slug)?.title;

	/** The prompts by thread, the threads in the order set under Settings (newest activity first by default). */
	const groups = $derived.by(() => {
		const by = new Map<string, Prompt[]>();
		for (const p of prompts) by.set(p.threadSlug, [...(by.get(p.threadSlug) ?? []), p]);
		const list = [...by].map(([slug, items]) => ({ slug, title: titleOf(slug) ?? slug, items: orderPrompts(items, listOrder.order) }));
		const newest = (g: { items: Prompt[] }) => g.items.map((p) => p.createdAt).reduce((a, b) => (a > b ? a : b));
		return list.sort((a, b) => (listOrder.order === 'desc' ? newest(b).localeCompare(newest(a)) : newest(a).localeCompare(newest(b))));
	});

	/** The dialog: a new prompt for this deliverable, or the one being edited. */
	let dialog = $state<{ prompt: Prompt | null } | undefined>();
	/** The prompt that open feedback is being added to. */
	let adding = $state<Prompt | undefined>();

	const open = $derived(doc.feedback.filter((f) => f.status === 'open'));
	const addable = $derived(adding ? open.filter((f) => !adding?.feedback.includes(f.id)) : []);

	async function addTo(prompt: Prompt, picked: typeof doc.feedback) {
		const done = await attempt(() => api.attachPromptFeedback(project, prompt.threadSlug, prompt.id, picked.map((f) => f.id)));
		if (!done) return;
		toast(`Added ${picked.length} ${picked.length === 1 ? 'piece' : 'pieces'} of feedback to the prompt`);
		await refresh();
	}

	const span = (items: Prompt[]) => {
		const first = items.map((p) => p.createdAt).reduce((a, b) => (a < b ? a : b));
		const last = items.map((p) => p.createdAt).reduce((a, b) => (a > b ? a : b));
		return dayKey(first) === dayKey(last) ? date(first) : `${date(first)} – ${date(last)}`;
	};
</script>

<div class="pv-line" style="justify-content:space-between">
	<button class="btn small primary" disabled={!thread} onclick={() => (dialog = { prompt: null })}>New prompt…</button>
	{#if thread}<span class="hint">In the thread “{threadTitle}”</span>{:else}<span class="hint">Create a thread first</span>{/if}
</div>

{#each groups as group (group.slug)}
	<section class="pv-group">
		<div class="pv-thread">
			<a href={threadUrl(project, group.slug)} title="Open the thread">{group.title}</a>
			<span class="pv-meta">{span(group.items)}</span>
		</div>
		<div class="fb-list">
			{#each group.items as p (p.id)}
				<PromptCard
					{project}
					thread={p.threadSlug}
					prompt={p}
					onDeliverable
					feedback={doc.feedback}
					{onfeedback}
					onaddfeedback={() => (adding = p)}
					onedit={(prompt) => (dialog = { prompt })}
				/>
			{/each}
		</div>
	</section>
{:else}
	<div class="pv-hint">
		No prompts about this file yet. A prompt asks Claude Code, or whoever works on it, for changes to the deliverable; it can be written from the feedback
		on it. Select feedback in the Feedback window to make one.
	</div>
{/each}

{#if dialog}
	<PromptModal {project} thread={dialog.prompt?.threadSlug ?? thread} prompt={dialog.prompt} deliverable={doc.basePath} onclose={() => (dialog = undefined)} />
{/if}

{#if adding}
	{@const target = adding}
	<FeedbackPicker
		title="Add feedback to “{promptName(target)}”"
		items={addable}
		threadTitle={titleOf}
		onpick={(picked) => addTo(target, picked)}
		onclose={() => (adding = undefined)}
	/>
{/if}
