<script lang="ts">
	import ClipboardCheck from '@lucide/svelte/icons/clipboard-check';
	import FileClock from '@lucide/svelte/icons/file-clock';
	import GitCompareArrows from '@lucide/svelte/icons/git-compare-arrows';
	import MessageSquareText from '@lucide/svelte/icons/message-square-text';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import type { Snippet } from 'svelte';
	import type { Anchor } from '$lib/anchors';
	import type { ChangeTracker } from '$lib/liveChanges.svelte';
	import type { Deliverable, Feedback, Prompt, Provenance, ProvenanceDraft, ThreadSummary } from '$lib/types';
	import ChangesPanel from './ChangesPanel.svelte';
	import FeedbackPanel from './FeedbackPanel.svelte';
	import PromptsPanel from './PromptsPanel.svelte';
	import ProvenancePanel from './ProvenancePanel.svelte';
	import SideWindow from './SideWindow.svelte';

	// The side of a deliverable's page: the side window (SideWindow.svelte, also beside a thread's timeline) with its
	// sections: Produced by, Changes, its evidence description (when it has or can have one), Feedback and Prompts.

	// `thread` is the page's thread, where new feedback goes; `pending` the passage selected in the document that feedback is
	// about to be written on; `lost` the feedback whose passage is no longer in the text; `onlocate` shows one in the document.
	// `here` says whether a piece of feedback belongs to what the page shows; `draft` is what the live file holds beyond the
	// last version, as it stands while it is being edited; `prompts` are the ones about this deliverable.
	let {
		project,
		doc,
		provenance,
		draft,
		prompts,
		threads,
		thread = $bindable(),
		threadTitle,
		pending = $bindable(null),
		lost,
		here,
		onlocate,
		changes,
		live = false,
		onhistory,
		description,
		descriptionEmpty = false
	}: {
		project: string;
		doc: Deliverable;
		provenance: Provenance;
		draft: ProvenanceDraft | null;
		prompts: Prompt[];
		threads: ThreadSummary[];
		thread: string;
		threadTitle: string;
		pending?: Anchor | null;
		lost: Set<string>;
		here: (f: Feedback) => boolean;
		onlocate: (id: string) => void;
		/** A markdown deliverable's changes since a version; without it, no Changes section. */
		changes?: ChangeTracker;
		/** The live file is open, so its changes are followed. */
		live?: boolean;
		onhistory?: (view: 'changes' | 'authors') => void;
		/** The evidence description (What? / Goal? / Why it contributes?); without it, no Evidence section. */
		description?: Snippet;
		/** It isn't written yet. */
		descriptionEmpty?: boolean;
	} = $props();

	type Section = 'provenance' | 'changes' | 'evidence' | 'feedback' | 'prompts';

	const openCount = $derived(doc.feedback.filter((f) => f.status === 'open').length);
	const sections = $derived([
		{ id: 'provenance', title: 'Produced by', icon: FileClock },
		...(changes
			? [
					{
						id: 'changes',
						title: 'Changes',
						icon: GitCompareArrows,
						count: live ? changes.changes.length : undefined,
						headCount: live && changes.data?.against ? `since v${changes.data.against}` : undefined
					}
				]
			: []),
		...(description ? [{ id: 'evidence', title: 'Evidence description', icon: ClipboardCheck, headCount: descriptionEmpty ? 'not written yet' : undefined }] : []),
		{
			id: 'feedback',
			title: 'Feedback',
			icon: MessageSquareText,
			count: openCount,
			headCount: doc.feedback.length ? `${openCount} open` : undefined
		},
		{ id: 'prompts', title: 'Prompts', icon: Sparkles, headCount: prompts.length ? String(prompts.length) : undefined }
	]);

	let side: SideWindow | undefined = $state();

	/** Opens a section; for feedback, `feedbackId` is scrolled into view and flashes. */
	export async function reveal(section: Section, feedbackId?: string) {
		await side?.reveal(section, feedbackId ? `f-${feedbackId}` : undefined);
	}
</script>

<SideWindow bind:this={side} {sections} storageKey="ledger-doc-side">
	{#snippet panel(id)}
		{#if id === 'provenance'}
			<ProvenancePanel {project} {doc} {provenance} {draft} />
		{:else if id === 'changes' && changes}
			<ChangesPanel tracker={changes} {live} onhistory={(view) => onhistory?.(view)} />
		{:else if id === 'evidence' && description}
			{@render description()}
		{:else if id === 'feedback'}
			<FeedbackPanel {project} {doc} {prompts} {threads} bind:thread {threadTitle} bind:pending {lost} {here} {onlocate} onopenprompts={() => void reveal('prompts')} />
		{:else}
			<PromptsPanel {project} {doc} {prompts} {threads} {thread} {threadTitle} onfeedback={(fid) => void reveal('feedback', fid)} />
		{/if}
	{/snippet}
</SideWindow>
