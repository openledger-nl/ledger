<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import BookOpen from '@lucide/svelte/icons/book-open';
	import CodeXml from '@lucide/svelte/icons/code-xml';
	import Copy from '@lucide/svelte/icons/copy';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import FileText from '@lucide/svelte/icons/file-text';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import MessagesSquare from '@lucide/svelte/icons/messages-square';
	import PencilLine from '@lucide/svelte/icons/pencil-line';
	import Trash from '@lucide/svelte/icons/trash';
	import { goto, replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import Dropdown from '$lib/components/Dropdown.svelte';
	import DocSide from '$lib/components/DocSide.svelte';
	import FileImage from '$lib/components/FileImage.svelte';
	import Markdown from '$lib/components/Markdown.svelte';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import Menu, { type MenuItem } from '$lib/components/Menu.svelte';
	import MmmBadge from '$lib/components/MmmBadge.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import SelectionComment from '$lib/components/SelectionComment.svelte';
	import { api } from '$lib/api';
	import { deliverableFile, deliverableUrl, formatSize, newThreadFlow, nextVersion, snapshotPath } from '$lib/actions';
	import { locate, plain, type Anchor, type Mark } from '$lib/anchors';
	import { sha256Hex } from '$lib/hash';
	import { ago, dateTime } from '$lib/format';
	import { MMM } from '$lib/mmm';
	import { dirOf } from '$lib/vaultImages';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import { beforeRestart } from '$lib/updates.svelte';
	import type { Bump, Feedback, MmmLevel, ProvenanceDraft, ProvenanceVersion } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const doc = $derived(data.deliverable);
	const provenance = $derived(data.provenance);
	const project = $derived(data.project.slug);
	const threads = $derived(data.projects.find((p) => p.slug === project)?.threads ?? []);
	const snapshot = $derived(doc.version !== null);
	const isMarkdown = $derived(doc.kind === 'markdown');
	const file = $derived(deliverableFile(doc.path));
	// Pictures in it are looked for beside the file.
	const images = $derived({ project, dir: dirOf(file) });

	// ---- the editable copy (markdown) ----------------------------------------------------------
	// The page keeps its own text while you type and saves it in the background. It is only reset when a
	// different file is opened — or when the file changed elsewhere while nothing here was waiting to be saved.

	let body = $state('');
	let html = $state('');
	let loadedKey = '';
	let loadedPath = '';
	let dirty = false;
	/** Typed in this file during this visit, and when last. */
	let touched = $state(false);
	let editedAt = $state('');
	let lastSaved = 0;
	let saveState = $state<'saved' | 'saving' | 'unsaved'>('saved');
	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	let renderTimer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => {
		const key = doc.path;
		const theirs = doc.body ?? '';
		const theirHtml = doc.html ?? '';
		untrack(() => {
			if (key !== loadedKey) {
				if (dirty) void flush(); // still the previous file's path and text at this point
				loadedKey = key;
				loadedPath = key;
				body = theirs;
				html = theirHtml;
				saveState = 'saved';
				touched = false;
			} else if (!dirty && saveState === 'saved' && Date.now() - lastSaved > 1500 && theirs !== body) {
				body = theirs;
				html = theirHtml;
			}
		});
	});

	async function flush() {
		clearTimeout(saveTimer);
		if (!dirty) return;
		dirty = false;
		saveState = 'saving';
		const saved = await attempt(() => api.saveDeliverable(project, loadedPath, body));
		lastSaved = Date.now();
		saveState = saved ? 'saved' : 'unsaved';
		if (!saved) dirty = true;
	}

	function edited() {
		touched = true;
		editedAt = new Date().toISOString();
		dirty = true;
		saveState = 'unsaved';
		clearTimeout(saveTimer);
		saveTimer = setTimeout(flush, 800);
		clearTimeout(renderTimer);
		renderTimer = setTimeout(async () => {
			html = (await attempt(() => api.renderMarkdown(body))) ?? html;
		}, 250);
	}

	onDestroy(() => {
		clearTimeout(renderTimer);
		void flush();
	});

	// Installing an update restarts Ledger: what is typed and still waiting for the autosave is saved first.
	$effect(() => beforeRestart(flush));

	// ---- view mode -----------------------------------------------------------------------------

	// As in Obsidian: the raw markdown, the live preview (marks hidden away from the cursor), or the rendered page.
	type Mode = 'source' | 'live' | 'reading';
	const modes: { id: Mode; label: string; icon: typeof BookOpen }[] = [
		{ id: 'source', label: 'Source', icon: CodeXml },
		{ id: 'live', label: 'Live preview', icon: PencilLine },
		{ id: 'reading', label: 'Reading', icon: BookOpen }
	];
	let mode = $state<Mode>('live');
	try {
		const saved = localStorage.getItem('ledger-doc-mode');
		if (saved === 'source' || saved === 'live' || saved === 'reading') mode = saved;
	} catch {
		// Storage unavailable: the default is fine.
	}
	function setMode(next: Mode) {
		mode = next;
		try {
			localStorage.setItem('ledger-doc-mode', next);
		} catch {
			// Not persisted.
		}
	}

	// ---- the thread of this page ---------------------------------------------------------------
	// One thread for everything done here: the note a publish leaves, new feedback, and the prompts made from feedback. It is
	// chosen once, at the top, and kept for this deliverable. Until then: where the last version went (or the prompt that
	// asked for the file was written), or else the project's first thread.

	let workThread = $state('');
	let threadKey = '';

	$effect(() => {
		const key = `ledger-doc-thread:${project}:${doc.basePath}`;
		const options = threads.map((t) => t.slug);
		const fallback = [provenance.defaultThread, doc.prompts[0]?.threadSlug].find((slug) => slug && options.includes(slug)) ?? options[0] ?? '';
		untrack(() => {
			if (key !== threadKey) {
				threadKey = key;
				try {
					workThread = localStorage.getItem(key) ?? '';
				} catch {
					workThread = '';
				}
			}
			if (!options.includes(workThread)) workThread = fallback;
		});
	});

	// Remembered as soon as it is chosen.
	$effect(() => {
		const slug = workThread;
		untrack(() => {
			if (!slug || !threadKey) return;
			try {
				localStorage.setItem(threadKey, slug);
			} catch {
				// Not persisted.
			}
		});
	});

	const threadTitle = $derived(threads.find((t) => t.slug === workThread)?.title ?? '');
	const threadOptions = $derived(threads.map((t) => ({ value: t.slug, label: t.title })));

	// ---- actions -------------------------------------------------------------------------------

	let publishing = $state(false);
	let bump = $state<Bump | 'custom'>('minor');
	let custom = $state('');
	let publishBusy = $state(false);
	const preview = $derived({ minor: nextVersion(doc.versions, 'minor'), major: nextVersion(doc.versions, 'major') });

	// What the person says about the change. Claude Code runs started here are recorded as they happen; the box is for AI
	// used anywhere else, and starts ticked when a run is known to have changed this file without being published.
	let aiMade = $state(false);
	let level = $state<MmmLevel | ''>('');
	let promptId = $state('');
	let changeNote = $state('');
	const stopped = $derived(provenance.stoppedRun);
	const manMadeConflict = $derived(level === 'man-made' && (aiMade || provenance.historyHasAi));
	const suggestManMade = $derived(!aiMade && !provenance.historyHasAi && !level);

	function startPublishing() {
		bump = 'minor';
		custom = '';
		aiMade = stopped !== null;
		level = provenance.suggestedLabel ?? '';
		promptId = stopped?.promptId ?? '';
		changeNote = '';
		publishing = true;
	}

	async function publish() {
		publishBusy = true;
		await flush();
		const published = await attempt(() =>
			api.publishDeliverable(project, doc.path, bump === 'custom' ? 'minor' : bump, bump === 'custom' ? custom : null, {
				thread: workThread,
				ai: aiMade,
				mmm: level || null,
				prompt: aiMade ? promptId || null : null,
				// The stopped run only goes on the record when the prompt is the one it ran.
				run: aiMade && stopped && (!promptId || promptId === stopped.promptId) ? stopped.id : null,
				note: changeNote.trim() || null
			})
		);
		publishBusy = false;
		if (!published) return;
		publishing = false;
		toast(`Published version ${published.versions.at(-1) ?? ''}`.trim());
		await refresh();
	}

	async function remove() {
		const ok = await confirmDialog({
			title: `Delete “${doc.name}”?`,
			message: doc.versions.length
				? 'The live file is deleted. Its published versions are kept and stay readable.'
				: 'The file is deleted from the project.',
			confirmLabel: 'Delete',
			danger: true
		});
		if (!ok) return;
		dirty = false;
		if ((await attempt(() => api.deleteDeliverable(project, doc.path))) === undefined) return;
		await refresh();
		await goto(`/projects/${project}/deliverables`);
	}

	async function copyMarkdown() {
		try {
			await navigator.clipboard.writeText(snapshot ? (doc.body ?? '') : body);
			toast('Markdown copied');
		} catch {
			toast('Could not reach the clipboard', true);
		}
	}

	const open = (reveal: boolean) => attempt(() => api.openProjectFile(project, file, reveal));

	// What else can be done with it, behind the ⋯ next to Publish. Delete only for the live file; a version is kept.
	const more = $derived<MenuItem[]>([
		...(isMarkdown ? [{ id: 'copy', label: 'Copy markdown', icon: Copy }] : []),
		...(doc.exists
			? [
					{ id: 'open', label: 'Open in editor', icon: ExternalLink },
					{ id: 'reveal', label: 'Show in folder', icon: FolderOpen }
				]
			: []),
		...(!snapshot && doc.exists ? [{ id: 'delete', label: 'Delete deliverable', icon: Trash, danger: true, rule: true }] : [])
	]);

	function doMore(id: string) {
		if (id === 'copy') void copyMarkdown();
		else if (id === 'open') void open(false);
		else if (id === 'reveal') void open(true);
		else if (id === 'delete') void remove();
	}

	// ---- feedback on a passage -----------------------------------------------------------------
	// A passage selected in the document, in any mode, can be commented on: the comment icon beside the selection hands it
	// here, and the Feedback window opens with it ready to be written on. The passages that already have feedback are lit up
	// in the text, and a click on one shows its feedback.

	let side: DocSide | undefined = $state();
	let docArea: HTMLElement | undefined = $state();
	let reader: Markdown | undefined = $state();
	let editor: MarkdownEditor | undefined = $state();
	/** The passage that feedback is being written on. */
	let pending = $state<Anchor | null>(null);

	// Another file, another selection.
	$effect(() => {
		void doc.path;
		untrack(() => (pending = null));
	});

	const anchorFor = (f: Feedback): Anchor => ({ text: f.quote ?? '', prefix: f.quotePrefix ?? '', suffix: f.quoteSuffix ?? '' });
	/** When each version was published, oldest first. */
	const published = $derived(
		provenance.groups
			.flatMap((g) => g.entries)
			.filter((e): e is ProvenanceVersion => e.kind === 'version')
			.sort((a, b) => a.at.localeCompare(b.at))
	);
	/**
	 * Whether a piece of feedback belongs to what the page shows: the live file takes what was given on it, a version what was
	 * given on that version (`givenOn`: feedback on the live file is on the version that froze it, once one is published).
	 */
	const here = (f: Feedback) => (doc.version === null ? f.deliverableVersion === null : f.givenOn === doc.version);
	/** Feedback on a passage of what is open now: the live file, or the version being read. */
	const anchored = $derived(doc.feedback.filter((f) => f.quote && here(f)));
	const marks = $derived<Mark[]>([
		...anchored.map((f) => ({ id: f.id, anchor: anchorFor(f), tone: f.status })),
		...(pending ? [{ id: 'new', anchor: pending, tone: 'new' as const }] : [])
	]);
	/** The feedback whose passage is no longer in the text. */
	const lost = $derived.by(() => {
		if (!isMarkdown || !anchored.length) return new Set<string>();
		const text = plain(snapshot ? (doc.body ?? '') : body, true);
		return new Set(anchored.filter((f) => !locate(text, anchorFor(f))).map((f) => f.id));
	});

	function commentOn(anchor: Anchor) {
		pending = anchor;
		void side?.reveal('feedback');
	}

	/** A lit-up passage was clicked: show its feedback. */
	const showFeedback = (id: string) => void side?.reveal('feedback', id);

	// A [[link]] to a piece of feedback opens this page with `?feedback=<id>`: its window opens at that piece, which flashes.
	// The address is cleaned up afterwards, so following the same link again does it again.
	const wantedFeedback = $derived(page.url.searchParams.get('feedback'));
	$effect(() => {
		const id = wantedFeedback;
		if (!id || !side) return;
		untrack(() => {
			void side?.reveal('feedback', id);
			const url = new URL(page.url);
			url.searchParams.delete('feedback');
			replaceState(url, page.state);
		});
	});

	/** Feedback's passage, shown in the document. */
	const showPassage = (id: string) => (snapshot || mode === 'reading' ? reader : editor)?.reveal(id);

	// ---- the live file against the last version ------------------------------------------------
	// What the backend says (`provenance.draft`) is true as the page loads. Once you type, the text is compared with the last
	// version's right here, so "same as" turns into "draft" with the first change, and back when you undo it.

	const latest = $derived(doc.versions.at(-1) ?? null);
	const latestSha = $derived(published.find((v) => v.version === latest)?.sha256 ?? null);
	let sameAsLatest = $state<boolean | null>(null);
	$effect(() => {
		const text = body;
		const sha = latestSha;
		if (!touched || !isMarkdown || snapshot || !sha) {
			sameAsLatest = null;
			return;
		}
		let stale = false;
		void sha256Hex(text).then((hash) => {
			if (!stale) sameAsLatest = hash === sha;
		});
		return () => (stale = true);
	});
	const draft = $derived<ProvenanceDraft | null>(
		sameAsLatest === null ? provenance.draft : sameAsLatest ? null : { since: latest, modified: editedAt, stoppedRun: provenance.draft?.stoppedRun ?? null }
	);
