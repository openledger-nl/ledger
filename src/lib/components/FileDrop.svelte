<script lang="ts">
	// Files dragged onto Ledger from outside. While they hover, the window shows where they will go. Dropped in a
	// thread, they are attached to it at once; anywhere else, a dialog asks which project (and thread) they belong to.
	// A drag that starts inside Ledger (a card on the board, selected text) is left to whatever handles it: those start
	// with a `dragstart` here, one from outside never does. Every drag from outside is caught before anything else sees
	// it, whatever it says it carries (a file manager on Linux says "text/uri-list", not "Files", while it hovers):
	// otherwise the window would open the dropped file itself instead of Ledger, and an editor would paste its text.
	import FileUp from '@lucide/svelte/icons/file-up';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { formatSize, newProjectFlow, threadUrl } from '$lib/actions';
	import { newest, plural } from '$lib/format';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { ProjectSummary, Upload } from '$lib/types';
	import Dropdown, { type DropdownOption } from './Dropdown.svelte';
	import Modal from './Modal.svelte';

	let { projects }: { projects: ProjectSummary[] } = $props();

	const NEW = '__new__';

	/** The thread that is open, when one is: dropped files go straight there. */
	const here = $derived.by(() => {
		const { project, thread } = page.params;
		if (!project || !thread) return null;
		const p = projects.find((x) => x.slug === project);
		const t = p?.threads.find((x) => x.slug === thread);
		return p && t ? { project: p.slug, thread: t.slug, title: t.title } : null;
	});

	// ---- hovering ------------------------------------------------------------------------------------
	// `dragover` keeps coming while files hover the window; when it stops (they left, or the drag was cancelled), the
	// overlay goes. That is steadier than counting enter and leave over every element underneath.

	let hovering = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	/** A drag that began in this window: not a file from outside. */
	let inside = false;
	const outside = (e: DragEvent) => !inside && !!e.dataTransfer;

	function over(e: DragEvent) {
		if (!outside(e)) return;
		e.preventDefault();
		e.stopPropagation();
		if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
		hovering = true;
		clearTimeout(timer);
		timer = setTimeout(() => (hovering = false), 250);
	}

	function drop(e: DragEvent) {
		if (!outside(e)) return;
		e.preventDefault();
		e.stopPropagation();
		clearTimeout(timer);
		hovering = false;
		const dropped = droppedOf(e.dataTransfer);
		if (!dropped) return;
		if (here) void save(here.project, here.thread, here.title, dropped, false);
		else open(dropped);
	}

	/** What was dropped: the files' paths when the system gives them (file:// URLs; Ledger copies the files itself, so a
	 *  large one is never read into the page), or else the files' contents. `null` when it held no file at all. */
	type Dropped = { kind: 'paths'; paths: string[] } | { kind: 'files'; files: File[] };
	function droppedOf(data: DataTransfer | null): Dropped | null {
		if (!data) return null;
		const paths = (data.getData('text/uri-list') || '')
			.split(/\r?\n/)
			.map((line) => line.trim())
			.filter((line) => line.startsWith('file://'))
			.map(pathOf)
			.filter((p): p is string => !!p);
		if (paths.length) return { kind: 'paths', paths };
		const files = Array.from(data.files ?? []);
		return files.length ? { kind: 'files', files } : null;
	}

	/** `file:///home/me/a%20b.pdf` → `/home/me/a b.pdf`; `file:///C:/x.pdf` → `C:/x.pdf`. */
	function pathOf(uri: string): string | null {
		try {
			const path = decodeURIComponent(new URL(uri).pathname);
			return /^\/[A-Za-z]:\//.test(path) ? path.slice(1) : path;
		} catch {
			return null;
		}
	}

	const namesOf = (d: Dropped) => (d.kind === 'paths' ? d.paths.map((p) => p.split(/[\\/]/).pop() ?? p) : d.files.map((f) => f.name));

	// ---- saving ----------------------------------------------------------------------------------------

	let busy = $state(false);

	/** Saves what was dropped; a file that can't be read (a folder, say) is said and skipped, the rest still go. */
	async function save(project: string, thread: string, title: string, dropped: Dropped, go: boolean): Promise<boolean> {
		busy = true;
		const saved: Upload[] = [];
		if (dropped.kind === 'paths') {
			saved.push(...((await attempt(() => api.uploadFiles(project, thread, dropped.paths, ''))) ?? []));
		} else {
			for (const file of dropped.files) {
				const upload = await attempt(() => api.uploadDropped(project, thread, file));
				if (upload) saved.push(upload);
			}
		}
		busy = false;
		if (!saved.length) return false;
		toast(`${plural(saved.length, 'file')} attached to “${title}”`);
		await refresh();
		if (go) await goto(`${threadUrl(project, thread)}?item=u-${saved[0].id}`);
		return true;
	}

	// ---- asking where they go ------------------------------------------------------------------------

	let pending = $state<Dropped | null>(null);
	let project = $state('');
	let thread = $state('');
	let title = $state('');

	const chosen = $derived(projects.find((p) => p.slug === project));
	const creating = $derived(thread === NEW);
	const projectOptions = $derived<DropdownOption[]>(projects.map((p) => ({ value: p.slug, label: p.name })));
	const threadOptions = $derived<DropdownOption[]>([
		...newest(chosen?.threads ?? [], (t) => t.lastActivity).map((t) => ({ value: t.slug, label: t.title })),
		{ value: NEW, label: '+ New thread…' }
	]);

	/** The project's most recently active thread, or a new one when it has none. */
	function defaultThread(slug: string): string {
		const p = projects.find((q) => q.slug === slug);
		return newest(p?.threads ?? [], (t) => t.lastActivity)[0]?.slug ?? NEW;
	}

	function open(dropped: Dropped) {
		pending = dropped;
		project = projects.find((p) => p.slug === page.params.project)?.slug ?? projects[0]?.slug ?? '';
		thread = defaultThread(project);
		title = '';
	}

	function pickProject(slug: string) {
		project = slug;
		thread = defaultThread(slug);
	}

	async function attach() {
		if (busy || !pending || !project || (creating && !title.trim())) return;
		let slug = thread;
		let name = chosen?.threads.find((t) => t.slug === thread)?.title ?? thread;
		if (creating) {
			busy = true;
			const made = await attempt(() => api.createThread(project, title.trim()));
			busy = false;
			if (!made) return;
			slug = made.slug;
			name = made.title;
		}
		if (await save(project, slug, name, pending, true)) pending = null;
	}
