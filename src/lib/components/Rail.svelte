<script lang="ts">
	// The sidebar folded to an icon rail, when Settings › Visual › Sidebar asks for one: at full width once the sidebar
	// is folded, and between 720 and 1199 px (Ledger snapped to half the screen, next to Canvas or Portflow). The main
	// pages as icons, each project as its initial, Portflow's dot and the vault switch; hovering or focusing an item shows its name.
	// Wireframes A6 and A7.
	import House from '@lucide/svelte/icons/house';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import Inbox from '@lucide/svelte/icons/inbox';
	import Plus from '@lucide/svelte/icons/plus';
	import Settings from '@lucide/svelte/icons/settings';
	import Vault from '@lucide/svelte/icons/vault';
	import { page } from '$app/state';
	import { newProjectFlow, projectUrl } from '$lib/actions';
	import { portflow, portflowFooter } from '$lib/portflow.svelte';
	import { openVaultPicker } from '$lib/vaults.svelte';
	import type { ProjectSummary } from '$lib/types';
	import EpsilonIcon from '$lib/components/EpsilonIcon.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';

	let { projects }: { projects: ProjectSummary[] } = $props();

	const path = $derived(page.url.pathname);
	const on = (prefix: string) => path === prefix || path.startsWith(prefix + '/');
	const pf = $derived(portflowFooter(portflow.status));

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
	<a class="rail-mark" href="/" data-tip="Ledger | Home" aria-label="Home"><Wordmark mark height={30} /></a>

	<nav class="rail-group" aria-label="Pages">
		<a class="rail-item" class:active={path === '/'} href="/" data-tip="Home" aria-label="Home"><House size={18} /></a>
		{#if portflow.enabled}
			<a class="rail-item" class:active={on('/epsilon')} href="/epsilon" data-tip="Epsilon+" aria-label="Epsilon+"><EpsilonIcon size={18} /></a>
		{/if}
		<a class="rail-item" class:active={on('/feedback')} href="/feedback" data-tip="Feedback" aria-label="Feedback"><Inbox size={18} /></a>
		<a class="rail-item" class:active={on('/meetings')} href="/meetings" data-tip="Meeting notes" aria-label="Meeting notes"><NotebookPen size={18} /></a>
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

	{#if portflow.enabled}
		<a class="rail-sync" href="/settings/sync#portflow" data-tip={[pf.title, pf.when, pf.canvas].filter(Boolean).join(' | ')} aria-label={pf.title}>
			<span class="dot {pf.dot}"></span>
		</a>
	{/if}
	<button class="rail-item rail-vault" data-tip="Switch vault" aria-label="Switch vault" onclick={openVaultPicker}><Vault size={16} /></button>
</aside>

{#if tip}
	<div class="rail-tip" role="tooltip" style:top="{tip.top}px">{tip.text}</div>
{/if}
