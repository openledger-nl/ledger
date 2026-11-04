<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import X from '@lucide/svelte/icons/x';
	import { tick } from 'svelte';
	import { api } from '$lib/api';
	import type { Anchor } from '$lib/anchors';
	import { deliverableUrl, promptName, snapshotPath, threadUrl } from '$lib/actions';
	import { date, dayKey } from '$lib/format';
	import { orderFeedback, orderVersions } from '$lib/order';
	import { listOrder } from '$lib/order.svelte';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { Deliverable, Feedback, Prompt, ThreadSummary } from '$lib/types';
	import FeedbackCard from './FeedbackCard.svelte';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import Menu, { type MenuItem } from './Menu.svelte';
	import PromptModal from './PromptModal.svelte';

	// What was said about the file: the box to say more, and the list, by thread and, within each, by version (the version
	// each piece was given on). New feedback goes to the page's thread (`thread`, chosen at the top of the page), on the
	// whole document or, with `pending`, on the passage selected in it. Open feedback can be selected, and the selected ones made
	// into a prompt, or added to one (`prompts` are the ones about this file). `lost`: the feedback whose passage is no longer
	// in the text; `onlocate` shows a passage in the document; `here` says whether a piece belongs to what the page shows.
	let {
		project,
		doc,
		prompts,
		threads,
		thread,
		threadTitle,
		pending = $bindable(null),
		lost,
		here,
		onlocate,
		onopenprompts
	}: {
		project: string;
		doc: Deliverable;
		prompts: Prompt[];
		threads: ThreadSummary[];
		thread: string;
		threadTitle: string;
		pending?: Anchor | null;
		lost: Set<string>;
		here: (f: Feedback) => boolean;
		onlocate: (id: string) => void;
		/** Show the Prompts window (a prompt was made or added to). */
		onopenprompts: () => void;
	} = $props();

	const snapshot = $derived(doc.version !== null);
	const titleOf = (slug: string) => threads.find((t) => t.slug === slug)?.title;

	let text = $state('');
	let busy = $state(false);
	let editor: MarkdownEditor | undefined = $state();

	// A passage was selected in the document: the box is where to go next.
	$effect(() => {
		if (pending) void tick().then(() => editor?.focus());
	});

	async function add() {
		busy = true;
		const added = await attempt(() => api.addFeedback(project, thread, doc.path, text, pending));
		busy = false;
		if (!added) return;
		text = '';
		pending = null;
		await refresh();
	}

	// ---- the list ------------------------------------------------------------------------------
	// Like the Produced by window: a header per thread, and under it a header per version. The live file is the newest
	// version, so it comes first when newest comes first, and last otherwise.

	interface VersionGroup {
		version: string | null;
		items: Feedback[];
	}
	interface ThreadGroup {
		slug: string;
		title: string;
		first: string;
		last: string;
		versions: VersionGroup[];
	}

	const groups = $derived.by<ThreadGroup[]>(() => {
		const order = listOrder.order;
		const byThread = new Map<string, Feedback[]>();
		for (const f of doc.feedback) byThread.set(f.threadSlug, [...(byThread.get(f.threadSlug) ?? []), f]);
		const out = [...byThread].map(([slug, items]) => {
			const stamps = items.map((f) => f.createdAt).sort();
			const given = [...new Set(items.map((f) => f.givenOn))];
			const numbered = orderVersions(given.filter((v): v is string => v !== null), order);
			const keys: (string | null)[] = given.includes(null) ? (order === 'desc' ? [null, ...numbered] : [...numbered, null]) : numbered;
			return {
				slug,
				title: titleOf(slug) ?? slug,
				first: stamps[0],
				last: stamps[stamps.length - 1],
				versions: keys.map((version) => ({ version, items: orderFeedback(items.filter((f) => f.givenOn === version), order) }))
			};
		});
		return out.sort((a, b) => (order === 'desc' ? b.last.localeCompare(a.last) : a.first.localeCompare(b.first)));
	});

	const span = (g: ThreadGroup) => (dayKey(g.first) === dayKey(g.last) ? date(g.first) : `${date(g.first)} – ${date(g.last)}`);
	const isOpen = (f: Feedback) => f.status === 'open';
	const versionUrl = (version: string | null) => deliverableUrl(project, version ? snapshotPath(doc.basePath, version) : doc.basePath);
	const promptsOf = (f: Feedback) => prompts.filter((p) => p.feedback.includes(f.id)).map((p) => p.title || 'Untitled prompt');

	// ---- selecting feedback for a prompt -------------------------------------------------------

	let selected = $state<string[]>([]);
	const picked = $derived(doc.feedback.filter((f) => selected.includes(f.id)));

	// Feedback that was resolved or deleted meanwhile is no longer selected.
	$effect(() => {
		const stay = selected.filter((id) => doc.feedback.some((f) => f.id === id && isOpen(f)));
		if (stay.length !== selected.length) selected = stay;
	});

	const toggle = (id: string) => (selected = selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id]);
	const selectOpen = (items: Feedback[]) => (selected = [...new Set([...selected, ...items.filter(isOpen).map((f) => f.id)])]);

	let creating = $state(false);
	const promptItems = $derived<MenuItem[]>(prompts.map((p) => ({ id: p.id, label: promptName(p), sub: titleOf(p.threadSlug) })));

	async function addToPrompt(id: string) {
		const prompt = prompts.find((p) => p.id === id);
		if (!prompt) return;
		const done = await attempt(() => api.attachPromptFeedback(project, prompt.threadSlug, prompt.id, picked.map((f) => f.id)));
		if (!done) return;
		toast(`Added ${picked.length} ${picked.length === 1 ? 'piece' : 'pieces'} of feedback to the prompt`);
		selected = [];
		await refresh();
		onopenprompts();
	}
