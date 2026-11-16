<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import BookOpen from '@lucide/svelte/icons/book-open';
	import CodeXml from '@lucide/svelte/icons/code-xml';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import FileText from '@lucide/svelte/icons/file-text';
	import Paperclip from '@lucide/svelte/icons/paperclip';
	import PencilLine from '@lucide/svelte/icons/pencil-line';
	import TextCursorInput from '@lucide/svelte/icons/text-cursor-input';
	import Trash from '@lucide/svelte/icons/trash';
	import { goto, replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import Dropdown from '$lib/components/Dropdown.svelte';
	import DocSide from '$lib/components/DocSide.svelte';
	import EvidenceDescription from '$lib/components/EvidenceDescription.svelte';
	import FileImage from '$lib/components/FileImage.svelte';
	import Markdown from '$lib/components/Markdown.svelte';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import Menu, { type MenuItem } from '$lib/components/Menu.svelte';
	import MmmBadge from '$lib/components/MmmBadge.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import SelectionComment from '$lib/components/SelectionComment.svelte';
	import { api, portflowApi } from '$lib/api';
	import { portflow } from '$lib/portflow.svelte';
	import { deliverableFile, deliverableUrl, nextVersion, snapshotPath } from '$lib/actions';
	import { locate, plain, type Anchor, type Mark } from '$lib/anchors';
	import { sha256Hex } from '$lib/hash';
	import { dateTime } from '$lib/format';
	import { MMM, mmmInfo } from '$lib/mmm';
	import MmmPicker from '$lib/components/MmmPicker.svelte';
	import { dirOf } from '$lib/vaultImages';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import { beforeRestart } from '$lib/updates.svelte';
	import type { Bump, Evidence, Feedback, MmmLevel, ProvenanceDraft, ProvenanceVersion } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const doc = $derived(data.deliverable);
	const provenance = $derived(data.provenance);
	const project = $derived(data.project.slug);
	const threads = $derived(data.projects.find((p) => p.slug === project)?.threads ?? []);
	const snapshot = $derived(doc.version !== null);
	const isMarkdown = $derived(doc.kind === 'markdown');
	const file = $derived(deliverableFile(doc.path));
	/** How the deliverable reads: its name without the file's extension (an evidence item's name is shown as it is). */
	const title = $derived(doc.evidence ? doc.name : doc.name.replace(/\.[^./]+$/, '') || doc.name);
	// Pictures in it are looked for beside the live file, also in a version (which is kept in versions/, apart from them).
	const images = $derived({ project, dir: dirOf(deliverableFile(doc.basePath)) });

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
	// One thread for everything done here: the note a publish leaves, new feedback, and the prompts made from feedback. It
	// isn't chosen: it is the thread the deliverable belongs to. That is its evidence item's thread, else where its last
	// version went, else the thread of the prompt that asked for it, else the thread it was opened from (`?thread=<slug>`),
	// else the project's first thread.

	let workThread = $state('');

	const cameFrom = $derived(page.url.searchParams.get('thread'));
	$effect(() => {
		const options = threads.map((t) => t.slug);
		workThread =
			[evidence?.thread, provenance.defaultThread, doc.prompts[0]?.threadSlug, cameFrom].find((slug) => slug && options.includes(slug)) ??
			options[0] ??
			'';
	});

	const threadTitle = $derived(threads.find((t) => t.slug === workThread)?.title ?? '');

	// ---- the evidence item it is -----------------------------------------------------------------
	// A deliverable that is a Portflow evidence item shows the item's description (What? / Goal? / Why it contributes?)
	// under the document, to read and edit here rather than in the thread.

	let evidence = $state<Evidence | null>(null);
	const isEvidenceFile = (e: Evidence) => e.files.some((f) => f.path === doc.basePath || f.path === `deliverables/${doc.basePath}`);
	$effect(() => {
		const wanted = doc.evidence;
		const slug = project;
		void doc.basePath;
		evidence = null;
		if (!wanted || !portflow.enabled) return;
		let stale = false;
		portflowApi
			.project(slug)
			.then((view) => {
				if (!stale) evidence = view.evidence.find(isEvidenceFile) ?? view.evidence.find((e) => e.name === wanted) ?? null;
			})
			.catch(() => {});
		return () => (stale = true);
	});
	const evidenceSaved = (items: Evidence[]) => (evidence = items.find((e) => e.slug === evidence?.slug) ?? evidence);

	// ---- renaming ------------------------------------------------------------------------------
	// The live file gets another name (its extension stays) and its versions and feedback follow; the page moves along. An
	// evidence item is named after the item in Portflow, so it is renamed there.

	let renaming = $state(false);
	let newName = $state('');
	let renameBusy = $state(false);

	function startRenaming() {
		newName = title;
		renaming = true;
	}

	async function rename() {
		if (!newName.trim() || renameBusy) return;
		renameBusy = true;
		await flush();
		const to = await attempt(() => api.renameDeliverable(project, doc.path, newName.trim()));
		renameBusy = false;
		if (!to) return;
		renaming = false;
		dirty = false;
		await refresh();
		const url = new URL(deliverableUrl(project, to), page.url);
		if (cameFrom) url.searchParams.set('thread', cameFrom);
		await goto(url.pathname + url.search, { replaceState: true });
	}

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

	// Wireframe D5: the project's intended label is offered, a study project's version needs a label, and a
	// label other than the intended one asks you to confirm it honestly. It never blocks: confirm or change it.
	const projectMeta = $derived(data.meta);
	const deliverableMeta = $derived(data.deliverableMeta);
	const intended = $derived(projectMeta?.intendedMmm ?? null);
	const labelRequired = $derived(projectMeta?.kind === 'study');
	const differs = $derived(!!intended && !!level && level !== intended);
	let confirmed = $state(false);
	let confirmReason = $state('');
	$effect(() => {
		void level;
		confirmed = false;
	});
	const publishBlocked = $derived((labelRequired && !level) || (differs && !confirmed));

	function startPublishing() {
		bump = 'minor';
		custom = '';
		aiMade = stopped !== null;
		level = intended ?? provenance.suggestedLabel ?? '';
		confirmReason = '';
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
		if (differs && intended && level) {
			await attempt(() =>
				api.confirmLabel({
					project,
					path: doc.path,
					version: published.versions.at(-1) ?? '',
					intended,
					chosen: level as MmmLevel,
					reason: confirmReason.trim(),
					at: new Date().toISOString()
				})
			);
		}
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
		// Copy, Open and Show in folder are buttons of their own beside it.
		...(!snapshot && doc.exists && !doc.evidence ? [{ id: 'rename', label: 'Rename…', icon: TextCursorInput }] : []),
		...(!snapshot && doc.exists ? [{ id: 'delete', label: 'Delete deliverable', icon: Trash, danger: true, rule: true }] : [])
	]);

	function doMore(id: string) {
		if (id === 'copy') void copyMarkdown();
		else if (id === 'open') void open(false);
		else if (id === 'reveal') void open(true);
		else if (id === 'rename') startRenaming();
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

<svelte:head><title>{title} · Ledger</title></svelte:head>

<!-- No tabs under it, so it keeps its own space below the buttons (`flat`), like the other pages' headers. -->
<div class="header-wrap flat">
	<div class="header-top">
		<div style="flex:1;min-width:0">
			<h1 title={doc.path}>
				{title}
				<span class="thread-box deliverable-pill" title="The deliverable this page is about, and which of its versions is open">
					<FileText size={15} />
					<span class="tb-label">Deliverable</span>
					<span class="db-value">{snapshot ? `v${doc.version}` : 'Live'}</span>
				</span>
			</h1>
			{#if deliverableMeta}
				<p class="pj-goal"><span><strong>Goal</strong> {deliverableMeta.goal}</span></p>
			{/if}
			{#if doc.attachments.length}
				<p class="doc-attachments">
					<Paperclip size={13} />
					<span>With it in Portflow:</span>
					{#each doc.attachments as file (file)}
						<a href={deliverableUrl(project, file)} class="mono">{file.split('/').at(-1)}</a>
					{/each}
				</p>
			{/if}
		</div>
		<div class="header-actions">
			{#if isMarkdown}<button class="btn ghost small" onclick={copyMarkdown}>Copy markdown</button>{/if}
			{#if doc.exists}
				<button class="btn ghost small" onclick={() => open(false)}>Open</button>
				<button class="btn ghost small" onclick={() => open(true)}>Show in folder</button>
			{/if}
			{#if !snapshot && doc.exists}
				<button class="btn primary small" onclick={startPublishing}>Publish version</button>
			{/if}
			{#if more.length}
				<Menu class="btn ghost small icon-only" label="More actions" items={more} onpick={doMore} align="end"><Ellipsis size={16} /></Menu>
			{/if}
		</div>
	</div>

</div>

<div class="doc-body">
<div class="page">
	<div class="page-inner" bind:this={docArea}>
		{#if snapshot}
			<div class="notice info">
				Published version {doc.version}, {dateTime(doc.modified)} — read-only.
				{#if doc.exists}
					<a href={deliverableUrl(project, doc.basePath)}>Back to the live file</a>
				{/if}
			</div>
		{/if}

		{#if !doc.exists && !snapshot && doc.versions.length}
			{@const newest = doc.versions[doc.versions.length - 1]}
			<div class="notice">
				There is no working copy of this file, only its published versions.
				<a href={deliverableUrl(project, snapshotPath(doc.basePath, newest))}>Open version {newest}</a>
			</div>
		{:else if !doc.exists && !snapshot}
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

		{#if evidence}
			<section class="card doc-evidence" aria-label="Evidence description">
				<EvidenceDescription {evidence} onsaved={evidenceSaved} />
			</section>
		{/if}
	</div>
</div>

<SelectionComment root={docArea} oncomment={commentOn} />
<DocSide bind:this={side} {project} {doc} {provenance} {draft} prompts={data.prompts} {threads} bind:thread={workThread} {threadTitle} bind:pending {lost} {here} onlocate={showPassage} />
</div>

{#if renaming}
	<Modal title="Rename the deliverable" onclose={() => (renaming = false)}>
		<label class="field">
			Name
			<!-- svelte-ignore a11y_autofocus -->
			<input bind:value={newName} autofocus onkeydown={(e) => e.key === 'Enter' && !e.isComposing && rename()} />
			<span class="hint">Its versions and feedback keep belonging to it.</span>
		</label>
		{#snippet footer()}
			<span class="spacer-h"></span>
			<button class="btn ghost" onclick={() => (renaming = false)}>Cancel</button>
			<button class="btn primary" onclick={rename} disabled={renameBusy || !newName.trim()}>Rename</button>
		{/snippet}
	</Modal>
{/if}

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
			<div class="field">
				<span>Which prompt asked for it? (optional)</span>
				<Dropdown
					bind:value={promptId}
					options={[{ value: '', label: 'None of them' }, ...doc.prompts.map((p) => ({ value: p.id, label: `${p.threadTitle} · ${p.title || 'Untitled prompt'}` }))]}
					label="Which prompt asked for it"
					full
				/>
			</div>
		{/if}
		<div class="field">
			<span>AI-use label for the result {labelRequired ? '' : '(optional)'}{#if labelRequired}<span class="req">required in a study project</span>{/if}</span>
			<MmmPicker bind:value={level} label="AI-use label for the result" empty="Not labelled" />
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
		</div>
		{#if differs && intended && level}
			<div class="pub-confront">
				<div class="pub-confront-head">
					<span>This project is meant to be</span>
					<MmmBadge level={intended} />
					<span>and you chose</span>
					<MmmBadge level={level} />
				</div>
				<p class="hint" style="margin:0">{mmmInfo(level)?.meaning} Keep it if it is true; your answer is kept with the version.</p>
				<label class="check"><input type="checkbox" bind:checked={confirmed} /> I choose {mmmInfo(level)?.name} honestly for this version</label>
				<input class="pub-reason" bind:value={confirmReason} placeholder="Why, in your own words (optional)" />
				<button class="link accent" style="align-self:flex-start" onclick={() => (level = intended ?? '')}>Use {mmmInfo(intended)?.name} instead</button>
			</div>
		{:else if labelRequired && !level}
			<p class="pv-warn" style="margin:0">A study project's published version needs an AI-use label.</p>
		{/if}
		<div class="field">
			<span>Changelog (optional)</span>
			<MarkdownEditor bind:value={changeNote} rows={2} label="Changelog" placeholder="What changed, in your words" />
			<span class="hint">Quoted in the note in the thread, and shown with the version in the Produced by window.</span>
		</div>
		{#snippet footer()}
			<span class="spacer-h"></span>
			<button class="btn ghost" onclick={() => (publishing = false)}>Cancel</button>
			<button class="btn primary" onclick={publish} disabled={publishBusy || (bump === 'custom' && !custom.trim()) || publishBlocked}>Publish</button>
		{/snippet}
	</Modal>
{/if}

<style>
	/* The document takes nearly all the room beside the side window: a narrow margin instead of the page's usual one. */
	.doc-body > .page {
		padding: 14px 16px;
	}
	/* Writing on white (the card colour: white in the light themes, the card's own in a dark one). */
	.page-inner :global(.md-editor.md-doc) {
		background: var(--field);
	}
	.doc-evidence {
		padding: 14px 18px;
	}
</style>
