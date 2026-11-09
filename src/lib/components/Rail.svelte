<script lang="ts">
	// The sidebar folded to an icon rail: at full width once the sidebar is folded, and between 720 and 1199 px (Ledger snapped to half the screen, next to Canvas or Portflow). The main
	// pages as icons, each project as its initial, and the sync dot; hovering or focusing an item shows its name.
	// Wireframes A6 and A7.
	import House from '@lucide/svelte/icons/house';
	import Plus from '@lucide/svelte/icons/plus';
	import Settings from '@lucide/svelte/icons/settings';
	import { page } from '$app/state';
	import { newProjectFlow, projectUrl } from '$lib/actions';
	import { portflow } from '$lib/portflow.svelte';
	import { sync, syncFooter } from '$lib/ui.svelte';
	import type { ProjectSummary } from '$lib/types';
	import GammaIcon from '$lib/components/GammaIcon.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';

	let { projects }: { projects: ProjectSummary[] } = $props();

	const path = $derived(page.url.pathname);
	const on = (prefix: string) => path === prefix || path.startsWith(prefix + '/');
	const footer = $derived(syncFooter(sync.status));

	/** The letter a project goes by on the rail: the first letter or digit of its name. */
	const initial = (name: string) => (name.match(/[\p{L}\p{N}]/u)?.[0] ?? '?').toUpperCase();

	// The name of what the pointer (or the focus) is on, beside the rail. Drawn here, fixed, rather than as a CSS
	// tooltip: the project list scrolls, and would clip one.
	let tip = $state<{ text: string; top: number } | null>(null);
	function show(e: Event) {
		const item = (e.target as HTMLElement).closest<HTMLElement>('[data-tip]');
		if (!item) return (tip = null);
		const box = item.getBoundingClientRect();
		tip = { text: item.dataset.tip ?? '', top: box.top + box.height / 2 };
	}
	const hide = () => (tip = null);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<aside class="rail" aria-label="Sidebar" onpointerover={show} onpointerleave={hide} onfocusin={show} onfocusout={hide} onclick={hide}>
	<a class="rail-mark" href="/" data-tip="Ledger · Home" aria-label="Home"><Wordmark mark height={30} /></a>

	<nav class="rail-group" aria-label="Pages">
		<a class="rail-item" class:active={path === '/'} href="/" data-tip="Home" aria-label="Home"><House size={18} /></a>
		{#if portflow.enabled}
			<a class="rail-item" class:active={on('/gamma')} href="/gamma" data-tip="Gamma" aria-label="Gamma"><GammaIcon size={18} /></a>
		{/if}
		<a class="rail-item" class:active={on('/settings')} href="/settings" data-tip="Settings" aria-label="Settings"><Settings size={18} /></a>
	</nav>

	<div class="rail-rule"></div>

	<nav class="rail-group rail-projects" aria-label="Projects">
		{#each projects as p (p.slug)}
			<a class="rail-project" class:active={page.params.project === p.slug} href={projectUrl(p.slug)} data-tip={p.name} aria-label={p.name}>
				{initial(p.name)}
			</a>
		{/each}
		<button class="rail-item" data-tip="New project" aria-label="New project" onclick={newProjectFlow}><Plus size={16} /></button>
	</nav>

	<a class="rail-sync" href="/settings/sync" data-tip={[footer.title, footer.when].filter(Boolean).join(' · ')} aria-label={footer.title}>
		<span class="dot {footer.dot}"></span>
	</a>
</aside>

{#if tip}
	<div class="rail-tip" role="tooltip" style:top="{tip.top}px">{tip.text}</div>
{/if}
