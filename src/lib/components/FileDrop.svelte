<script lang="ts">
	// Files dragged onto Ledger from outside. While they hover, the window shows where they will go. Dropped in a
	// thread, they are attached to it at once; anywhere else, a dialog asks which project (and thread) they belong to.
	// A drag that starts inside Ledger (a card on the board, selected text) is left to whatever handles it: those start
	// with a `dragstart` here, one from outside never does. Every drag from outside is caught before anything else sees
	// it, whatever it says it carries (a file manager on Linux says "text/uri-list", not "Files", while it hovers):
	// otherwise the window would open the dropped file itself instead of Ledger, and an editor would paste its text.
	import FileUp from '@lucide/svelte/icons/file-up';
	import { getCurrentWebview } from '@tauri-apps/api/webview';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { formatSize, isImageName, newProjectFlow, shownMimeOf, threadUrl } from '$lib/actions';
	import { newest, plural } from '$lib/format';
	import { logError } from '$lib/log.svelte';
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
		if (!outside(e)) {
			inside = false;
			return;
		}
		e.preventDefault();
		e.stopPropagation();
		clearTimeout(timer);
		hovering = false;
		const dropped = droppedOf(e.dataTransfer);
		if (!dropped) {
			// Never silently: say so, and log what the system did hand over, so it can be put right.
			const types = Array.from(e.dataTransfer?.types ?? []).join(', ') || 'nothing';
			const schemes = schemesOf(e.dataTransfer);
			logError('file drop', `A drop gave no file to save (it carried: ${types}; addresses: ${schemes})`);
			const web = /\bhttps?\b/.test(schemes);
			toast(
				web
					? 'That was a link from a web page, not a file. Download it first, then drop the file from your folder.'
					: 'Ledger got no file from that drop. Try again, or use “Upload files…” in a thread.',
				true
			);
			return;
		}
		if (here) void save(here.project, here.thread, here.title, dropped, false);
		else open(dropped);
	}

	// On Linux and macOS the window lets Tauri take files dropped on it (dragDropEnabled in tauri.linux.conf.json and
	// tauri.macos.conf.json; those repeat the window of tauri.conf.json, keep them alike): it hands over the files' real
	// paths, which the web view there doesn't. Windows keeps it off, as the board's drag and drop needs that there, and
	// its web view gives the page the files itself (below).
	$effect(() => {
		let alive = true;
		let unlisten: (() => void) | undefined;
		try {
			getCurrentWebview()
				.onDragDropEvent((event) => {
					const p = event.payload;
					if (p.type === 'enter' || p.type === 'over') {
						hovering = true;
						clearTimeout(timer);
						timer = setTimeout(() => (hovering = false), 1500);
					} else if (p.type === 'leave') {
						clearTimeout(timer);
						hovering = false;
					} else if (p.type === 'drop') {
						clearTimeout(timer);
						hovering = false;
						if (!p.paths.length) return;
						const dropped: Dropped = { kind: 'paths', paths: p.paths };
						if (here) void save(here.project, here.thread, here.title, dropped, false);
						else open(dropped);
					}
				})
				.then((fn) => (alive ? (unlisten = fn) : fn()))
				.catch(() => {});
		} catch {
			// Not in Ledger's window (a plain browser): only the page's own drops below.
		}
		return () => {
			alive = false;
			unlisten?.();
		};
	});

	// The listeners sit on the window in the capture phase, so they hear a drag before anything on the page does (an
	// editor would paste a dropped file's text). A drag that starts in Ledger sets `inside` until it ends or drops.
	$effect(() => {
		const start = () => (inside = true);
		const end = () => (inside = false);
		const options = { capture: true };
		window.addEventListener('dragstart', start, options);
		window.addEventListener('dragend', end, options);
		window.addEventListener('dragenter', over, options);
		window.addEventListener('dragover', over, options);
		window.addEventListener('drop', drop, options);
		return () => {
			window.removeEventListener('dragstart', start, options);
			window.removeEventListener('dragend', end, options);
			window.removeEventListener('dragenter', over, options);
			window.removeEventListener('dragover', over, options);
			window.removeEventListener('drop', drop, options);
		};
	});

	/** What was dropped: the files' paths when the system gives them (file:// URLs; Ledger copies the files itself, so a
	 *  large one is never read into the page), or else the files' contents. `null` when it held no file at all. */
	type Dropped = { kind: 'paths'; paths: string[] } | { kind: 'files'; files: File[] };
	function droppedOf(data: DataTransfer | null): Dropped | null {
		if (!data) return null;
		const paths = urisOf(data)
			.filter((uri) => /^file:/i.test(uri))
			.map(pathOf)
			.filter((p): p is string => !!p);
		if (paths.length) return { kind: 'paths', paths };
		const files = Array.from(data.files ?? []);
		return files.length ? { kind: 'files', files } : null;
	}

	/** The addresses a drop holds: its uri-list (comments left out), or a plain-text one that is a file address. */
	function urisOf(data: DataTransfer): string[] {
		const lines = (text: string) => text.split(/\r?\n/).map((line) => line.trim()).filter((line) => line && !line.startsWith('#'));
		const listed = lines(data.getData('text/uri-list') || '');
		return listed.length ? listed : lines(data.getData('text/plain') || '').filter((line) => /^file:/i.test(line));
	}

	/** What kinds of address a drop held, for the log: their schemes and how many, never the addresses themselves. */
	function schemesOf(data: DataTransfer | null): string {
		if (!data) return 'none';
		const counts = new Map<string, number>();
		for (const uri of urisOf(data)) {
			const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(uri)?.[1]?.toLowerCase() ?? 'no scheme';
			counts.set(scheme, (counts.get(scheme) ?? 0) + 1);
		}
		return counts.size ? [...counts].map(([s, n]) => `${n}× ${s}`).join(', ') : 'none';
	}

	/** `file:///home/me/a%20b.pdf`, `file:/home/me/x.pdf`, `file://localhost/…` → the path; `file:///C:/x.pdf` → `C:/x.pdf`. */
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

	// ---- previews of the pictures in the drop ------------------------------------------------------------
	// A path from the file manager is read through the backend (the page can't open it), which also turns a JPEG XL
	// picture into one the window can show; a file the page was handed itself is shown as it is.

	/** How many pictures of one drop get a preview. */
	const MAX_PREVIEWS = 12;
	type Preview = { name: string; url: string | null };
	let previews = $state<Preview[]>([]);

	$effect(() => {
		const dropped = pending;
		const made: string[] = [];
		let stale = false;
		previews = [];
		if (!dropped) return;
		const pictures = namesOf(dropped)
			.map((name, i) => ({ name, i }))
			.filter((p) => isImageName(p.name))
			.slice(0, MAX_PREVIEWS);
		previews = pictures.map((p) => ({ name: p.name, url: null }));
		pictures.forEach(({ name, i }, at) => {
			const blob: Promise<Blob | null> =
				dropped.kind === 'paths'
					? api.previewDroppedImage(dropped.paths[i]).then((b) => new Blob([b], { type: shownMimeOf(name) }))
					: Promise.resolve(/\.jxl$/i.test(name) ? null : dropped.files[i]);
			blob
				.then((b) => {
					if (stale || !b) return;
					const url = URL.createObjectURL(b);
					made.push(url);
					previews[at] = { name, url };
				})
				.catch(() => {});
		});
		return () => {
			stale = true;
			made.forEach((url) => URL.revokeObjectURL(url));
		};
	});

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
		{#if previews.some((p) => p.url)}
			<div class="file-drop-previews" class:single={previews.length === 1}>
				{#each previews as preview, i (i)}
					{#if preview.url}
						<img src={preview.url} alt={preview.name} title={preview.name} />
					{/if}
				{/each}
			</div>
		{/if}
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
