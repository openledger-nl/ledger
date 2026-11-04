<script lang="ts">
	// A project's Portflow side, on its overview: the collection linked for each semester (its description and
	// outcomes, as Portflow has them), and its evidence items, each to be given a place in one of the threads.
	import { portflowApi } from '$lib/api';
	import { threadUrl } from '$lib/actions';
	import { plural } from '$lib/format';
	import { openInPortflow, portflow } from '$lib/portflow.svelte';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { ProjectPortflow, ThreadSummary } from '$lib/types';
	import Markdown from './Markdown.svelte';

	let { project, threads, version }: { project: string; threads: ThreadSummary[]; version: unknown } = $props();

	let data = $state<ProjectPortflow | null>(null);
	let busy = $state<string | null>(null);

	$effect(() => {
		const slug = project;
		void version;
		void portflow.status?.lastSyncAt;
		if (!portflow.enabled) {
			data = null;
			return;
		}
		let stale = false;
		portflowApi.project(slug).then(
			(d) => !stale && (data = d),
			() => !stale && (data = null)
		);
		return () => (stale = true);
	});

	const unplaced = $derived((data?.evidence ?? []).filter((e) => !e.thread));

	async function place(evidence: string, name: string, choice: string) {
		if (!choice) return;
		busy = evidence;
		const isNew = choice === '__new__';
		const items = await attempt(() => portflowApi.placeEvidence(project, evidence, isNew ? null : choice, isNew ? name : null));
		busy = null;
		if (!items) return;
		data = data ? { ...data, evidence: items } : data;
		toast('Placed. Its feedback comes in with the next sync.');
		await refresh();
	}
</script>

{#if portflow.enabled && data && (data.collections.length || data.evidence.length)}
	<section class="panel">
		<div class="header-top">
			<div>
				<h2>Portflow</h2>
				<div class="sub">The collection for each semester and its evidence, as your portfolio has them.</div>
			</div>
		</div>

		{#each data.collections as c (c.semester)}
			<div class="collection">
				<div class="meta">
					<strong>{c.name || `Collection ${c.collection}`}</strong>
					{#if c.subtitle}<span class="hint">{c.subtitle}</span>{/if}
					<span class="badge">{c.semesterName ?? c.semester}</span>
				</div>
				{#if c.descriptionHtml}<Markdown html={c.descriptionHtml} />{/if}
				{#if c.outcomes.length}
					<div class="chips">
						{#each c.outcomes as o (o.id)}<a class="chip" href="/gamma">{o.name}</a>{/each}
					</div>
				{/if}
			</div>
		{/each}

		{#if unplaced.length}
			<h3 style="font-size:14px">Evidence to place</h3>
			<div class="hint">Put each evidence item in the thread where you work on it: its feedback from Portflow lands there.</div>
			{#each unplaced as e (e.slug)}
				<div class="kv" style="grid-template-columns: 1fr auto">
					<div>
						<strong>{e.name}</strong>
						<span class="hint">· {plural(e.versions.length, 'version')}</span>
						{#if e.portflowId}<button class="link accent" onclick={() => openInPortflow(`“${e.name}”`)}>Open in Portflow</button>{/if}
					</div>
					<select disabled={busy !== null} onchange={(ev) => place(e.slug, e.name, ev.currentTarget.value)}>
						<option value="">Put in a thread…</option>
						<option value="__new__">A new thread “{e.name}”</option>
						{#each threads as t (t.slug)}<option value={t.slug}>{t.title}</option>{/each}
					</select>
				</div>
			{/each}
		{/if}

		{#if data.evidence.some((e) => e.thread)}
			<h3 style="font-size:14px">Evidence</h3>
			<div class="chips">
				{#each data.evidence.filter((e) => e.thread) as e (e.slug)}
					<a class="chip" href={threadUrl(project, e.thread ?? '')} title={e.threadTitle ?? ''}
						>{e.name}{e.versions[0] ? ` · v${e.versions[0].number}` : ''}{e.openFeedback ? ` · ${e.openFeedback} open` : ''}</a
					>
				{/each}
			</div>
		{/if}
	</section>
{/if}

<style>
	.collection {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding-bottom: 10px;
		border-bottom: 1px solid var(--line);
		margin-bottom: 10px;
	}
	.meta {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
</style>
