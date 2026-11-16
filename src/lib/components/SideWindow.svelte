<script lang="ts" module>
	import type { Component } from 'svelte';

	/** One section of a side window: its button on the rail and its title; `count` shows on the button. */
	export interface SideSection {
		id: string;
		title: string;
		icon: Component<{ size?: number }>;
		count?: number;
		/** Beside the title in the header, e.g. "2 open". */
		headCount?: string;
	}
</script>

<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { tick, untrack, type Snippet } from 'svelte';
	import { HALF } from '$lib/layout.svelte';

	// The side window beside a page (a deliverable's document, a thread's timeline): a rail that is always there, with a
	// button for each section, and the section it opens beside the page. One section is open at a time; its button closes
	// it again, and so does the X in its header. Open by default where the window is wide enough to hold it beside the page
	// (below that it is a drawer over it, see app.css), and remembered per kind of page under `storageKey`.
	// Every section stays mounted while the window is shut, so a half-written feedback survives closing and switching.
	let {
		sections,
		storageKey,
		label = 'Side window',
		panel
	}: {
		sections: SideSection[];
		storageKey: string;
		label?: string;
		/** What a section holds, by its id. */
		panel: Snippet<[string]>;
	} = $props();

	// Which section is open is read once, when the page opens: from what was left open last time on this kind of page.
	function initial(): string | null {
		const first = sections[0]?.id ?? null;
		try {
			const saved = localStorage.getItem(storageKey);
			if (saved && sections.some((s) => s.id === saved)) return saved;
			if (saved === 'closed') return null;
			return window.innerWidth >= HALF ? first : null;
		} catch {
			return first; // storage unavailable: the first section is open
		}
	}
	let active = $state<string | null>(untrack(initial));

	function show(next: string | null) {
		active = next;
		try {
			localStorage.setItem(storageKey, next ?? 'closed');
		} catch {
			// Not persisted.
		}
	}

	const current = $derived(sections.find((s) => s.id === active));

	/** Opens a section; `cardId`, when given, is scrolled into view in it and flashes. */
	export async function reveal(section: string, cardId?: string) {
		show(section);
		if (!cardId) return;
		await tick();
		const card = document.getElementById(cardId);
		if (!card) return;
		card.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
		card.classList.add('flash');
		setTimeout(() => card.classList.remove('flash'), 1600);
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="side-scrim" class:open={active} onclick={() => show(null)}></div>
<aside class="side-window" hidden={!active} aria-label={current?.title}>
	<div class="side-head">
		<h2>
			{current?.title}
			{#if current?.headCount}<span class="side-count">{current.headCount}</span>{/if}
		</h2>
		<button class="icon-btn" aria-label="Close {current?.title}" title="Close" onclick={() => show(null)}><X size={18} /></button>
	</div>
	{#each sections as s (s.id)}
		<div class="side-body" hidden={active !== s.id}>{@render panel(s.id)}</div>
	{/each}
</aside>

<nav class="side-rail" aria-label={label}>
	{#each sections as s (s.id)}
		<button
			class="rail-btn"
			aria-pressed={active === s.id}
			aria-label={s.title}
			title="{s.title}{s.headCount ? ` · ${s.headCount}` : s.count ? ` · ${s.count}` : ''}"
			onclick={() => show(active === s.id ? null : s.id)}
		>
			<s.icon size={19} />
			{#if s.count}<span class="rail-count">{s.count}</span>{/if}
		</button>
	{/each}
</nav>
