<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { open } from '@tauri-apps/plugin-dialog';
	import { api } from '$lib/api';
	import { plainText, time } from '$lib/format';
	import { attempt, refresh } from '$lib/ui.svelte';
	import type { Post } from '$lib/types';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import { LABELS, POST_KINDS } from '$lib/v1';
	import type { EpistemicLabel, PostKind } from '$lib/types';

	let {
		project,
		thread,
		replyTo,
		oncancelreply,
		onsent,
		onprompt,
		onfeedback,
		onupload
	}: {
		project: string;
		thread: string;
		replyTo: Post | null;
		oncancelreply: () => void;
		onsent: () => void;
		onprompt: () => void;
		onfeedback: () => void;
		/** Called with the paths of the files the person picked. */
		onupload: (paths: string[]) => void;
	} = $props();

	let text = $state('');
	let sending = $state(false);
	// The kind and the label the next post gets (one label at most: Fact, To check or Assumption); the kind stays for the
	// next one, the label doesn't.
	let kind = $state<PostKind>('thought');
	let labels = $state<EpistemicLabel[]>([]);
	let input: MarkdownEditor;

	// Replying focuses the box; so does arriving in a thread.
	$effect(() => {
		project;
		thread;
		replyTo;
		input?.focus();
	});

	async function send() {
		const body = text.trim();
		if (!body || sending) return;
		sending = true;
		const post = await attempt(() => api.addPost(project, thread, body, replyTo?.id ?? null));
		sending = false;
		if (!post) return;
		await attempt(() => api.setPostMeta(project, thread, post.id, kind, labels));
		labels = [];
		text = '';
		await refresh();
		onsent();
		input.focus();
	}

	async function pickFiles() {
		const picked = await attempt(() => open({ multiple: true, title: 'Upload files to this thread' }));
		if (!picked) return;
		const paths = Array.isArray(picked) ? picked : [picked];
		if (paths.length) onupload(paths);
	}

</script>

<div class="composer">
	{#if replyTo}
		<div class="reply-bar">
			<span><b>Replying to {time(replyTo.createdAt)}:</b> {plainText(replyTo.body, 160)}</span>
			<button class="link" aria-label="Cancel reply" onclick={oncancelreply}><X size={14} /></button>
		</div>
	{/if}
	<div class="toolbar">
		<button class="btn small chip-ai" onclick={onprompt} title="Ask for a deliverable, or for changes to one">Make deliverable</button>
		<button class="btn small chip-own" onclick={onfeedback} title="Give feedback on a deliverable">Feedback</button>
		<button class="btn small" onclick={pickFiles} title="Attach files to this thread">Upload files</button>
	</div>
	<div class="input-row">
		<MarkdownEditor
			bind:this={input}
			bind:value={text}
			variant="composer"
			rows={1}
			label="Note"
			placeholder="Write a note... markdown works (Enter saves, Shift+Enter adds a line)"
			submitOnEnter
			onsubmit={send}
			oncancel={replyTo ? oncancelreply : undefined}
		/>
		<button class="btn primary" onclick={send} disabled={sending || !text.trim()}>Save</button>
	</div>
	<!-- What the next note is: its kind, and at most one label. Their own row under the box, so neither is squeezed. -->
	<div class="post-options">
		<div class="seg small" role="group" aria-label="Kind of note">
			{#each POST_KINDS as k (k.kind)}
				<button class:on={kind === k.kind} aria-pressed={kind === k.kind} onclick={() => (kind = k.kind)}>{k.name}</button>
			{/each}
		</div>
		<div class="seg small" role="group" aria-label="Label">
			<button class:on={!labels.length} aria-pressed={!labels.length} title="No label" onclick={() => (labels = [])}>No label</button>
			{#each LABELS as l (l.label)}
				<button class:on={labels[0] === l.label} aria-pressed={labels[0] === l.label} title={l.hint} onclick={() => (labels = labels[0] === l.label ? [] : [l.label])}
					>{l.name}</button
				>
			{/each}
		</div>
	</div>
</div>

<style>
	.post-options {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px 16px;
	}
</style>
