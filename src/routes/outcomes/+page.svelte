<script lang="ts">
	import { page } from '$app/state';
	import { portflowApi } from '$lib/api';
	import { threadUrl, projectUrl } from '$lib/actions';
	import Markdown from '$lib/components/Markdown.svelte';
	import { plural } from '$lib/format';
	import { portflow } from '$lib/portflow.svelte';
	import type { EvidenceRef, Outcome } from '$lib/types';

	let outcomes = $state<Outcome[] | null>(null);
	let error = $state<string | null>(null);
	let set = $state('');
	// Loaded again whenever Portflow is switched on, a sync finishes or the vault changes (the page data is invalidated).
	$effect(() => {
		void page.data;
		void portflow.status?.lastSyncAt;
		if (!portflow.enabled) {
			outcomes = null;
			return;
		}
		let stale = false;
		portflowApi.outcomes().then(
			(all) => {
				if (stale) return;
				outcomes = all;
				error = null;
			},
			(e) => !stale && (error = e instanceof Error ? e.message : String(e))
		);
		return () => (stale = true);
	});

	const sets = $derived([...new Set((outcomes ?? []).filter((o) => !o.removed).map((o) => o.goalSet).filter(Boolean))]);
	const shown = $derived((outcomes ?? []).filter((o) => !set || o.goalSet === set));
	const evidenceHref = (e: EvidenceRef) => (e.thread ? threadUrl(e.project, e.thread) : projectUrl(e.project));
</script>

<svelte:head><title>Learning outcomes · Ledger</title></svelte:head>

<div class="header-wrap flat">
	<div class="header-top">
		<div>
			<h1>Learning outcomes</h1>
			<div class="sub">From Portflow, as your portfolio has them: never typed into Ledger. Each shows the evidence that counts for it, and who gave feedback on that.</div>
		</div>
		{#if sets.length > 1}
			<label class="field">
				Goal set
				<select bind:value={set}>
					<option value="">All</option>
					{#each sets as s (s)}<option value={s}>{s}</option>{/each}
				</select>
			</label>
		{/if}
	</div>
</div>

<div class="page">
	<div class="page-inner">
		{#if !portflow.enabled}
			<div class="empty-state">
				<h1>Portflow is off</h1>
				<p>Switch it on under <a href="/settings/sync">Settings → Sync &amp; external → Portflow</a> to see your learning outcomes here.</p>
			</div>
		{:else if error}
			<div class="notice bad">{error}</div>
		{:else if outcomes && outcomes.length === 0}
			<div class="empty-state">
				<h1>No outcomes yet</h1>
				<p>They come in with the first sync after you sign in (Settings → Sync &amp; external → Portflow).</p>
			</div>
		{:else if outcomes}
			<div class="outcomes">
				{#each shown as o (o.id)}
					<section class="card outcome" class:removed={o.removed}>
						<div class="outcome-head">
							<h2>{o.name}</h2>
							{#if o.nickname}<span class="badge">{o.nickname}</span>{/if}
							{#if o.goalSet}<span class="badge type">{o.goalSet}</span>{/if}
							{#if o.removed}<span class="badge draft">no longer in Portflow</span>{/if}
							<span class="spacer-h"></span>
							{#if o.feedbackCount}
								<span class="hint">{plural(o.feedbackCount, 'piece', 'pieces')} of feedback{o.feedbackFrom.length ? ` from ${o.feedbackFrom.join(', ')}` : ''}</span>
							{/if}
						</div>
						{#if o.explanationHtml}
							<details>
								<summary class="hint">What it asks</summary>
								<Markdown html={o.explanationHtml} />
							</details>
						{/if}
						{#if o.evidence.length}
							<div class="chips">
								{#each o.evidence as e (e.project + e.slug)}<a class="chip" href={evidenceHref(e)} title={e.projectName}>{e.name}</a>{/each}
							</div>
						{/if}
						{#each o.subOutcomes as s (s.id)}
							<div class="sub-outcome">
								<div class="sub-head">
									<strong>{s.name}</strong>
									{#if s.evidence.length === 0}<span class="hint">no evidence yet</span>{/if}
									<div class="chips">
										{#each s.evidence as e (e.project + e.slug)}<a class="chip" href={evidenceHref(e)} title={e.projectName}>{e.name}</a>{/each}
									</div>
								</div>
								{#if s.descriptionHtml}
									<details>
										<summary class="hint">Level description</summary>
										<Markdown html={s.descriptionHtml} />
									</details>
								{/if}
							</div>
						{/each}
					</section>
				{/each}
			</div>
		{:else}
			<div class="hint">Loading…</div>
		{/if}
	</div>
</div>

<style>
	.outcomes {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
	.outcome {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.outcome.removed {
		opacity: 0.6;
	}
	.outcome-head {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.outcome-head h2 {
		font-size: 16px;
		margin: 0;
	}
	.sub-outcome {
		border-top: 1px solid var(--line);
		padding-top: 8px;
	}
	.sub-head {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
		font-size: 13px;
	}
	summary {
		cursor: pointer;
	}
</style>
