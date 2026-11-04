<script lang="ts">
	import type { Feedback } from '$lib/types';
	import Modal from './Modal.svelte';

	/**
	 * Choose pieces of feedback to add to a prompt: the open ones that are not in it yet. Several at a time; `onpick` hears
	 * the ones chosen, and the dialog closes itself.
	 */
	let {
		title,
		items,
		threadTitle,
		onpick,
		onclose
	}: {
		title: string;
		items: Feedback[];
		threadTitle: (slug: string) => string | undefined;
		onpick: (picked: Feedback[]) => void;
		onclose: () => void;
	} = $props();

	let picked = $state<string[]>([]);

	const toggle = (id: string) => (picked = picked.includes(id) ? picked.filter((p) => p !== id) : [...picked, id]);
	const all = $derived(items.length > 0 && picked.length === items.length);

	function add() {
		onpick(items.filter((f) => picked.includes(f.id)));
		onclose();
	}
</script>

<Modal {title} {onclose}>
	{#if items.length === 0}
		<p class="hint" style="margin:0">All the open feedback on this deliverable is in this prompt already, or there is none.</p>
	{:else}
		<label class="check">
			<input type="checkbox" checked={all} onchange={() => (picked = all ? [] : items.map((f) => f.id))} />
			<b>All open feedback</b> ({items.length})
		</label>
		<div class="pick-list">
			{#each items as f (f.id)}
				<label class="pick-row">
					<input type="checkbox" checked={picked.includes(f.id)} onchange={() => toggle(f.id)} />
					<span class="pick-text">
						<span class="pick-head">{f.givenOn ? `v${f.givenOn}` : 'Live file'} · {threadTitle(f.threadSlug) ?? f.threadSlug}</span>
						{#if f.quote}<span class="pick-quote">“{f.quote}”</span>{/if}
						<span class="pick-body">{f.body}</span>
					</span>
				</label>
			{/each}
		</div>
	{/if}

	{#snippet footer()}
		<span class="spacer-h"></span>
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={add} disabled={picked.length === 0}>Add{picked.length ? ` ${picked.length}` : ''} to the prompt</button>
	{/snippet}
</Modal>
