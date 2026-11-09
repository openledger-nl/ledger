<script lang="ts">
	// Every file uploaded in the project, from all its threads: what is in its attachments/ folder, with where each came
	// from. The files themselves are opened with the system's program; the upload is in its thread, which a click opens.
	import File from '@lucide/svelte/icons/file';
	import FileImage from '$lib/components/FileImage.svelte';
	import { api } from '$lib/api';
	import { formatSize, threadUrl } from '$lib/actions';
	import { ago, dateTime, plural } from '$lib/format';
	import { listOrder } from '$lib/order.svelte';
	import { attempt } from '$lib/ui.svelte';
	import type { Upload } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project.slug);
	const titles = $derived(new Map((data.projects.find((p) => p.slug === project)?.threads ?? []).map((t) => [t.slug, t.title])));
	const threadTitle = (u: Upload) => titles.get(u.threadSlug) ?? u.threadSlug;

	type Kind = 'all' | 'images' | 'files';
	let kind = $state<Kind>('all');
	let filter = $state('');

	const counts = $derived({
		all: data.uploads.length,
		images: data.uploads.filter((u) => u.image).length,
		files: data.uploads.filter((u) => !u.image).length
	});

	// Newest first from the backend; the order chosen in Settings decides.
	const shown = $derived.by(() => {
		const q = filter.trim().toLowerCase();
		const picked = data.uploads.filter(
			(u) =>
				(kind === 'all' || (kind === 'images') === u.image) &&
				(!q || [u.name, u.description, threadTitle(u)].some((text) => text.toLowerCase().includes(q)))
		);
		return listOrder.order === 'asc' ? [...picked].reverse() : picked;
	});

	/** The first line of a description, as plain words. */
	const firstLine = (text: string) => text.trim().split('\n')[0]?.replace(/[#*_`>]/g, '').trim() ?? '';

	const open = (u: Upload, reveal: boolean) => attempt(() => api.openProjectFile(project, u.file, reveal));
	const KINDS: Array<[Kind, string]> = [
		['all', 'All'],
		['images', 'Images'],
		['files', 'Other files']
	];
</script>

<svelte:head><title>Attachments · {data.project.name} · Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner wide">
		<div class="panel">
			<div class="header-top">
				<h2>{plural(data.uploads.length, 'attachment')}</h2>
				<input type="search" class="att-filter" placeholder="Filter…" bind:value={filter} aria-label="Filter by name, description or thread" />
			</div>
			<p class="sub" style="margin:0">
				Every file uploaded in this project’s threads, screenshots included. The files are in the project’s <code>attachments/</code> folder; each
				thread keeps a note of what was uploaded there, and why.
			</p>
			{#if data.uploads.length === 0}
				<div class="hint">Nothing yet. Upload a file or attach a screenshot in a thread, and it shows up here.</div>
			{:else}
				<div class="seg" role="group" aria-label="Which attachments" style="align-self:flex-start">
					{#each KINDS as [value, label] (value)}
						<button class:on={kind === value} aria-pressed={kind === value} onclick={() => (kind = value)}>
							{label}<span class="att-n">{counts[value]}</span>
						</button>
					{/each}
				</div>

				<div class="att-list">
					{#each shown as u (u.id)}
						<div class="att-row" class:missing={!u.exists}>
							<button class="att-thumb" title={u.exists ? `Open ${u.name}` : 'The file is missing'} disabled={!u.exists} onclick={() => open(u, false)}>
								{#if u.exists && u.image}
									<FileImage {project} path={u.file} alt={u.name} cls="att-img" />
								{:else}
									<File size={18} />
								{/if}
							</button>
							<div class="att-main">
								<span class="att-name" title={u.file}>{u.name}</span>
								{#if firstLine(u.description)}<span class="att-desc">{firstLine(u.description)}</span>{/if}
							</div>
							<a class="att-thread" href="{threadUrl(project, u.threadSlug)}#u-{u.id}" title="Open the thread it was uploaded in">{threadTitle(u)}</a>
							<span class="badge type">{formatSize(u.size)}</span>
							<span class="doc-date" title={dateTime(u.uploadedAt)}>{ago(u.uploadedAt)}</span>
							<span class="att-actions">
								{#if u.exists}
									<button class="link accent" onclick={() => open(u, false)}>Open</button>
									<button class="link" onclick={() => open(u, true)}>Show in folder</button>
								{:else}
									<span class="badge draft">File is missing</span>
								{/if}
							</span>
						</div>
					{:else}
						<div class="hint">Nothing matches{filter.trim() ? ` “${filter.trim()}”` : ''}.</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.att-filter {
		padding: 6px 10px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--bg);
	}
	.att-n {
		margin-left: 6px;
		opacity: 0.7;
		font-weight: 500;
	}
	.att-list {
		display: flex;
		flex-direction: column;
	}
	.att-row {
		display: grid;
		grid-template-columns: 48px minmax(0, 1fr) minmax(0, 160px) 80px 100px auto;
		gap: 12px;
		align-items: center;
		padding: 10px 4px;
		border-bottom: 1px solid var(--line);
	}
	.att-row:last-child {
		border-bottom: 0;
	}
	.att-thumb {
		width: 48px;
		height: 48px;
		display: grid;
		place-items: center;
		border: 1px solid var(--line);
		border-radius: 8px;
		background: var(--card-2);
		color: var(--muted);
		overflow: hidden;
		padding: 0;
		cursor: pointer;
	}
	.att-thumb:disabled {
		cursor: default;
	}
	.att-thumb :global(img.att-img) {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.att-main {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.att-name {
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.att-desc,
	.att-thread {
		color: var(--muted);
		font-size: 13px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.att-thread:hover {
		color: var(--accent);
	}
	.att-actions {
		display: flex;
		gap: 10px;
		justify-content: flex-end;
		white-space: nowrap;
	}
	.att-row.missing .att-name {
		color: var(--muted);
	}
	@media (max-width: 760px) {
		.att-row {
			grid-template-columns: 48px minmax(0, 1fr) auto;
		}
		.att-row .att-thread,
		.att-row .badge.type,
		.att-row .doc-date {
			display: none;
		}
	}
</style>
