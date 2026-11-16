<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$lib/api';
	import { openInPortflow } from '$lib/portflow.svelte';
	import { baseName, deliverableUrl, liveDeliverable, threadUrl } from '$lib/actions';
	import { time } from '$lib/format';
	import { attempt, confirmDialog, refresh } from '$lib/ui.svelte';
	import type { Feedback, FeedbackFrom } from '$lib/types';
	import Markdown from './Markdown.svelte';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import PersonPicker from './PersonPicker.svelte';
	import PromptModal from './PromptModal.svelte';

	/**
	 * `onDeliverable`: shown on the deliverable's own page, where naming the deliverable would only be noise. There, the
	 * header says which version it was given on (`version`: null for the live file, as it stands) and in which thread
	 * (`threadTitle`); `promptThread` is the thread a prompt made from it goes to (the page's own, rather than the
	 * feedback's); `onlocate` shows the passage it is about in the document, and `lost` says that passage is no longer in it.
	 * `onselect` (open feedback only) puts a box on it to pick it for a prompt; `inPrompts` names the prompts it is in already.
	 * `replies`: answers to it in Portflow, shown inside it. `evidenceName`: the evidence item it is on; `portflowItem`:
	 * what it is on in Portflow, to say where to go there.
	 */
	let {
		project,
		thread,
		feedback,
		onDeliverable = false,
		version = null,
		threadTitle,
		promptThread,
		onlocate,
		lost = false,
		selected = false,
		onselect,
		inPrompts = [],
		onprompted,
		replies = [],
		evidenceName = null,
		portflowItem = null
	}: {
		project: string;
		thread: string;
		feedback: Feedback;
		onDeliverable?: boolean;
		version?: string | null;
		threadTitle?: string;
		promptThread?: string;
		onlocate?: (id: string) => void;
		lost?: boolean;
		selected?: boolean;
		onselect?: (id: string) => void;
		inPrompts?: string[];
		/** A prompt made from this feedback, on the deliverable's page: what to do about it (rather than going to its thread). */
		onprompted?: () => void;
		replies?: Feedback[];
		evidenceName?: string | null;
		/** What it is on in Portflow, to say where to go there ("“Project plan”, version 2"). */
		portflowItem?: string | null;
	} = $props();

	let editing = $state(false);
	let draft = $state('');
	let from = $state<FeedbackFrom>({ name: null, portflowUser: null, role: null, given: null });
	/** Pulled from Portflow: Portflow says what it says and who said it. */
	const fromPortflow = $derived(feedback.portflowComment !== null);
	const GIVEN: Record<string, string> = { 'in person': 'in person', meeting: 'in a meeting', portflow: 'in Portflow', email: 'by email', phone: 'by phone', other: '' };
	let saving = $state(false);
	let prompting = $state(false);

	const resolved = $derived(feedback.status === 'resolved');
	/** Every piece (this one and each reply) is resolved on its own; the card counts as resolved only when all are. */
	const pieces = $derived([feedback, ...replies]);
	const openPieces = $derived(pieces.filter((f) => f.status !== 'resolved').length);
	const allResolved = $derived(openPieces === 0);
	const kind = $derived(
		allResolved ? 'Feedback · resolved' : replies.length && openPieces < pieces.length ? `Feedback · ${openPieces} of ${pieces.length} open` : 'Feedback'
	);

	async function toggle() {
		const next = resolved ? 'open' : 'resolved';
		if (await attempt(() => api.setFeedbackStatus(project, thread, feedback.id, next))) await refresh();
	}

	/** Resolves (or reopens) the whole card: this piece and every reply. */
	async function toggleAll() {
		const next = allResolved ? 'open' : 'resolved';
		if (await attempt(() => api.setFeedbackStatus(project, thread, feedback.id, next, true))) await refresh();
	}

	/** A reply (an expert's answer in Portflow) is resolved on its own. */
	async function toggleReply(reply: Feedback) {
		const next = reply.status === 'resolved' ? 'open' : 'resolved';
		if (await attempt(() => api.setFeedbackStatus(project, thread, reply.id, next))) await refresh();
	}

	function startEdit() {
		draft = feedback.body;
		from = { name: feedback.from, portflowUser: feedback.fromUser, role: feedback.fromRole, given: feedback.given };
		editing = true;
	}

	async function save() {
		if (!draft.trim()) return;
		saving = true;
		const done =
			(await attempt(() => api.updateFeedback(project, thread, feedback.id, draft))) &&
			(await attempt(() => api.setFeedbackFrom(project, thread, feedback.id, from)));
		saving = false;
		if (!done) return;
		editing = false;
		await refresh();
	}

	async function remove() {
		const ok = await confirmDialog({ title: 'Delete this feedback?', confirmLabel: 'Delete', danger: true });
		if (!ok) return;
		if ((await attempt(() => api.deleteFeedback(project, thread, feedback.id))) !== undefined) await refresh();
	}
</script>