</script>

<svelte:head><title>{doc.name} · Ledger</title></svelte:head>

<div class="header-wrap flat">
	<div class="crumbs">
		<a href="/projects/{project}">{data.project.name}</a><span>/</span><a href="/projects/{project}/deliverables">Deliverables</a>
	</div>
	<div class="header-top">
		<div style="flex:1;min-width:0">
			<h1 title={doc.path}>
				{doc.name}
				<span class="thread-box deliverable-pill" title="The deliverable this page is about, and which of its versions is open">
					<FileText size={15} />
					<span class="tb-label">Deliverable</span>
					<span class="db-value">{snapshot ? `v${doc.version}` : 'Live'}</span>
				</span>
			</h1>
			<div class="subline">
				<span class="badge type">{doc.kind === 'markdown' ? 'Markdown' : doc.kind === 'image' ? 'Image' : 'File'}</span>
				{#if snapshot}
					<span class="badge published">v{doc.version}</span>
				{:else if draft || !latest}
					<span class="badge draft">{draft?.since ? `Draft after v${draft.since}` : 'Draft'}</span>
				{:else}
					<span class="badge published">Same as v{latest}</span>
				{/if}
				<span>{formatSize(doc.size)}</span>
				{#if provenance.summary.label}<MmmBadge level={provenance.summary.label.level} />{/if}
				{#if doc.path.includes('/')}<span class="mono">{doc.path}</span>{/if}
				{#if isMarkdown && !snapshot && doc.exists}
					<span class="save-state">{saveState === 'saved' ? 'Saved' : saveState === 'saving' ? 'Saving…' : 'Unsaved changes'}</span>
				{/if}
			</div>
		</div>
		<div class="header-actions">
			<div class="thread-box" title="Notes on publishing, new feedback and the prompts made from feedback go to this thread">
				<MessagesSquare size={15} />
				<span class="tb-label">Thread</span>
				{#if threadOptions.length}
					<Dropdown bind:value={workThread} options={threadOptions} label="Thread for this page" align="end" />
				{:else}
					<button class="link accent" onclick={() => newThreadFlow(project)}>Create one</button>
				{/if}
			</div>
			{#if !snapshot && doc.exists}
				<button class="btn primary" onclick={startPublishing}>Publish version…</button>
			{/if}
			{#if more.length}
				<Menu class="btn ghost icon-only" label="More actions" items={more} onpick={doMore} align="end"><Ellipsis size={18} /></Menu>
			{/if}
		</div>
	</div>

</div>

<div class="doc-body">
<div class="page">
	<div class="page-inner wide" bind:this={docArea}>
		{#if snapshot}
			<div class="notice info">
				Published version {doc.version}, {dateTime(doc.modified)} — read-only.
				{#if doc.exists}
					<a href={deliverableUrl(project, doc.basePath)}>Back to the live file</a>
				{/if}
			</div>
		{/if}

		{#if !doc.exists && !snapshot}
			<div class="notice">The file is not in the project’s deliverables folder (any more).</div>
		{:else if isMarkdown && snapshot}
			<div class="doc-view"><Markdown bind:this={reader} html={doc.html ?? ''} {images} {marks} onmark={showFeedback} /></div>
		{:else if isMarkdown}
			<div class="form-row" style="justify-content:space-between">
				<div class="seg" role="group" aria-label="View">
					{#each modes as m (m.id)}
						<button class:on={mode === m.id} aria-pressed={mode === m.id} onclick={() => setMode(m.id)}><m.icon size={14} />{m.label}</button>
					{/each}
				</div>
				<span class="hint">Last changed {ago(doc.modified)}. Changes save automatically.</span>
			</div>
			{#if mode === 'reading'}
				<div class="doc-view">
					{#if body.trim()}<Markdown bind:this={reader} {html} {images} {marks} onmark={showFeedback} />{:else}<span class="hint">Nothing here yet.</span>{/if}
				</div>
			{:else}
				<MarkdownEditor
					bind:this={editor}
					bind:value={body}
					variant="doc"
					minHeight="58vh"
					label={doc.name}
					placeholder="Write it in markdown…"
					source={mode === 'source'}
					{images}
					{marks}
					onmark={showFeedback}
					oninput={edited}
				/>
			{/if}
		{:else if doc.kind === 'image'}
			<div class="doc-view" style="display:grid;place-items:center">
				<FileImage {project} path={file} alt={doc.name} cls="shot-preview" />
			</div>
		{:else}
			<div class="panel">
				<h2>{doc.name}</h2>
				<p class="sub" style="margin:0">
					This kind of file can’t be shown here. Open it with the program your system uses for it — it opens the file in the vault, so changes you
					save there are picked up when you come back.
				</p>
				<div class="row-end" style="justify-content:flex-start">
					<button class="btn primary" onclick={() => open(false)}>Open</button>
					<button class="btn" onclick={() => open(true)}>Show in folder</button>
				</div>
			</div>
		{/if}
	</div>
</div>

<SelectionComment root={docArea} oncomment={commentOn} />
<DocSide bind:this={side} {project} {doc} {provenance} {draft} prompts={data.prompts} {threads} thread={workThread} {threadTitle} bind:pending {lost} {here} onlocate={showPassage} />
</div>

{#if publishing}
	<Modal title="Publish a version" onclose={() => (publishing = false)}>
		<p style="margin:0;color:var(--muted)">
			Publishing freezes the current file as an immutable version (<code>{snapshotPath(doc.path, bump === 'custom' ? custom || 'x.y' : preview[bump])}</code>),
			records who made the change{#if workThread}, and leaves a note in <code>{threadTitle}</code>{/if}.
			{doc.versions.length ? `Latest so far: v${doc.versions.at(-1)}.` : 'This is the first version.'}
		</p>
		<label class="check"><input type="radio" bind:group={bump} value="minor" /> Minor — v{preview.minor}</label>
		<label class="check"><input type="radio" bind:group={bump} value="major" /> Major — v{preview.major}</label>
		<label class="check">
			<input type="radio" bind:group={bump} value="custom" /> Custom
			<input
				bind:value={custom}
				placeholder="e.g. 1.4"
				style="width:100px;padding:5px 8px;border-radius:8px;border:1px solid var(--line);background:var(--bg)"
				onfocus={() => (bump = 'custom')}
			/>
		</label>

		<hr class="rule" />
		<div class="field">
			<label class="check">
				<input type="checkbox" bind:checked={aiMade} /> This change was made with generative AI
			</label>
			<span class="hint">
				{#if stopped}
					Claude Code’s run of “{stopped.promptTitle}” changed this file{stopped.status === 'finished' ? '' : ` and was ${stopped.status}`}, and no version holds
					its changes, so this starts ticked. Untick it only if you rewrote them yourself.
				{:else}
					Leave it empty for what you wrote yourself. Runs started from Ledger are recorded automatically; tick this for AI used elsewhere (a chat, an editor assistant, a paste).
				{/if}
			</span>
		</div>
		{#if aiMade && doc.prompts.length}
			<label class="field">
				<span>Which prompt asked for it? (optional)</span>
				<select bind:value={promptId}>
					<option value="">None of them</option>
					{#each doc.prompts as p (p.id)}<option value={p.id}>{p.threadTitle} · {p.title || 'Untitled prompt'}</option>{/each}
				</select>
			</label>
		{/if}
		<label class="field">
			<span>AI-use label for the result (optional)</span>
			<select bind:value={level}>
				<option value="">Not labelled</option>
				{#each MMM as m (m.level)}<option value={m.level}>{m.name} — {m.tagline}</option>{/each}
			</select>
			{#if level}
				<span class="hint">{MMM.find((m) => m.level === level)?.meaning}</span>
			{/if}
			{#if manMadeConflict}
				<span class="pv-warn">Man-Made means no generative AI, but AI is recorded {aiMade ? 'for this change' : 'in this file’s history'}.</span>
			{:else if suggestManMade}
				<span class="hint">
					Nothing here is recorded as AI. <button class="link accent" onclick={() => (level = 'man-made')}>Label it Man-Made</button> if that is true.
				</span>
			{/if}
		</label>
		<div class="field">
			<span>Changelog (optional)</span>
			<MarkdownEditor bind:value={changeNote} rows={2} label="Changelog" placeholder="What changed, in your words" />
			<span class="hint">Quoted in the note in the thread, and shown with the version in the Produced by window.</span>
		</div>
		{#snippet footer()}
			<span class="spacer-h"></span>
			<button class="btn ghost" onclick={() => (publishing = false)}>Cancel</button>
			<button class="btn primary" onclick={publish} disabled={publishBusy || (bump === 'custom' && !custom.trim())}>Publish</button>
		{/snippet}
	</Modal>
{/if}
