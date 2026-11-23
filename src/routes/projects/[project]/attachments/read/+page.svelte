<script lang="ts">
	// A markdown file of the project that isn't one of Ledger's own (a note in attachments/, say), read in Ledger: rendered
	// as every other text in the app, its [[links]] and pictures working. It is only read here; to change it, open it in
	// its own program.
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import Markdown from '$lib/components/Markdown.svelte';
	import { api } from '$lib/api';
	import { attempt } from '$lib/ui.svelte';
	import { dirOf } from '$lib/vaultImages';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project.slug);
	const name = $derived(data.path.slice(data.path.lastIndexOf('/') + 1));
	/** The folders on the way to it, each a step back on the Attachments tab. */
	const crumbs = $derived(
		data.path
			.split('/')
			.slice(0, -1)
			.map((part, i, all) => ({ part, path: all.slice(0, i + 1).join('/') }))
	);
	/** A folder on the way, on the Attachments tab (`attachments/hans` is its folder `hans`); null outside attachments/. */
	const back = (path: string) =>
		path === 'attachments' || path.startsWith('attachments/')
			? `/projects/${project}/attachments?dir=${encodeURIComponent(path.slice('attachments/'.length))}`
			: null;
	const open = (reveal: boolean) => attempt(() => api.openProjectFile(project, data.path, reveal));
</script>

<svelte:head><title>{name} | {data.project.name} | Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner">
		<div class="panel reader">
			<div class="reader-head">
				<nav class="reader-crumbs" aria-label="Where it is">
					{#each crumbs as crumb (crumb.path)}
						{@const to = back(crumb.path)}
						{#if to}<a href={to}>{crumb.part}</a>{:else}<span>{crumb.part}</span>{/if}
						<ChevronRight size={14} />
					{/each}
					<span class="here">{name}</span>
				</nav>
				<span class="reader-actions">
					<button class="btn small ghost" onclick={() => open(true)} title="Show it in its folder"><FolderOpen size={14} />Show in folder</button>
					<button class="btn small" onclick={() => open(false)} title="Open it in its own program, to change it"><ExternalLink size={14} />Open</button>
				</span>
			</div>
			{#if data.meta.trim()}
				<details class="reader-meta">
					<summary>Properties</summary>
					<pre>{data.meta}</pre>
				</details>
			{/if}
			<div class="reader-body">
				<Markdown html={data.html} images={{ project, dir: dirOf(data.path) }} />
			</div>
		</div>
	</div>
</div>

<style>
	.reader-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		flex-wrap: wrap;
		padding-bottom: 12px;
		border-bottom: 1px solid var(--line);
	}
	.reader-crumbs {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-wrap: wrap;
		color: var(--faint);
		font-size: 14px;
		min-width: 0;
	}
	.reader-crumbs a {
		color: var(--muted);
		text-decoration: none;
	}
	.reader-crumbs a:hover {
		color: var(--accent);
	}
	.reader-crumbs .here {
		color: var(--ink);
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.reader-actions {
		display: flex;
		gap: 8px;
	}
	.reader-meta {
		font-size: 13px;
		color: var(--muted);
	}
	.reader-meta summary {
		cursor: pointer;
	}
	.reader-meta pre {
		margin: 8px 0 0;
		padding: 10px 12px;
		border-radius: 8px;
		background: var(--card-2);
		font-family: var(--font-mono);
		font-size: 12px;
		white-space: pre-wrap;
	}
	.reader-body {
		max-width: 760px;
	}
</style>
