<script lang="ts">
	// The panels of one settings group, as its tab shows them. Each panel can be linked to by its id:
	// /settings/sync#portflow opens the tab scrolled to the Portflow panel.
	import { tick } from 'svelte';
	import { page } from '$app/state';
	import { panels, type GroupId } from './panels';
	import type { Settings } from '$lib/types';

	let { group, settings }: { group: GroupId; settings: Settings } = $props();

	// The panels above load their contents after the page opens and grow, so the panel is scrolled to again once they
	// have settled; scrolling by hand in between stops that.
	$effect(() => {
		const id = page.url.hash.slice(1);
		if (!id) return;
		let stopped = false;
		const stop = () => (stopped = true);
		const go = () => !stopped && document.getElementById(id)?.scrollIntoView({ block: 'start', behavior: 'smooth' });
		void tick().then(go);
		const later = setTimeout(go, 400);
		window.addEventListener('wheel', stop, { once: true, passive: true });
		window.addEventListener('touchmove', stop, { once: true, passive: true });
		return () => {
			clearTimeout(later);
			window.removeEventListener('wheel', stop);
			window.removeEventListener('touchmove', stop);
		};
	});
</script>

{#each panels.filter((p) => p.group === group) as p (p.id)}
	<div class="settings-anchor" id={p.id}>
		<p.component {settings} />
	</div>
{/each}

<style>
	.settings-anchor {
		display: flex;
		flex-direction: column;
		scroll-margin-top: 12px;
	}
</style>
