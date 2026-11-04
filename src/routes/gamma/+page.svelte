<script module lang="ts">
	import type { Gamma } from '$lib/types';
	// The last matrix per semester ('' for "the current one"): shown at once on the way back, refreshed behind it.
	const shownBefore = new Map<string, Gamma>();
</script>

<script lang="ts">
	import BookOpen from '@lucide/svelte/icons/book-open';
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { page } from '$app/state';
	import { projectUrl, threadUrl } from '$lib/actions';
	import { portflowApi } from '$lib/api';
	import IntendedDevelopmentModal from '$lib/components/IntendedDevelopmentModal.svelte';
	import GammaIcon from '$lib/components/GammaIcon.svelte';
	import GammaMatrix from '$lib/components/GammaMatrix.svelte';
	import { ago, plural } from '$lib/format';
	import { portflow } from '$lib/portflow.svelte';

	let semester = $state('');
	let view = $state<Gamma | null>(shownBefore.get('') ?? null);
	let error = $state<string | null>(null);

	// Gamma reads the vault's mirror of Portflow: nothing goes over the network, so it is there at once. It is read again
	// when another semester is picked, a Portflow sync finishes or the vault changes.
	$effect(() => {
		void page.data;
		void portflow.status?.lastSyncAt;
		const key = semester;
		if (!portflow.enabled) {
			view = null;
			return;
		}
		view = shownBefore.get(key) ?? view;
		let stale = false;
		portflowApi.gamma(key || null).then(
			(fresh) => {
				shownBefore.set(key, fresh);
				if (stale) return;
				view = fresh;
				error = null;
			},
			(e) => !stale && (error = e instanceof Error ? e.message : String(e))
		);
		return () => (stale = true);
	});

	const current = $derived(view?.semesters.find((s) => s.current));
	const shown = $derived(view?.semesters.find((s) => s.id === view?.semester));
	let editing = $state(false);

	// What the semester aims for, in words: "Infrastructure → 2", "PS → 2".
	const aims = $derived(
		view
			? [
					...view.rows.filter((r) => r.target).map((r) => `${r.name} → ${r.target}`),
					...view.skills.filter((s) => s.target).map((s) => `${s.nickname || s.name} → ${s.target}`)
				]
			: []
	);

	function saved(fresh: Gamma) {
		shownBefore.set(semester, fresh);
		view = fresh;
		editing = false;
	}
</script>

<svelte:head><title>Gamma · Ledger</title></svelte:head>

