<script module lang="ts">
	import type { DotMethod, DotStrategy } from '$lib/types';

	export const STRATEGIES: { id: DotStrategy; label: string; motto: string }[] = [
		{ id: 'library', label: 'Library', motto: 'Standing on the shoulders of giants' },
		{ id: 'field', label: 'Field', motto: 'Understand your users' },
		{ id: 'lab', label: 'Lab', motto: 'To measure is to know' },
		{ id: 'showroom', label: 'Showroom', motto: 'Know & show your contribution' },
		{ id: 'workshop', label: 'Workshop', motto: 'Seek variation and improvement' },
		{ id: 'extra', label: 'Extra', motto: 'Another method that fits better' }
	];

	export const strategyLabel = (id: DotStrategy | null) => STRATEGIES.find((s) => s.id === id)?.label ?? '';

	// The cards don't change while the app runs: read once.
	let cards: Promise<DotMethod[]> | null = null;
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import Dropdown from '$lib/components/Dropdown.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { attempt } from '$lib/ui.svelte';
	import type { PlannedMethod } from '$lib/types';

	/**
	 * The DOT framework of a document: which research methods it is made with, and why. They are kept in the document's
	 * frontmatter; `onsave` gets the document's new text, to save as typing is saved.
	 */
	let { body, onsave, onclose }: { body: string; onsave: (body: string) => void; onclose: () => void } = $props();

	let catalog = $state<DotMethod[]>([]);
	let plan = $state<PlannedMethod[]>([]);
	let loaded = $state(false);
	let busy = $state(false);

	$effect(() => {
		cards ??= api.dotMethods().catch((e) => {
			cards = null;
			throw e;
		});
		const text = untrack(() => body);
		void attempt(async () => {
			const [all, current] = await Promise.all([cards!, api.dotPlan(text)]);
			catalog = all;
			plan = current;
			loaded = true;
		});
	});

	const cardOf = (name: string) => catalog.find((c) => c.name.toLowerCase() === name.trim().toLowerCase());
	const chosen = $derived(new Set(plan.map((p) => p.method.trim().toLowerCase())));

	// Picking a card adds it; the picker is empty again right after.
	let adding = $state('');
	const options = $derived(
		catalog.map((c) => ({
			value: c.name,
			label: c.name,
			group: strategyLabel(c.strategy),
			hint: c.why,
			disabled: chosen.has(c.name.toLowerCase())
		}))
	);
	$effect(() => {
		const name = adding;
		if (!name) return;
		const card = cardOf(name);
		if (card && !chosen.has(name.toLowerCase())) plan.push({ method: card.name, strategy: card.strategy, why: '' });
		adding = '';
	});

	// A method that is not one of the cards.
	let ownName = $state('');
	let ownStrategy = $state<string>('workshop');
	const strategyOptions = STRATEGIES.map((s) => ({ value: s.id, label: s.label, hint: s.motto }));
	function addOwn() {
		const name = ownName.trim();
		if (!name || chosen.has(name.toLowerCase())) return;
		const card = cardOf(name);
		plan.push({ method: card?.name ?? name, strategy: card?.strategy ?? (ownStrategy as DotStrategy), why: '' });
		ownName = '';
	}

	function move(i: number, by: number) {
		const j = i + by;
		if (j < 0 || j >= plan.length) return;
		[plan[i], plan[j]] = [plan[j], plan[i]];
	}

	async function save() {
		busy = true;
		const next = await attempt(() => api.setDotPlan(body, $state.snapshot(plan)));
		busy = false;
		if (next === undefined) return;
		onsave(next);
		onclose();
	}

	const hasTable = $derived(/^[ \t]*!dotmethod[ \t]*$/im.test(body));
</script>

<Modal title="Research methods (DOT framework)" wide {onclose}>
	<p class="hint" style="margin:0">
		Which methods this document is researched with, and why. Write <code>!dotmethod</code> on a line of its own to show them as a table{hasTable
			? ''
			: '; this document has none yet'}.
	</p>

	{#if loaded}
		{#if plan.length}
			<ol class="dot-chosen">
				{#each plan as p, i (p.method)}
					{@const card = cardOf(p.method)}
					<li>
						<div class="dot-chosen-head">
							{#if card}
								<span class="dot-strategy dot-{p.strategy}">{strategyLabel(p.strategy)}</span>
							{:else}
								<Dropdown bind:value={() => p.strategy ?? 'extra', (v) => (p.strategy = v as DotStrategy)} options={strategyOptions} label="Strategy of {p.method}" />
							{/if}
							<strong>{p.method}</strong>
							<span class="spacer-h"></span>
							<button class="btn ghost small" onclick={() => move(i, -1)} disabled={i === 0} aria-label="Move {p.method} up">↑</button>
							<button class="btn ghost small" onclick={() => move(i, 1)} disabled={i === plan.length - 1} aria-label="Move {p.method} down">↓</button>
							<button class="btn ghost small icon-only" onclick={() => plan.splice(i, 1)} aria-label="Remove {p.method}"><X size={14} /></button>
						</div>
						{#if card}<span class="hint">{card.why}</span>{/if}
						<textarea bind:value={p.why} rows="2" placeholder="Why this document uses it (optional; shown in the table once written)"></textarea>
					</li>
				{/each}
			</ol>
		{:else}
			<p class="dot-none">No methods yet. Pick the cards that fit below.</p>
		{/if}

		<div class="field">
			<span>Add a method card</span>
			<Dropdown bind:value={adding} {options} label="Method card" placeholder="Choose from the 56 cards…" full size="field" />
		</div>
		<div class="field">
			<span>Or a method of your own</span>
			<div class="dot-own">
				<input bind:value={ownName} placeholder="e.g. Diary study" onkeydown={(e) => e.key === 'Enter' && !e.isComposing && addOwn()} />
				<Dropdown bind:value={ownStrategy} options={strategyOptions} label="Strategy of your own method" size="field" />
				<button class="btn" onclick={addOwn} disabled={!ownName.trim()}>Add</button>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		<span class="spacer-h"></span>
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={save} disabled={busy || !loaded}>Save</button>
	{/snippet}
</Modal>

<style>
	.dot-chosen {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.dot-chosen li {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 10px 12px;
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--card);
	}
	.dot-chosen-head {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.dot-chosen textarea {
		font: inherit;
		color: var(--ink);
		background: var(--bg);
		border: 1px solid var(--line);
		border-radius: 8px;
		padding: 6px 8px;
		resize: vertical;
	}
	.dot-chosen textarea:focus {
		outline: none;
		border-color: var(--accent);
	}
	.dot-none {
		margin: 0;
		color: var(--muted);
	}
	.dot-own {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.dot-own input {
		flex: 1;
		min-width: 160px;
	}
</style>