<div class="card feedback-card" class:resolved={allResolved} id="f-{feedback.id}">
	<div class="fb-head">
		<div class="meta">
			{#if onselect}
				<input class="fb-pick" type="checkbox" checked={selected} aria-label="Select this feedback for a prompt" title="Select it for a prompt" onchange={() => onselect(feedback.id)} />
			{/if}
			<span class="card-kind">{kind}</span>
			{#if feedback.from}
				<span class="from"
					>from <strong>{feedback.from}</strong>{#if feedback.fromRole}<span class="badge">{feedback.fromRole}</span>{/if}{#if feedback.given && GIVEN[feedback.given]}
						{GIVEN[feedback.given]}{/if}</span
				>
			{/if}
			<span class="time">{time(feedback.createdAt)}</span>
			{#if inPrompts.length}<span class="badge accent" title="In: {inPrompts.join(', ')}">In a prompt</span>{/if}
		</div>
		<div class="meta file">
			{#if feedback.evidence}
				<span>on {evidenceName ?? feedback.evidence}{feedback.evidenceVersion ? ` v${feedback.evidenceVersion}` : ''}</span>
			{/if}
			{#if !feedback.deliverablePath}
				<!-- Feedback on an evidence item whose files Ledger doesn't have names no deliverable. -->
			{:else if onDeliverable}
				<span>on {version ? `v${version}` : 'the live file'}</span>
				<span>·</span>
				<span>in <a href="/projects/{project}/threads/{thread}?item=f-{feedback.id}">{threadTitle ?? 'its thread'}</a></span>
			{:else}
				<span>on <a href={deliverableUrl(project, feedback.deliverablePath)}>{baseName(feedback.deliverablePath)}</a></span>
			{/if}
		</div>
	</div>

	{#if feedback.quote}
		{#snippet passage()}
			<span class="qhead">{lost ? 'On text that has changed' : 'On the text'}</span>
			<span class="qtext">“{feedback.quote}”</span>
		{/snippet}
		{#if onlocate && !lost}
			<button class="quote passage" title="Show it in the document" onclick={() => onlocate(feedback.id)}>{@render passage()}</button>
		{:else}
			<div class="quote passage gone">{@render passage()}</div>
		{/if}
	{/if}

	{#if editing}
		<PersonPicker bind:value={from} {project} />
		<MarkdownEditor bind:value={draft} label="Feedback" autofocus onsubmit={save} oncancel={() => (editing = false)} />
		<div class="row-end">
			<span class="hint" style="align-self:center">Ctrl+Enter saves</span>
			<button class="btn small ghost" onclick={() => (editing = false)}>Cancel</button>
			<button class="btn small primary" onclick={save} disabled={saving || !draft.trim()}>Save</button>
		</div>
	{:else}
		<Markdown html={feedback.html} />
		<div class="actions">
			<button class="link accent" onclick={toggle}>{resolved ? 'Reopen' : 'Mark resolved'}</button>
			{#if replies.length}
				<button class="link accent" onclick={toggleAll}>{allResolved ? 'Reopen all' : 'Resolve all'}</button>
			{/if}
			<button class="link accent" onclick={() => (prompting = true)}>Create prompt…</button>
			{#if !fromPortflow}
				<button class="link" onclick={startEdit}>Edit</button>
				<button class="link danger" onclick={remove}>Delete</button>
			{:else}
				{#if portflowItem}
					<button class="link accent" onclick={() => openInPortflow(portflowItem ?? '')}>Open in Portflow</button>
				{:else}
					<span class="hint">From Portflow: answer it there</span>
				{/if}
			{/if}
		</div>
		{#if replies.length}
			<div class="replies">
				{#each replies as r (r.id)}
					<div class="reply" class:done={r.status === 'resolved'} id="f-{r.id}">
						<div class="meta">
							<span><strong>{r.from ?? 'Someone'}</strong>{#if r.fromRole}<span class="badge">{r.fromRole}</span>{/if}</span>
							<span class="time">{time(r.createdAt)}</span>
							{#if r.status === 'resolved'}<span class="hint">resolved</span>{/if}
						</div>
						<Markdown html={r.html} />
						<div class="actions">
							<button class="link accent" onclick={() => toggleReply(r)}>{r.status === 'resolved' ? 'Reopen' : 'Mark resolved'}</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	{/if}
</div>

{#if prompting}
	<!-- The prompt lands in this feedback's own thread, or in the deliverable page's (`promptThread`). From a deliverable's
	     page that is another screen, so go and show it; in the thread itself the new prompt scrolls into view on its own. -->
	<PromptModal
		{project}
		thread={promptThread ?? thread}
		prompt={null}
		deliverable={liveDeliverable(feedback.deliverablePath, feedback.deliverableVersion)}
		feedback={[feedback.id]}
		onsaved={onDeliverable ? (saved) => (onprompted ? onprompted() : goto(`${threadUrl(project, promptThread ?? thread)}?item=pr-${saved.id}`)) : undefined}
		onclose={() => (prompting = false)}
	/>
{/if}

<style>
	.from {
		display: inline-flex;
		align-items: center;
		gap: 4px;
	}
	.reply.done {
		opacity: 0.6;
	}
	.replies {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: 8px;
		padding-left: 12px;
		border-left: 2px solid var(--line);
	}
</style>
