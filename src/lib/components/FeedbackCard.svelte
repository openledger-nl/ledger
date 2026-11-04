<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$lib/api';
	import { baseName, deliverableUrl, liveDeliverable, threadUrl } from '$lib/actions';
	import { time } from '$lib/format';
	import { attempt, confirmDialog, refresh } from '$lib/ui.svelte';
	import type { Feedback } from '$lib/types';
	import Markdown from './Markdown.svelte';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import PromptModal from './PromptModal.svelte';

	/**
	 * `onDeliverable`: shown on the deliverable's own page, where naming the deliverable would only be noise. There, the
	 * header says which version it was given on (`version`: null for the live file, as it stands) and in which thread
	 * (`threadTitle`); `promptThread` is the thread a prompt made from it goes to (the page's own, rather than the
	 * feedback's); `onlocate` shows the passage it is about in the document, and `lost` says that passage is no longer in it.
	 * `onselect` (open feedback only) puts a box on it to pick it for a prompt; `inPrompts` names the prompts it is in already.
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
		onprompted
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
	} = $props();

	let editing = $state(false);
	let draft = $state('');
	let saving = $state(false);
	let prompting = $state(false);

	const resolved = $derived(feedback.status === 'resolved');

	async function toggle() {
		const next = resolved ? 'open' : 'resolved';
		if (await attempt(() => api.setFeedbackStatus(project, thread, feedback.id, next))) await refresh();
	}

	function startEdit() {
		draft = feedback.body;
		editing = true;
	}

	async function save() {
		if (!draft.trim()) return;
		saving = true;
		const done = await attempt(() => api.updateFeedback(project, thread, feedback.id, draft));
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

<div class="card feedback-card" class:resolved id="f-{feedback.id}">
	<div class="fb-head">
		<div class="meta">
			{#if onselect}
				<input class="fb-pick" type="checkbox" checked={selected} aria-label="Select this feedback for a prompt" title="Select it for a prompt" onchange={() => onselect(feedback.id)} />
			{/if}
			<span class="card-kind">Feedback · {resolved ? 'resolved' : 'unresolved'}</span>
			<span class="time">{time(feedback.createdAt)}</span>
			{#if inPrompts.length}<span class="badge accent" title="In: {inPrompts.join(', ')}">In a prompt</span>{/if}
		</div>
		<div class="meta file">
			{#if onDeliverable}
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
			<button class="link accent" onclick={() => (prompting = true)}>Create prompt…</button>
			<button class="link" onclick={startEdit}>Edit</button>
			<button class="link danger" onclick={remove}>Delete</button>
		</div>
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
