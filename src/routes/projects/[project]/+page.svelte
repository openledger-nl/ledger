<script lang="ts">
	import BookOpen from '@lucide/svelte/icons/book-open';
	import Plus from '@lucide/svelte/icons/plus';
	import Send from '@lucide/svelte/icons/send';
	import { api, portflowApi } from '$lib/api';
	import { newDeliverableFlow, newThreadFlow } from '$lib/actions';
	import Markdown from '$lib/components/Markdown.svelte';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import ProjectLatest from '$lib/components/ProjectLatest.svelte';
	import ProjectPortflow from '$lib/components/ProjectPortflow.svelte';
	import { openInPortflow, portflow } from '$lib/portflow.svelte';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { ProjectPortflow as ProjectPortflowData } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project);
	const summary = $derived(data.projects.find((p) => p.slug === project.slug));
	// The guide is the project's README.md: pictures in it are looked for from the project's folder.
	const images = $derived({ project: project.slug, dir: '' });

	// A project linked to a Portflow collection is part of a portfolio, and there the reading guide is what an assessor
	// reads first: it comes first on the overview, says where it goes, and can be handed in.
	let linked = $state<ProjectPortflowData | null>(null);
	$effect(() => {
		const slug = project.slug;
		void data;
		void portflow.status?.lastSyncAt;
		if (!portflow.enabled) {
			linked = null;
			return;
		}
		let stale = false;
		portflowApi.project(slug).then(
			(d) => !stale && (linked = d.collections.length ? d : null),
			() => !stale && (linked = null)
		);
		return () => (stale = true);
	});
	const target = $derived(linked?.collections.find((c) => c.collection === linked?.guideCollection) ?? null);
	const empty = $derived(!project.readme.trim());

	let editing = $state(false);
	let draft = $state('');
	let saving = $state(false);

	function edit() {
		draft = project.readme;
		editing = true;
	}

	async function save() {
		saving = true;
		const done = await attempt(() => api.saveReadme(project.slug, draft));
		saving = false;
		if (!done) return;
		editing = false;
		await refresh();
	}

	/** Hands the guide in by hand: Ledger can't create an evidence item in Portflow yet (see the Portflow plan), so it
	 *  copies the guide, rendered and as markdown, and opens Portflow where it is added as a text. */
	async function handIn() {
		const plain = project.readme;
		try {
			await navigator.clipboard.write([
				new ClipboardItem({
					'text/html': new Blob([project.readmeHtml], { type: 'text/html' }),
					'text/plain': new Blob([plain], { type: 'text/plain' })
				})
			]);
		} catch {
			try {
				await navigator.clipboard.writeText(plain);
			} catch {
				toast('Could not copy the reading guide', true);
				return;
			}
		}
		await openInPortflow(`“${target?.name || 'your collection'}”, add an evidence item and paste the reading guide (it is copied) as its text`);
	}
</script>

{#snippet guide()}
	<section class="panel" class:guide-first={!!linked}>
		<div class="header-top">
			<div>
				<h2>Reading guide</h2>
				<div class="sub">
					{#if linked}
						README.md — the first thing an assessor reads in your portfolio: where to start, what matters, how this project works.
					{:else}
						README.md — where to start, what matters, how this project works.
					{/if}
				</div>
			</div>
			{#if !editing}
				<div class="header-actions">
					{#if linked && !empty}
						<button class="btn small ghost" onclick={handIn} title="Copy the guide and open Portflow to add it there"><Send size={14} />Hand in to Portflow…</button>
					{/if}
					<button class="btn small" onclick={edit}>{empty ? 'Write' : 'Edit'}</button>
				</div>
			{/if}
		</div>
		{#if linked}
			<p class="hint guide-where">
				{#if linked.guideWrite && target}
					Saved here and written to the description of <strong>{target.name || 'the linked collection'}</strong> in Portflow with the next sync; Ledger’s
					text wins.
				{:else}
					Kept in Ledger. To put it in the description of {target?.name ? `“${target.name}”` : 'the linked collection'}, switch on
					<a href="/settings/sync#portflow">Reading guide → collection description</a> under Settings → Portflow (off by default).
				{/if}
			</p>
		{/if}
		{#if editing}
			<MarkdownEditor
				bind:value={draft}
				variant="doc"
				minHeight="58vh"
				label="Reading guide"
				autofocus
				spellcheck={false}
				{images}
				onsubmit={save}
				oncancel={() => (editing = false)}
			/>
			<div class="row-end">
				<span class="hint" style="align-self:center">Ctrl+Enter saves</span>
				<button class="btn ghost" onclick={() => (editing = false)}>Cancel</button>
				<button class="btn primary" onclick={save} disabled={saving}>Save</button>
			</div>
		{:else if !empty}
			<Markdown html={project.readmeHtml} {images} />
		{:else if linked}
			<div class="notice guide-missing">
				<BookOpen size={16} />
				<span>This project is part of your Portflow portfolio and has no reading guide yet. The portfolio needs one: write where an assessor should start and what each piece of evidence shows.</span>
				<button class="btn small primary" onclick={edit}>Write the reading guide</button>
			</div>
		{:else}
			<p class="hint" style="margin:0">The guide is empty. Write down what a newcomer should read first.</p>
		{/if}
	</section>
{/snippet}

<div class="page">
	<div class="page-inner">
		{#if linked}{@render guide()}{/if}

		{#if summary}
			<section class="panel" aria-label="Latest">
				<div class="latest-cols">
					<ProjectLatest project={summary} deliverables={data.deliverables} />
				</div>
				<footer class="proj-foot">
					<button class="btn small" onclick={() => newThreadFlow(project.slug)}><Plus size={14} />New thread</button>
					<button class="btn small" onclick={() => newDeliverableFlow(project.slug)}><Plus size={14} />New deliverable</button>
				</footer>
			</section>
		{/if}

		<ProjectPortflow project={project.slug} threads={summary?.threads ?? []} version={data} />

		{#if !linked}{@render guide()}{/if}
	</div>
</div>

<style>
	.guide-first {
		border-color: var(--accent);
	}
	.guide-where {
		margin: 0;
	}
	.guide-missing {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.guide-missing span {
		flex: 1;
		min-width: 220px;
	}
</style>
