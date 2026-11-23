<script lang="ts">
	// Every file in the project's attachments/ folder, as a document library: those uploaded in its threads, with where
	// each came from, and those put in the folder by hand (a PDF someone sent). The files themselves are opened with the
	// system's program; an upload is in its thread, which a click opens. Any of them can be linked from markdown as
	// [[file name]]. Its folders are kept: one folder is shown at a time, its subfolders first, with the path to it on top;
	// a filter searches every folder. A list with sortable columns, or tiles with a preview.
	import { onMount, tick } from 'svelte';
	import { invalidate } from '$app/navigation';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import FileIcon from '@lucide/svelte/icons/file';
	import FileArchive from '@lucide/svelte/icons/file-archive';
	import FileCode from '@lucide/svelte/icons/file-code';
	import FileImageIcon from '@lucide/svelte/icons/file-image';
	import FileMusic from '@lucide/svelte/icons/file-music';
	import FileSpreadsheet from '@lucide/svelte/icons/file-spreadsheet';
	import FileText from '@lucide/svelte/icons/file-text';
	import FileVideoCamera from '@lucide/svelte/icons/file-video-camera';
	import Folder from '@lucide/svelte/icons/folder';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import List from '@lucide/svelte/icons/list';
	import Presentation from '@lucide/svelte/icons/presentation';
	import FileImage from '$lib/components/FileImage.svelte';
	import { api } from '$lib/api';
	import { formatSize, threadUrl } from '$lib/actions';
	import { ago, dateTime, plural } from '$lib/format';
	import { listOrder } from '$lib/order.svelte';
	import { attempt } from '$lib/ui.svelte';
	import type { Upload } from '$lib/types';
	import type { PageData } from './$types';
	import { _ATTACHMENTS } from './+page';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project.slug);
	const titles = $derived(new Map((data.projects.find((p) => p.slug === project)?.threads ?? []).map((t) => [t.slug, t.title])));
	const threadTitle = (u: Upload) => titles.get(u.threadSlug) ?? u.threadSlug;
	/** Put in the folder by hand, not uploaded in a thread. */
	const loose = (u: Upload) => u.threadSlug === '';

	// ---- what kind of file ------------------------------------------------------------------------

	type Tone = 'red' | 'green' | 'amber' | 'accent' | 'muted';
	interface FileType {
		label: string;
		icon: typeof FileIcon;
		tone: Tone;
	}
	const TYPES: Array<[string[], FileType]> = [
		[['pdf'], { label: 'PDF', icon: FileText, tone: 'red' }],
		[['doc', 'docx', 'odt', 'rtf', 'txt', 'md', 'pages'], { label: 'Document', icon: FileText, tone: 'accent' }],
		[['xls', 'xlsx', 'ods', 'csv', 'numbers'], { label: 'Spreadsheet', icon: FileSpreadsheet, tone: 'green' }],
		[['ppt', 'pptx', 'odp', 'key'], { label: 'Presentation', icon: Presentation, tone: 'amber' }],
		[['zip', 'rar', '7z', 'tar', 'gz'], { label: 'Archive', icon: FileArchive, tone: 'muted' }],
		[['mp4', 'mov', 'webm', 'mkv', 'avi'], { label: 'Video', icon: FileVideoCamera, tone: 'accent' }],
		[['mp3', 'wav', 'm4a', 'ogg', 'flac'], { label: 'Audio', icon: FileMusic, tone: 'accent' }],
		[['js', 'ts', 'py', 'rs', 'json', 'html', 'css', 'java', 'cs', 'sql', 'yml', 'yaml'], { label: 'Code', icon: FileCode, tone: 'muted' }]
	];
	const IMAGE: FileType = { label: 'Image', icon: FileImageIcon, tone: 'amber' };
	const OTHER: FileType = { label: 'File', icon: FileIcon, tone: 'muted' };

	const extension = (name: string) => (name.includes('.') ? (name.split('.').pop() ?? '').toLowerCase() : '');
	function typeOf(u: Upload): FileType {
		if (u.image) return IMAGE;
		const ext = extension(u.name);
		return TYPES.find(([exts]) => exts.includes(ext))?.[1] ?? OTHER;
	}

	// ---- what is shown, and how ---------------------------------------------------------------------

	type Kind = 'all' | 'images' | 'files';
	let kind = $state<Kind>('all');
	let filter = $state('');

	const counts = $derived({
		all: data.uploads.length,
		images: data.uploads.filter((u) => u.image).length,
		files: data.uploads.filter((u) => !u.image).length
	});

	type View = 'list' | 'tiles';
	const VIEW_KEY = 'ledger.attachments.view';
	let view = $state<View>('list');
	function setView(next: View) {
		view = next;
		try {
			localStorage.setItem(VIEW_KEY, next);
		} catch {
			// Only a convenience.
		}
	}

	type SortKey = 'name' | 'modified' | 'size';
	// Newest first unless Settings asks for oldest first, until a column is picked.
	let sortKey = $state<SortKey>('modified');
	let ascending = $state(listOrder.order === 'asc');
	function sortBy(key: SortKey) {
		if (sortKey === key) ascending = !ascending;
		else {
			sortKey = key;
			ascending = key === 'name';
		}
	}
	const ariaSort = (key: SortKey) => (sortKey === key ? (ascending ? 'ascending' : 'descending') : 'none');

	// ---- folders ------------------------------------------------------------------------------------

	/** Where a file is inside attachments/: `hans/brief.pdf`. */
	const relOf = (u: Upload) => u.file.replace(/^attachments\/?/, '');
	/** The folder a path is in, inside attachments/ (empty for attachments/ itself). */
	const parentOf = (rel: string) => rel.split('/').slice(0, -1).join('/');

	/** The folder shown, inside attachments/: '' is attachments/ itself, `hans/2026` a folder in it. */
	let dir = $state('');
	const crumbs = $derived(dir ? dir.split('/').map((name, i, all) => ({ name, path: all.slice(0, i + 1).join('/') })) : []);
	const searching = $derived(filter.trim() !== '');

	interface FolderView {
		name: string;
		path: string;
		files: number;
		size: number;
		/** When the newest file in it was added. */
		latest: string;
	}

	const ofKind = $derived(data.uploads.filter((u) => kind === 'all' || (kind === 'images') === u.image));

	const byName = (a: string, b: string) => a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' });
	const order = <T,>(items: T[], name: (t: T) => string, size: (t: T) => number, at: (t: T) => string) => {
		const compare = (a: T, b: T) => (sortKey === 'name' ? byName(name(a), name(b)) : sortKey === 'size' ? size(a) - size(b) : at(a).localeCompare(at(b)));
		return [...items].sort((a, b) => (ascending ? compare(a, b) : compare(b, a)));
	};

	/** The folders in the one shown, with what is in them (all the way down); none while searching. */
	const folders = $derived.by(() => {
		if (searching) return [];
		const found = new Map<string, FolderView>();
		const prefix = dir ? `${dir}/` : '';
		for (const u of ofKind) {
			const rel = relOf(u);
			if (!rel.startsWith(prefix)) continue;
			const rest = rel.slice(prefix.length);
			const slash = rest.indexOf('/');
			if (slash < 0) continue;
			const name = rest.slice(0, slash);
			const folder = found.get(name) ?? { name, path: prefix + name, files: 0, size: 0, latest: '' };
			folder.files += 1;
			folder.size += u.size;
			if (u.uploadedAt > folder.latest) folder.latest = u.uploadedAt;
			found.set(name, folder);
		}
		return order(
			[...found.values()],
			(f) => f.name,
			(f) => f.size,
			(f) => f.latest
		);
	});

	/** The files in the folder shown; while searching, every match in every folder. */
	const shown = $derived.by(() => {
		const q = filter.trim().toLowerCase();
		const picked = q
			? ofKind.filter((u) => [u.name, u.file, u.description, loose(u) ? '' : threadTitle(u)].some((text) => text.toLowerCase().includes(q)))
			: ofKind.filter((u) => parentOf(relOf(u)) === dir);
		return order(
			picked,
			(u) => u.name,
			(u) => u.size,
			(u) => u.uploadedAt
		);
	});

	function enter(path: string) {
		dir = path;
	}

	/** The first line of a description, as plain words. */
	const firstLine = (text: string) => text.trim().split('\n')[0]?.replace(/[#*_`>]/g, '').trim() ?? '';
	/** The folder a file is in, under attachments/ (empty for the folder itself). */
	const subfolder = (u: Upload) => parentOf(relOf(u));

	const open = (u: Upload, reveal: boolean) => attempt(() => api.openProjectFile(project, u.file, reveal));
	const openFolder = () => attempt(() => api.openAttachmentsFolder(project, dir));

	// Files put in the folder outside Ledger show up when it gets the focus back.
	onMount(() => {
		try {
			const saved = localStorage.getItem(VIEW_KEY);
			if (saved === 'list' || saved === 'tiles') view = saved;
		} catch {
			// Only a convenience.
		}
		const onFocus = () => void invalidate(_ATTACHMENTS);
		window.addEventListener('focus', onFocus);
		return () => window.removeEventListener('focus', onFocus);
	});

	// Another project starts in its attachments/ folder.
	$effect(() => {
		void project;
		dir = '';
	});

	// The file a link pointed at: its folder shown, whatever the filter, and scrolled to.
	// Once per link: the page reloads when Ledger gets the focus back, and that shouldn't take you back there.
	let pointedAt: string | null = null;
	$effect(() => {
		const file = data.file;
		if (!file || file === pointedAt) return;
		pointedAt = file;
		kind = 'all';
		filter = '';
		dir = parentOf(file.replace(/^attachments\/?/, ''));
		void tick().then(() => document.querySelector(`[data-file="${CSS.escape(file)}"]`)?.scrollIntoView({ block: 'center' }));
	});

	const KINDS: Array<[Kind, string]> = [
		['all', 'All'],
		['images', 'Images'],
		['files', 'Other files']
	];
	const COLUMNS: Array<[SortKey, string]> = [
		['name', 'Name'],
		['modified', 'Added'],
		['size', 'Size']
	];
</script>

<svelte:head><title>Attachments | {data.project.name} | Ledger</title></svelte:head>

{#snippet thumb(u: Upload, big: boolean)}
	{@const type = typeOf(u)}
	{#if u.exists && u.image}
		<FileImage {project} path={u.file} alt={u.name} cls={big ? 'lib-tile-img' : 'lib-thumb-img'} />
	{:else}
		<span class="lib-icon tone-{type.tone}" class:big><type.icon size={big ? 34 : 18} /></span>
	{/if}
{/snippet}

{#snippet source(u: Upload)}
	{#if loose(u)}
		<span class="lib-source" title="Put in the attachments folder, not uploaded in a thread">
			<FolderOpen size={13} />{searching && subfolder(u) ? subfolder(u) : 'In the folder'}
		</span>
	{:else}
		<a class="lib-source lib-thread" href="{threadUrl(project, u.threadSlug)}#u-{u.id}" title="Open the thread it was uploaded in">{threadTitle(u)}</a>
	{/if}
{/snippet}

<div class="page">
	<div class="page-inner">
		<div class="panel lib">
			<div class="lib-head">
				<div>
					<h2>Attachments</h2>
					<p class="sub">
						Every file in the project's <code>attachments/</code> folder: uploaded in a thread, or put there yourself. Link one from any note
						with <code>[[</code> and its name.
					</p>
				</div>
				<button class="btn primary small" onclick={openFolder} title={dir ? `Open attachments/${dir}, to put files in it` : 'Open the attachments folder, to put files in it'}>
					<FolderOpen size={14} />Open folder
				</button>
			</div>

			<div class="lib-bar">
				<div class="lib-kinds" role="group" aria-label="Which attachments">
					{#each KINDS as [value, label] (value)}
						<button class:on={kind === value} aria-pressed={kind === value} onclick={() => (kind = value)}>
							{label}<span class="lib-n">{counts[value]}</span>
						</button>
					{/each}
				</div>
				<span class="lib-bar-end">
					<input type="search" class="lib-filter" placeholder="Filter" bind:value={filter} aria-label="Filter by name, description or thread" />
					<span class="lib-views" role="group" aria-label="View">
						<button class:on={view === 'list'} aria-pressed={view === 'list'} title="List" aria-label="List" onclick={() => setView('list')}><List size={15} /></button>
						<button class:on={view === 'tiles'} aria-pressed={view === 'tiles'} title="Tiles" aria-label="Tiles" onclick={() => setView('tiles')}><LayoutGrid size={15} /></button>
					</span>
				</span>
			</div>

			{#if data.uploads.length > 0}
				<nav class="lib-crumbs" aria-label="Folder">
					{#if searching}
						<span class="lib-crumb here">Search results in every folder</span>
					{:else}
						<button class="lib-crumb" class:here={!dir} onclick={() => enter('')} aria-current={dir ? undefined : 'page'}><FolderOpen size={14} />attachments</button>
						{#each crumbs as crumb, i (crumb.path)}
							<ChevronRight size={14} class="lib-crumb-sep" />
							<button class="lib-crumb" class:here={i === crumbs.length - 1} onclick={() => enter(crumb.path)} aria-current={i === crumbs.length - 1 ? 'page' : undefined}>
								{crumb.name}
							</button>
						{/each}
					{/if}
				</nav>
			{/if}

			{#if data.uploads.length === 0}
				<div class="lib-empty">
					<FolderOpen size={32} />
					<strong>No files yet</strong>
					<span>Upload a file or a screenshot in a thread, or put files in the folder, and they show up here.</span>
					<button class="btn small" onclick={openFolder}><FolderOpen size={14} />Open folder</button>
				</div>
			{:else if shown.length === 0 && folders.length === 0}
				<div class="lib-empty">
					<span>{searching ? `Nothing matches "${filter.trim()}".` : kind === 'all' ? 'This folder is empty.' : 'Nothing of this kind in this folder.'}</span>
				</div>
			{:else if view === 'list'}
				<div class="lib-table" role="table" aria-label="Attachments">
					<div class="lib-row lib-columns" role="row">
						<span role="columnheader"></span>
						{#each COLUMNS as [key, label] (key)}
							<button class="lib-col col-{key}" class:on={sortKey === key} role="columnheader" aria-sort={ariaSort(key)} onclick={() => sortBy(key)}>
								{label}
								{#if sortKey === key}
									{#if ascending}<ArrowUp size={12} />{:else}<ArrowDown size={12} />{/if}
								{/if}
							</button>
							{#if key === 'name'}<span class="lib-col col-source" role="columnheader">From</span>{/if}
						{/each}
						<span role="columnheader"></span>
					</div>
					{#each folders as f (f.path)}
						<div class="lib-row lib-folder" role="row">
							<span class="lib-thumb" role="cell"><span class="lib-icon tone-amber"><Folder size={18} /></span></span>
							<span class="lib-name" role="cell">
								<button class="lib-title" title="Open {f.path}" onclick={() => enter(f.path)}>{f.name}</button>
							</span>
							<span class="col-source lib-meta" role="cell">{plural(f.files, 'file')}</span>
							<span class="col-modified lib-meta" role="cell" title={dateTime(f.latest)}>{ago(f.latest)}</span>
							<span class="col-size lib-meta" role="cell">{formatSize(f.size)}</span>
							<span role="cell"></span>
						</div>
					{/each}
					{#each shown as u (u.id)}
						<div class="lib-row" class:missing={!u.exists} class:pointed={u.file === data.file} data-file={u.file} role="row">
							<span class="lib-thumb" role="cell">{@render thumb(u, false)}</span>
							<span class="lib-name" role="cell">
								<button class="lib-title" title={u.exists ? `Open ${u.file}` : 'The file is missing'} disabled={!u.exists} onclick={() => open(u, false)}>{u.name}</button>
								{#if firstLine(u.description)}<span class="lib-desc">{firstLine(u.description)}</span>{/if}
							</span>
							<span class="col-source" role="cell">{@render source(u)}</span>
							<span class="col-modified lib-meta" role="cell" title={dateTime(u.uploadedAt)}>{ago(u.uploadedAt)}</span>
							<span class="col-size lib-meta" role="cell">{formatSize(u.size)}</span>
							<span class="lib-actions" role="cell">
								{#if u.exists}
									<button class="btn ghost small" onclick={() => open(u, true)} title="Show it in its folder"><FolderOpen size={14} /></button>
								{:else}
									<span class="badge draft">Missing</span>
								{/if}
							</span>
						</div>
					{/each}
				</div>
			{:else}
				<div class="lib-tiles">
					{#each folders as f (f.path)}
						<div class="lib-tile">
							<button class="lib-preview" title="Open {f.path}" onclick={() => enter(f.path)}>
								<span class="lib-icon big tone-amber"><Folder size={34} /></span>
							</button>
							<div class="lib-tile-body">
								<span class="lib-tile-name" title={f.name}>{f.name}</span>
								<span class="lib-meta">Folder | {plural(f.files, 'file')}</span>
							</div>
						</div>
					{/each}
					{#each shown as u (u.id)}
						{@const type = typeOf(u)}
						<div class="lib-tile" class:missing={!u.exists} class:pointed={u.file === data.file} data-file={u.file}>
							<button class="lib-preview" title={u.exists ? `Open ${u.file}` : 'The file is missing'} disabled={!u.exists} onclick={() => open(u, false)}>
								{@render thumb(u, true)}
							</button>
							<div class="lib-tile-body">
								<span class="lib-tile-name" title={u.name}>{u.name}</span>
								<span class="lib-meta">{u.exists ? `${type.label} | ${formatSize(u.size)}` : 'Missing'}</span>
								{@render source(u)}
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if data.uploads.length > 0}
				<p class="lib-foot">
					{[folders.length ? plural(folders.length, 'folder') : '', plural(shown.length, 'file')].filter(Boolean).join(', ')}{searching
						? ` found of ${data.uploads.length}`
						: ''}
				</p>
			{/if}
		</div>
	</div>
</div>

<style>
	.lib {
		gap: 14px;
	}
	.lib-head {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 16px;
	}
	.lib-head h2 {
		margin: 0 0 4px;
	}
	.lib-head .sub {
		margin: 0;
	}

	/* ---- the command bar ---- */
	.lib-bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		flex-wrap: wrap;
		padding-bottom: 12px;
		border-bottom: 1px solid var(--line);
	}
	.lib-bar-end {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.lib-kinds,
	.lib-views {
		display: inline-flex;
		gap: 2px;
		padding: 2px;
		border-radius: 8px;
		background: var(--card-2);
	}
	.lib-kinds button,
	.lib-views button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		border: 0;
		background: transparent;
		color: var(--muted);
		padding: 5px 10px;
		border-radius: 6px;
		font-size: 13px;
		font-weight: 500;
	}
	.lib-views button {
		padding: 5px 8px;
	}
	.lib-kinds button:hover,
	.lib-views button:hover {
		color: var(--ink);
	}
	.lib-kinds button.on,
	.lib-views button.on {
		background: var(--card);
		color: var(--ink);
		box-shadow: 0 0 0 1px var(--line);
	}
	.lib-n {
		font-size: 11px;
		color: var(--faint);
	}
	.lib-filter {
		width: 200px;
		padding: 6px 10px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--field);
	}

	/* ---- where you are ---- */
	.lib-crumbs {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-wrap: wrap;
		font-size: 14px;
	}
	.lib-crumb {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		border: 0;
		background: none;
		padding: 3px 6px;
		border-radius: 6px;
		color: var(--muted);
		font-weight: 500;
	}
	button.lib-crumb:hover {
		background: var(--card-2);
		color: var(--ink);
	}
	.lib-crumb.here {
		color: var(--ink);
		font-weight: 600;
	}
	.lib-crumbs :global(.lib-crumb-sep) {
		color: var(--faint);
	}

	/* ---- the list ---- */
	.lib-table {
		display: flex;
		flex-direction: column;
	}
	.lib-row {
		display: grid;
		grid-template-columns: 36px minmax(0, 1fr) minmax(0, 180px) 110px 80px 44px;
		gap: 14px;
		align-items: center;
		padding: 8px 8px;
		border-bottom: 1px solid var(--line);
		border-radius: 6px;
	}
	.lib-row:not(.lib-columns):hover {
		background: var(--card-2);
	}
	.lib-columns {
		padding-top: 2px;
		padding-bottom: 6px;
		border-radius: 0;
	}
	.lib-col {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		border: 0;
		background: none;
		padding: 0;
		color: var(--muted);
		font-size: 12px;
		font-weight: 600;
		text-align: left;
		justify-self: start;
	}
	button.lib-col:hover,
	.lib-col.on {
		color: var(--ink);
	}
	.lib-thumb {
		width: 36px;
		height: 36px;
		display: grid;
		place-items: center;
		border-radius: 6px;
		overflow: hidden;
	}
	.lib-thumb :global(img.lib-thumb-img) {
		width: 100%;
		height: 100%;
		object-fit: cover;
		border-radius: 6px;
		border: 1px solid var(--line);
	}
	.lib-icon {
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		border-radius: 6px;
	}
	.lib-icon.big {
		width: 64px;
		height: 64px;
		border-radius: 14px;
	}
	.tone-red {
		color: var(--red);
		background: var(--red-soft);
	}
	.tone-green {
		color: var(--green);
		background: var(--green-soft);
	}
	.tone-amber {
		color: var(--amber);
		background: var(--amber-soft);
	}
	.tone-accent {
		color: var(--accent);
		background: var(--accent-soft);
	}
	.tone-muted {
		color: var(--muted);
		background: var(--card-2);
	}
	.lib-name {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.lib-title {
		border: 0;
		background: none;
		padding: 0;
		color: var(--ink);
		font-weight: 600;
		text-align: left;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.lib-title:not(:disabled):hover {
		color: var(--accent);
		text-decoration: underline;
	}
	.lib-desc,
	.lib-meta,
	.lib-source {
		color: var(--muted);
		font-size: 13px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.lib-source {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		max-width: 100%;
	}
	.lib-thread:hover {
		color: var(--accent);
	}
	.lib-actions {
		display: flex;
		justify-content: flex-end;
		opacity: 0;
		transition: opacity 120ms;
	}
	.lib-row:hover .lib-actions,
	.lib-row:focus-within .lib-actions,
	.lib-row.missing .lib-actions {
		opacity: 1;
	}
	.missing .lib-title,
	.missing .lib-tile-name {
		color: var(--faint);
	}
	.pointed,
	.lib-row.pointed:hover {
		background: var(--accent-soft);
	}

	/* ---- the tiles ---- */
	.lib-tiles {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
		gap: 12px;
	}
	.lib-tile {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--card);
		overflow: hidden;
		transition: border-color 120ms;
	}
	.lib-tile:hover {
		border-color: var(--accent);
	}
	.lib-preview {
		height: 120px;
		display: grid;
		place-items: center;
		border: 0;
		border-bottom: 1px solid var(--line);
		background: var(--card-2);
		padding: 0;
		overflow: hidden;
	}
	.lib-preview :global(img.lib-tile-img) {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.lib-tile-body {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 10px 12px 12px;
		min-width: 0;
	}
	.lib-tile-name {
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* ---- empty, and the count ---- */
	.lib-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		padding: 40px 16px;
		color: var(--muted);
		text-align: center;
	}
	.lib-empty strong {
		color: var(--ink);
	}
	.lib-foot {
		margin: 0;
		color: var(--faint);
		font-size: 12px;
	}

	@media (max-width: 760px) {
		.lib-head {
			flex-direction: column;
		}
		.lib-filter {
			width: 140px;
		}
		.lib-row {
			grid-template-columns: 36px minmax(0, 1fr) 70px 44px;
		}
		.lib-row .col-source,
		.lib-row .col-modified {
			display: none;
		}
	}
</style>
