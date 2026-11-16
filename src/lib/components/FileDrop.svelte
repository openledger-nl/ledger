<script lang="ts">
	// Files dragged onto Ledger from outside. While they hover, the window shows where they will go. Dropped in a
	// thread, they are attached to it at once; anywhere else, a dialog asks which project (and thread) they belong to.
	// Only drags that carry files count: dragging a card on the board, or text, is left to whatever handles it. File
	// drops are caught before anything else sees them, so an editor never pastes a dropped file's text instead.
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

	const carriesFiles = (e: DragEvent) => !!e.dataTransfer && Array.from(e.dataTransfer.types).includes('Files');

	function over(e: DragEvent) {
		if (!carriesFiles(e)) return;
		e.preventDefault();
		e.stopPropagation();
		if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
		hovering = true;
		clearTimeout(timer);
		timer = setTimeout(() => (hovering = false), 250);
	}

	function drop(e: DragEvent) {
		if (!carriesFiles(e)) return;
		e.preventDefault();
		e.stopPropagation();
		clearTimeout(timer);
		hovering = false;
		const files = Array.from(e.dataTransfer?.files ?? []);
		if (!files.length) return;
		if (here) void save(here.project, here.thread, here.title, files, false);
		else open(files);
	}

	// ---- saving ----------------------------------------------------------------------------------------

	let busy = $state(false);

	/** Saves the files one by one; one that can't be read (a folder, say) is said and skipped, the rest still go. */
	async function save(project: string, thread: string, title: string, files: File[], go: boolean): Promise<boolean> {
		busy = true;
		const saved: Upload[] = [];
		for (const file of files) {
			const upload = await attempt(() => api.uploadDropped(project, thread, file));
			if (upload) saved.push(upload);
		}
		busy = false;
		if (!saved.length) return false;
		toast(`${plural(saved.length, 'file')} attached to “${title}”`);
		await refresh();
		if (go) await goto(`${threadUrl(project, thread)}?item=u-${saved[0].id}`);
		return true;
	}

	// ---- asking where they go ------------------------------------------------------------------------

	let pending = $state<File[] | null>(null);
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

	function open(files: File[]) {
		pending = files;
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

<svelte:window ondragovercapture={over} ondragentercapture={over} ondropcapture={drop} />

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
	<Modal title={pending.length === 1 ? 'Save the file' : `Save ${pending.length} files`} onclose={() => (pending = null)}>
		<ul class="file-drop-list">
			{#each pending as f, i (i)}
				<li><span class="name">{f.name}</span><span class="hint">{formatSize(f.size)}</span></li>
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