</script>

<svelte:window
	ondragstartcapture={() => (inside = true)}
	ondragendcapture={() => (inside = false)}
	ondragovercapture={over}
	ondragentercapture={over}
	ondropcapture={drop}
/>

{#if hovering}
	<div class="file-drop" aria-hidden="true">
		<div class="file-drop-card">
			<FileUp size={28} />
			{#if here}
				<span>Drop to attach to <strong>{here.title}</strong></span>
			{:else}
				<span>Drop to save in a project</span>
				<span class="hint">You choose the project and the thread next.</span>
			{/if}
		</div>
	</div>
{/if}

{#if pending}
	{@const names = namesOf(pending)}
	<Modal title={names.length === 1 ? 'Save the file' : `Save ${names.length} files`} onclose={() => (pending = null)}>
		<ul class="file-drop-list">
			{#each names as name, i (i)}
				<li>
					<span class="name">{name}</span>
					{#if pending.kind === 'files'}<span class="hint">{formatSize(pending.files[i].size)}</span>{/if}
				</li>
			{/each}
		</ul>

		{#if projects.length === 0}
			<p class="notice">Files are kept in a thread of a project, and there are no projects yet.</p>
			<div class="row-end">
				<button class="btn" onclick={newProjectFlow}>New project</button>
			</div>
		{:else}
			<div class="form-row">
				<div class="field grow">
					Project
					<!-- Picking the same project again keeps the thread already chosen. -->
					<Dropdown bind:value={() => project, (v) => v !== project && pickProject(v)} options={projectOptions} label="Project" full />
				</div>
				<div class="field grow">
					Thread
					<Dropdown bind:value={thread} options={threadOptions} label="Thread" full />
				</div>
			</div>
			{#if creating}
				<label class="field">
					New thread title
					<!-- svelte-ignore a11y_autofocus -->
					<input bind:value={title} placeholder="e.g. Research material" autofocus onkeydown={(e) => e.key === 'Enter' && attach()} />
				</label>
			{/if}
		{/if}

		{#snippet footer()}
			<span class="spacer-h"></span>
			<button class="btn ghost" onclick={() => (pending = null)}>Cancel</button>
			<button class="btn primary" onclick={attach} disabled={busy || !project || (creating && !title.trim())}>
				{busy ? 'Saving…' : 'Save'}
			</button>
		{/snippet}
	</Modal>
{/if}