<div class="header-wrap flat">
	<div class="header-top">
		<div>
			<h1 class="gamma-title"><span class="gamma-mark"><GammaIcon size={20} /></span>Gamma</h1>
			<div class="sub">
				Your HBO-i competences for a semester: activities across, layers down. Each cell counts the evidence for it and is shaded by
				the highest level with evidence. Comes from Portflow, linked to the semester through its section.
			</div>
		</div>
		{#if view && view.semesters.length}
			<div class="header-actions">
				<button
					class="btn"
					onclick={() => (editing = true)}
					disabled={!view.semester}
					title={view.semester ? 'What you aim for this semester' : 'Add a semester covering today first (Settings → Semesters)'}
				>
					<Crosshair size={15} />Intended development
				</button>
				<label class="field">
				Semester
				<select bind:value={semester}>
					<option value="">Current{current ? ` (${current.name})` : ''}</option>
					{#each view.semesters as s (s.id)}<option value={s.id}>{s.name}{s.sectionName ? '' : ' · no Portflow section'}</option>{/each}
				</select>
				</label>
			</div>
		{/if}
	</div>
</div>

<div class="page">
	<div class="page-inner">
		{#if !portflow.enabled}
			<div class="empty-state">
				<h1>Portflow is off</h1>
				<p>Gamma reads your learning outcomes from Portflow. Switch it on under <a href="/settings/sync">Settings → Sync &amp; external → Portflow</a>.</p>
			</div>
		{:else if error}
			<div class="notice bad">{error}</div>
		{:else if view && view.cellsTotal + view.skills.length === 0}
			<div class="empty-state">
				<h1>No learning outcomes yet</h1>
				<p>They come in with the first Portflow sync after you sign in (Settings → Sync &amp; external → Portflow).</p>
			</div>
		{:else if view}
			<div class="gamma-head">
				<h2>{view.sectionName ?? 'Newest goal set'}</h2>
				<span class="hint">
					{plural(view.evidenceCount, 'evidence item', 'evidence items')} · {view.cellsWithEvidence} of {view.cellsTotal} cells with evidence{view.goalSet
						? ` · ${view.goalSet}`
						: ''}
				</span>
			</div>
			{#if !view.scoped}
				<div class="notice">
					{view.semester ? 'This semester isn’t linked to a Portflow section yet' : 'No semester covers today'}, so the newest goal set is shown. Say which section a
					semester is under <a href="/settings/semesters">Settings → Semesters</a>.
				</div>
			{/if}
			{#if view.semester}
				<div class="overview">
					<div class="overview-item">
						<span class="overview-label">Intended development</span>
						{#if aims.length}
							<span>{aims.join(' · ')}</span>
							<span class="hint">{view.targetsReached} of {view.targetsTotal} reached</span>
						{:else}
							<button class="link" onclick={() => (editing = true)}>Say what you aim for this semester</button>
						{/if}
					</div>
					<div class="overview-item">
						<span class="overview-label">Learning plan</span>
						{#if view.plan}
							<a href={view.plan.thread ? threadUrl(view.plan.project, view.plan.thread) : projectUrl(view.plan.project)}>{view.plan.name}</a>
							<span class="hint">
								{view.plan.version ? `v${view.plan.version} delivered ${view.plan.deliveredAt ? ago(view.plan.deliveredAt) : ''}` : 'not delivered yet'}
							</span>
						{:else if view.goals.learningPlan}
							<span class="hint">{view.goals.learningPlan} (not found: is its project linked to a collection?)</span>
						{:else}
							<button class="link" onclick={() => (editing = true)}>Pick your learning plan</button>
						{/if}
					</div>
					{#if view.goals.studyGuide}
						<div class="overview-item">
							<span class="overview-label">Study guide</span>
							<button class="link" onclick={() => openUrl(view!.goals.studyGuide!).catch(() => {})}><BookOpen size={13} />Open<ExternalLink size={12} /></button>
						</div>
					{/if}
				</div>
			{/if}
			<GammaMatrix {view} />
		{:else}
			<div class="hint">Loading…</div>
		{/if}
	</div>
</div>

{#if editing && view}
	<IntendedDevelopmentModal {view} semesterName={shown?.name ?? ''} onclose={() => (editing = false)} onsaved={saved} />
{/if}

<style>
	.header-actions {
		display: flex;
		align-items: flex-end;
		gap: 12px;
	}
	.overview {
		display: flex;
		gap: 12px 28px;
		flex-wrap: wrap;
		padding: 12px 14px;
		margin-bottom: 16px;
		border: 1px solid var(--line);
		border-radius: var(--radius);
		background: var(--card);
		font-size: 13px;
	}
	.overview-item {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}
	.overview-label {
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		color: var(--faint);
	}
	.overview .link {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		background: none;
		border: 0;
		padding: 0;
		color: var(--accent);
		cursor: pointer;
		font: inherit;
		text-align: left;
	}
	.gamma-title {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.gamma-mark {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border-radius: 9px;
		background: var(--accent-soft);
		color: var(--accent);
	}
	.gamma-head {
		display: flex;
		align-items: baseline;
		gap: 12px;
		flex-wrap: wrap;
		margin-bottom: 12px;
	}
	.gamma-head h2 {
		margin: 0;
		font-size: 16px;
	}
	.notice {
		margin-bottom: 12px;
	}
</style>
