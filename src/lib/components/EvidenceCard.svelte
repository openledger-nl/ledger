<script lang="ts">
	// An evidence item in its thread: what is handed in to Portflow (one or more files, versioned together), the
	// learning outcomes it counts for, and its versions. Version N in Portflow is vN.0 of every file in it. A version
	// that came from Ledger opens here; one uploaded in Portflow is not copied, and opens in Portflow (in the browser).
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import { deliverableUrl } from '$lib/actions';
	import { date, plural } from '$lib/format';
	import { openInPortflow } from '$lib/portflow.svelte';
	import type { Evidence } from '$lib/types';
	import Markdown from './Markdown.svelte';

	let { evidence }: { evidence: Evidence } = $props();

	let open = $state(false);
	const latest = $derived(evidence.versions[0] ?? null);
	const KIND: Record<string, string> = { file: 'files', free_text: 'text', url: 'link', video: 'video', lms_submission: 'Canvas submission', badge: 'badge' };
	const fileName = (path: string) => path.split('/').pop() ?? path;
	const open_ = (version: number | null) => openInPortflow(`“${evidence.name}”${version ? `, version ${version}` : ''}`);
</script>

{#snippet files(v: Evidence['versions'][number])}
	<div class="chips">
		{#each v.files as f (f.name)}
			{#if f.path}
				<a class="chip" href={deliverableUrl(evidence.project, f.path)} title="In Ledger">{fileName(f.path)}</a>
			{:else}
				<button class="chip plain" title="Uploaded in Portflow: opens there" onclick={() => open_(v.number)}>
					{f.name}<ExternalLink size={11} />
				</button>
			{/if}
		{/each}
	</div>
{/snippet}

<div class="card evidence-card">
	<div class="meta">
		<span class="card-kind">Evidence</span>
		<strong>{evidence.name}</strong>
		{#if evidence.portflowId}<span class="badge accent">Portflow</span>{:else}<span class="badge draft">not in Portflow yet</span>{/if}
		{#if latest}<span class="badge published">v{latest.number}</span>{/if}
		<span class="hint">{KIND[evidence.kind] ?? evidence.kind}</span>
		<span class="spacer-h"></span>
		{#if evidence.feedbackCount}
			<span class="hint">{plural(evidence.feedbackCount, 'piece', 'pieces')} of feedback{evidence.openFeedback ? `, ${evidence.openFeedback} open` : ''}</span>
		{/if}
		{#if evidence.portflowUrl}
			<button class="btn small ghost" onclick={() => open_(null)}><ExternalLink size={13} />Open in Portflow</button>
		{/if}
	</div>

	{#if latest && latest.files.length}
		{@render files(latest)}
	{:else if evidence.files.length}
		<div class="chips">
			{#each evidence.files as f (f.path)}<a class="chip" href={deliverableUrl(evidence.project, f.path)}>{f.name}</a>{/each}
		</div>
	{/if}

	{#if evidence.outcomes.length}
		<div class="outcomes hint">
			Counts for:
			{#each evidence.outcomes as o, i (o.id)}<a href="/gamma">{o.name}</a>{i < evidence.outcomes.length - 1 ? ', ' : ''}{/each}
		</div>
	{/if}

	{#if evidence.versions.length}
		<button class="link accent" onclick={() => (open = !open)}>{open ? 'Hide versions' : `${plural(evidence.versions.length, 'version')}`}</button>
		{#if open}
			<div class="versions">
				{#each evidence.versions as v (v.number)}
					<div class="version">
						<div class="meta">
							<strong>v{v.number}</strong>
							<span class="time">{date(v.createdAt)}</span>
							{#if v.kind !== 'file'}<span class="hint">{KIND[v.kind] ?? v.kind}</span>{/if}
							{#if v.files.length && !v.inLedger}<span class="hint">uploaded in Portflow</span>{/if}
							{#if v.portflowUrl}
								<button class="link accent" onclick={() => open_(v.number)}>Open in Portflow</button>
							{/if}
						</div>
						{#if v.files.length}{@render files(v)}{/if}
						{#if v.descriptionHtml}<Markdown html={v.descriptionHtml} />{/if}
					</div>
				{/each}
			</div>
		{/if}
	{/if}
</div>

<style>
	.evidence-card {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.meta {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.versions {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding-left: 12px;
		border-left: 2px solid var(--line);
	}
	.link {
		align-self: flex-start;
	}
</style>
