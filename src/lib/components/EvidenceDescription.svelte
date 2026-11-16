<script lang="ts">
	// An evidence item's description, in the shape teachers expect in Portflow: What? / Goal? / Why it contributes?.
	// Kept as the body of the item's file in Ledger; with the "Evidence description → evidence item" write on, the next
	// round sends it to the item in Portflow. A description written freely (in Portflow, or before) stays as it is
	// until it is split into the three parts.
	import Pencil from '@lucide/svelte/icons/pencil';
	import { portflowApi } from '$lib/api';
	import { portflow } from '$lib/portflow.svelte';
	import type { Evidence, EvidenceParts } from '$lib/types';
	import { attempt } from '$lib/ui.svelte';
	import Markdown from './Markdown.svelte';

	let { evidence, onsaved }: { evidence: Evidence; onsaved?: (items: Evidence[]) => void } = $props();

	let editing = $state(false);
	let busy = $state(false);
	let parts = $state<EvidenceParts>({ what: '', goal: '', why: '' });
	/** Editing free text rather than the three parts. */
	let free = $state<string | null>(null);

	const empty = $derived(!evidence.description.trim());

	function start() {
		if (evidence.descriptionParts || empty) {
			parts = { ...(evidence.descriptionParts ?? { what: '', goal: '', why: '' }) };
			free = null;
		} else {
			free = evidence.description;
		}
		editing = true;
	}

	/** Free text into the shape: all of it becomes What?, to be split by hand. */
	function split() {
		parts = { what: (free ?? '').trim(), goal: '', why: '' };
		free = null;
	}

	async function save() {
		busy = true;
		const items = await attempt(() => portflowApi.saveEvidenceDescription(evidence.project, evidence.slug, free === null ? parts : null, free));
		busy = false;
		if (!items) return;
		editing = false;
		onsaved?.(items);
	}
</script>

<div class="evidence-description">
	{#if editing}
		<div class="hint">
			Teachers expect this shape: what the evidence is, what it is for, and how it shows your learning outcomes (which
			ones, at what level).
		</div>
		{#if free !== null}
			<label class="field">
				Description
				<textarea bind:value={free} rows="5"></textarea>
			</label>
			<button class="link accent" onclick={split}>Split into What? / Goal? / Why it contributes?</button>
		{:else}
			<label class="field">
				What?
				<textarea bind:value={parts.what} rows="3" placeholder="A written plan with the context, the main question and three sub questions, the research approach, the scope…"
				></textarea>
			</label>
			<label class="field">
				Goal?
				<textarea bind:value={parts.goal} rows="2" placeholder="To give my expert, the stakeholder and myself a clear picture of what I research and deliver this semester."
				></textarea>
			</label>
			<label class="field">
				Why it contributes?
				<textarea
					bind:value={parts.why}
					rows="3"
					placeholder="It shows that I analyse the problem and plan my research with the DOT framework before I build, which supports Analysing at level 2."
				></textarea>
			</label>
		{/if}
		<div class="foot">
			<span class="hint">
				{#if !evidence.portflowId}
					Kept in Ledger until the item is in Portflow
				{:else if portflow.evidenceWrite}
					Sent to Portflow with the next sync
				{:else}
					Kept in Ledger · <a href="/settings/sync#portflow">switch on “Evidence description → evidence item”</a> to send it
				{/if}
			</span>
			<span class="spacer-h"></span>
			<button class="btn small ghost" disabled={busy} onclick={() => (editing = false)}>Cancel</button>
			<button class="btn small primary" disabled={busy} onclick={save}>Save</button>
		</div>
	{:else if empty}
		<div class="meta">
			<span class="hint">No evidence description yet: What? / Goal? / Why it contributes?</span>
			<button class="link accent" onclick={start}>Describe it</button>
		</div>
	{:else}
		<div class="meta">
			<!-- A description in the shape starts with its own bold title; free text gets one here. -->
			{#if !evidence.descriptionParts}<span class="card-kind">Evidence description</span>{/if}
			<span class="spacer-h"></span>
			<button class="btn small ghost" onclick={start} title="Edit the description"><Pencil size={13} />Edit</button>
		</div>
		<Markdown html={evidence.descriptionHtml} />
	{/if}
</div>

<style>
	.evidence-description {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.meta,
	.foot {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.field textarea {
		width: 100%;
	}
	.link {
		align-self: flex-start;
	}
</style>