</script>

<div class="field">
	<span>
		{#if pending}On the selected text{:else if snapshot}On version {doc.version}, as a whole{:else}On the whole document{/if}
	</span>
	{#if !thread}
		<span class="hint">Create a thread first — feedback is written in one.</span>
	{:else}
		{#if pending}
			<div class="quote passage pending">
				<span class="qhead">Selected text</span>
				<span class="qtext">“{pending.text}”</span>
				<button class="icon-btn" title="Comment on the whole document instead" aria-label="Comment on the whole document instead" onclick={() => (pending = null)}>
					<X size={14} />
				</button>
			</div>
		{/if}
		<MarkdownEditor
			bind:this={editor}
			bind:value={text}
			label="Feedback"
			placeholder="What works, what doesn’t, what to change…"
			onsubmit={() => text.trim() && add()}
		/>
		<div class="row-end" style="align-items:center">
			<span class="hint" style="margin-right:auto">In the thread “{threadTitle}”</span>
			<button class="btn small primary" onclick={add} disabled={busy || !text.trim()}>Add feedback</button>
		</div>
		{#if !pending}
			<span class="hint">To comment on a passage, select it in the document and click the comment icon beside it.</span>
		{/if}
	{/if}
</div>

{#each groups as group (group.slug)}
	<section class="pv-group">
		<div class="pv-thread">
			<a href={threadUrl(project, group.slug)} title="Open the thread">{group.title}</a>
			<span class="pv-meta">{span(group)}</span>
		</div>
		{#each group.versions as v (v.version ?? 'live')}
			<div class="fb-version">
				<a class="vchip" class:on={doc.version === v.version} href={versionUrl(v.version)}>{v.version ? `v${v.version}` : 'Live'}</a>
				<span class="pv-meta">{v.items.filter(isOpen).length} open · {v.items.length} in all</span>
				{#if v.items.some(isOpen)}<button class="link accent" onclick={() => selectOpen(v.items)}>Select open</button>{/if}
			</div>
			<div class="fb-list">
				{#each v.items as f (f.id)}
					<FeedbackCard
						{project}
						thread={f.threadSlug}
						feedback={f}
						onDeliverable
						version={f.givenOn}
						threadTitle={titleOf(f.threadSlug)}
						promptThread={thread || undefined}
						lost={lost.has(f.id)}
						onlocate={here(f) ? onlocate : undefined}
						selected={selected.includes(f.id)}
						onselect={isOpen(f) ? toggle : undefined}
						inPrompts={promptsOf(f)}
						onprompted={onopenprompts}
					/>
				{/each}
			</div>
		{/each}
	</section>
{:else}
	<div class="pv-hint">No feedback on this file yet.</div>
{/each}

{#if picked.length}
	<div class="fb-bar">
		<span><b>{picked.length}</b> selected</span>
		<button class="link" onclick={() => (selected = [])}>Clear</button>
		<span class="spacer-h"></span>
		{#if prompts.length}
			<Menu class="btn small ghost" label="Add the selected feedback to a prompt" items={promptItems} onpick={addToPrompt} align="end">
				Add to prompt<ChevronDown size={12} />
			</Menu>
		{/if}
		<button class="btn small primary" disabled={!thread} onclick={() => (creating = true)}>New prompt</button>
	</div>
{/if}

{#if creating}
	<PromptModal
		{project}
		{thread}
		prompt={null}
		deliverable={doc.basePath}
		feedback={selected}
		onsaved={() => {
			selected = [];
			onopenprompts();
		}}
		onclose={() => (creating = false)}
	/>
{/if}
