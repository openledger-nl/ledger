<script lang="ts">
	// An evidence description, in the shape teachers expect in Portflow: What? / Goal? / Why it contributes?. Either an
	// evidence item's (`evidence`): kept as the body of the item's file in Ledger; with the "Evidence description →
	// evidence item" write on, the next round sends it to the item in Portflow. Or a deliverable's that isn't in Portflow
	// (`deliverable`): kept in Ledger, ready for when it is handed in. A description written freely (in Portflow, or
	// before) stays as it is until it is split into the three parts.
	import Pencil from '@lucide/svelte/icons/pencil';
	import { api, portflowApi } from '$lib/api';
	import { portflow } from '$lib/portflow.svelte';
	import type { DeliverableDescription, Evidence, EvidenceParts } from '$lib/types';
	import { attempt } from '$lib/ui.svelte';
	import Markdown from './Markdown.svelte';

	let {
		evidence,
		deliverable,
		onsaved,
		ondescribed
	}: {
		evidence?: Evidence;
		deliverable?: DeliverableDescription;
		onsaved?: (items: Evidence[]) => void;
		ondescribed?: (described: DeliverableDescription) => void;
	} = $props();

	/** What is described, whichever of the two it is. */
	const shown = $derived(evidence ?? deliverable ?? { description: '', descriptionHtml: '', descriptionParts: null });

	let editing = $state(false);
	let busy = $state(false);
	let parts = $state<EvidenceParts>({ what: '', goal: '', why: '' });
	/** Editing free text rather than the three parts. */
	let free = $state<string | null>(null);

	const empty = $derived(!shown.description.trim());

	function start() {
		if (shown.descriptionParts || empty) {
			parts = { ...(shown.descriptionParts ?? { what: '', goal: '', why: '' }) };
			free = null;
		} else {
			free = shown.description;
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
		const asParts = free === null ? parts : null;
		if (evidence) {
			const item = evidence;
			const items = await attempt(() => portflowApi.saveEvidenceDescription(item.project, item.slug, asParts, free));
			if (items) {
				editing = false;
				onsaved?.(items);
			}
		} else if (deliverable) {
			const d = deliverable;
			const described = await attempt(() => api.saveDeliverableDescription(d.project, d.path, asParts, free));
			if (described) {
				editing = false;
				ondescribed?.(described);
			}
		}
		busy = false;
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
				{#if !evidence}
					Kept in Ledger, ready for when you hand it in
				{:else if !evidence.portflowId}
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
			<span class="hint">
				{evidence ? 'No evidence description yet' : 'Evidence description, for when you hand it in'}: What? / Goal? / Why it contributes?
			</span>
			<button class="link accent" onclick={start}>{evidence ? 'Describe it' : 'Add evidence description'}</button>
		</div>
	{:else}
		<div class="meta">
			<!-- A description in the shape starts with its own bold title; free text gets one here. -->
			{#if !shown.descriptionParts}<span class="card-kind">Evidence description</span>{/if}
			<span class="spacer-h"></span>
			<button class="btn small ghost" onclick={start} title="Edit the description"><Pencil size={13} />Edit</button>
		</div>
		<Markdown html={shown.descriptionHtml} />
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
