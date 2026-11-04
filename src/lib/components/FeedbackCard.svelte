<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$lib/api';
	import { openInPortflow } from '$lib/portflow.svelte';
	import { baseName, deliverableUrl, feedbackPromptDraft, threadUrl } from '$lib/actions';
	import { time } from '$lib/format';
	import { attempt, confirmDialog, refresh } from '$lib/ui.svelte';
	import type { Feedback, FeedbackFrom } from '$lib/types';
	import Markdown from './Markdown.svelte';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import PersonPicker from './PersonPicker.svelte';
	import PromptModal from './PromptModal.svelte';

	/** `onDeliverable`: shown on the deliverable's own page, where naming the deliverable would only be noise. */
	/** `replies`: answers to it in Portflow, shown inside it. `evidenceName`: the evidence item it is on. */
	let {
		project,
		thread,
		feedback,
		onDeliverable = false,
		replies = [],
		evidenceName = null,
		portflowItem = null
	}: {
		project: string;
		thread: string;
		feedback: Feedback;
		onDeliverable?: boolean;
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
	const GIVEN: Record<string, string> = { 'in person': 'in person', portflow: 'in Portflow', email: 'by email', other: '' };
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
	<div class="meta">
		<span class="card-kind">{kind}</span>
		{#if feedback.from}
			<span class="from"
				>from <strong>{feedback.from}</strong>{#if feedback.fromRole}<span class="badge">{feedback.fromRole}</span>{/if}{#if feedback.given && GIVEN[feedback.given]}
					{GIVEN[feedback.given]}{/if}</span
			>
		{/if}
		<span class="time">{time(feedback.createdAt)}</span>
		{#if feedback.evidence}
			<span class="file">on {evidenceName ?? feedback.evidence}{feedback.evidenceVersion ? ` v${feedback.evidenceVersion}` : ''}</span>
		{/if}
		{#if !feedback.deliverablePath}
			<!-- Feedback on an evidence item whose files Ledger doesn't have names no deliverable. -->
		{:else if onDeliverable}
			<span class="file">
				{feedback.deliverableVersion ? `on version ${feedback.deliverableVersion}` : 'on the current file'} ·
				<a href="/projects/{project}/threads/{thread}?item=f-{feedback.id}">in its thread</a>
			</span>
		{:else}
			<span class="file">
				on <a href={deliverableUrl(project, feedback.deliverablePath)}>{baseName(feedback.deliverablePath)}</a>
			</span>
		{/if}
	</div>

	{#if editing}
		<PersonPicker bind:value={from} />
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
	<!-- The prompt lands in this feedback's own thread. From a deliverable's page that is another screen, so go and show it;
	     in the thread itself the new prompt scrolls into view on its own. -->
	<PromptModal
		{project}
		{thread}
		prompt={null}
		draft={feedbackPromptDraft(feedback)}
		onsaved={onDeliverable ? (saved) => goto(`${threadUrl(project, thread)}?item=pr-${saved.id}`) : undefined}
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
