<script lang="ts">
	import FileClock from '@lucide/svelte/icons/file-clock';
	import MessageSquareText from '@lucide/svelte/icons/message-square-text';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import X from '@lucide/svelte/icons/x';
	import { tick } from 'svelte';
	import type { Anchor } from '$lib/anchors';
	import type { Deliverable, Feedback, Prompt, Provenance, ProvenanceDraft, ThreadSummary } from '$lib/types';
	import FeedbackPanel from './FeedbackPanel.svelte';
	import PromptsPanel from './PromptsPanel.svelte';
	import ProvenancePanel from './ProvenancePanel.svelte';

	// The side of a deliverable's page: a rail that is always there, with a button for each section (Produced by, Feedback,
	// Prompts), and the section it opens beside the page. One section is open at a time; its button closes it again, and so does the X in its header.
	// Open by default where the window is wide enough to hold it next to the page (below that it is a drawer over it).

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
		thread,
		threadTitle,
		pending = $bindable(null),
		lost,
		here,
		onlocate
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
	} = $props();

	type Section = 'provenance' | 'feedback' | 'prompts';

	const openCount = $derived(doc.feedback.filter((f) => f.status === 'open').length);
	const sections = $derived([
		{ id: 'provenance' as const, title: 'Produced by', icon: FileClock, count: 0 },
		{ id: 'feedback' as const, title: 'Feedback', icon: MessageSquareText, count: openCount },
		{ id: 'prompts' as const, title: 'Prompts', icon: Sparkles, count: 0 }
	]);

	const KEY = 'ledger-doc-side';

	let active = $state<Section | null>('provenance');
	try {
		const saved = localStorage.getItem(KEY);
		if (saved === 'provenance' || saved === 'feedback' || saved === 'prompts') active = saved;
		else if (saved === 'closed') active = null;
		else active = window.innerWidth >= 1100 ? 'provenance' : null;
	} catch {
		// Storage unavailable: Produced by is open.
	}

	function show(next: Section | null) {
		active = next;
		try {
			localStorage.setItem(KEY, next ?? 'closed');
		} catch {
			// Not persisted.
		}
	}

	const current = $derived(sections.find((s) => s.id === active));

	/** Opens a section; for feedback, `feedbackId` is scrolled into view and flashes. */
	export async function reveal(section: Section, feedbackId?: string) {
		show(section);
		if (!feedbackId) return;
		await tick();
		const card = document.getElementById(`f-${feedbackId}`);
		if (!card) return;
		card.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
		card.classList.add('flash');
		setTimeout(() => card.classList.remove('flash'), 1600);
	}
</script>

<!-- Both sections stay mounted while the window is shut, so a half-written feedback survives closing and switching. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="side-scrim" class:open={active} onclick={() => show(null)}></div>
<aside class="side-window" hidden={!active} aria-label={current?.title}>
	<div class="side-head">
		<h2>
			{current?.title}
			{#if active === 'feedback' && doc.feedback.length}<span class="side-count">{openCount} open</span>{/if}
			{#if active === 'prompts' && prompts.length}<span class="side-count">{prompts.length}</span>{/if}
		</h2>
		<button class="icon-btn" aria-label="Close {current?.title}" title="Close" onclick={() => show(null)}><X size={18} /></button>
	</div>
	<div class="side-body" hidden={active !== 'provenance'}>
		<ProvenancePanel {project} {doc} {provenance} {draft} />
	</div>
	<div class="side-body" hidden={active !== 'feedback'}>
		<FeedbackPanel {project} {doc} {prompts} {threads} {thread} {threadTitle} bind:pending {lost} {here} {onlocate} onopenprompts={() => void reveal('prompts')} />
	</div>
	<div class="side-body" hidden={active !== 'prompts'}>
		<PromptsPanel {project} {doc} {prompts} {threads} {thread} {threadTitle} onfeedback={(id) => void reveal('feedback', id)} />
	</div>
</aside>

<nav class="side-rail" aria-label="Side window">
	{#each sections as s (s.id)}
		<button
			class="rail-btn"
			aria-pressed={active === s.id}
			aria-label={s.title}
			title="{s.title}{s.count ? ` · ${s.count} open` : ''}"
			onclick={() => show(active === s.id ? null : s.id)}
		>
			<s.icon size={19} />
			{#if s.count}<span class="rail-count">{s.count}</span>{/if}
		</button>
	{/each}
</nav>
