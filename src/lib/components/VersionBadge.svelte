<script lang="ts">
	// A deliverable's newest version (of its whole timeline, the file it continues included), or Draft while it has none.
	import type { DeliverableSummary } from '$lib/types';

	let { d }: { d: Pick<DeliverableSummary, 'versions'> } = $props();
	const latest = $derived(d.versions.at(-1) ?? null);
</script>

{#if latest}
	<span class="badge published">v{latest}</span>
{:else}
	<span class="badge draft">Draft</span>
{/if}
