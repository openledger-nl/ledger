<script lang="ts">
	// Conflict copies: two machines changed the same file before the cloud storage carried one change to the other, and
	// both versions were kept, the second beside the first under another name (the provider's, or Ledger's own when a
	// file changed on disk while it was about to write it). Neither is read in twice: here one of the two is chosen.
	import Check from '@lucide/svelte/icons/check';
	import { api } from '$lib/api';
	import { dateTime } from '$lib/format';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { ConflictView } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	let busy = $state<string | null>(null);

	const when = (stamp: string | null) => (stamp ? dateTime(stamp) : 'unknown');
	const name = (rel: string) => rel.split('/').pop() ?? rel;

	async function keep(c: ConflictView, which: 'copy' | 'original') {
		busy = c.copy;
		const left = await attempt(() => api.resolveConflict(c.copy, which));
		busy = null;
		if (!left) return;
		toast(which === 'copy' ? `Kept the copy as ${name(c.original)}` : `Kept ${name(c.original)}`);
		await refresh();
	}
</script>

<svelte:head><title>Conflicts | Ledger</title></svelte:head>

<div class="header-wrap flat">
	<div class="header-top">
		<div>
			<h1>Conflicts</h1>
			<div class="sub">
				Two machines changed the same file before your cloud storage carried one change to the other, so both versions were kept. Choose the one to keep;
				the other goes (your cloud storage still has it in its version history).
			</div>
		</div>
	</div>
</div>

<div class="page">
	<div class="page-inner">
		{#if data.conflicts.length === 0}
			<div class="empty-state">
				<span class="cf-done"><Check size={26} /></span>
				<h1>No conflicts</h1>
				<p>Every file in the vault has one version.</p>
			</div>
		{:else}
			{#each data.conflicts as c (c.copy)}
				<section class="panel cf">
					<div class="panel-head">
						<h2 class="mono cf-path">{c.original}</h2>
					</div>
					<div class="cf-sides">
						<div class="cf-side">
							<div class="cf-head">
								<strong>This file</strong><span class="hint">changed {when(c.originalModified)}</span>
							</div>
							{#if c.text}<pre class="cf-text">{c.originalText}</pre>{:else}<p class="hint">Not text: open it from its folder to compare.</p>{/if}
							<button class="btn" disabled={busy !== null} onclick={() => keep(c, 'original')}>Keep this one</button>
						</div>
						<div class="cf-side">
							<div class="cf-head">
								<strong>The copy</strong><span class="hint mono" title={c.copy}>{name(c.copy)}</span><span class="hint">changed {when(c.copyModified)}</span>
							</div>
							{#if c.text}<pre class="cf-text">{c.copyText}</pre>{:else}<p class="hint">Not text: open it from its folder to compare.</p>{/if}
							<button class="btn" disabled={busy !== null} onclick={() => keep(c, 'copy')}>Keep the copy</button>
						</div>
					</div>
				</section>
			{/each}
		{/if}
	</div>
</div>

<style>
	.cf {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.cf-path {
		font-size: 13px;
		word-break: break-all;
	}
	.cf-sides {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 12px;
	}
	@media (max-width: 860px) {
		.cf-sides {
			grid-template-columns: 1fr;
		}
	}
	.cf-side {
		display: flex;
		flex-direction: column;
		gap: 8px;
		align-items: flex-start;
		min-width: 0;
	}
	.cf-head {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: baseline;
	}
	.cf-text {
		width: 100%;
		max-height: 320px;
		overflow: auto;
		margin: 0;
		padding: 10px 12px;
		background: var(--field);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		font-family: var(--font-mono);
		font-size: 12px;
		white-space: pre-wrap;
	}
	.cf-done {
		color: var(--green);
	}
</style>
