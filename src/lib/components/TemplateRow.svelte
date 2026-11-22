<script lang="ts">
	// One template in a list of them: its name, a line about it, and what can be done with it: an Edit button for
	// `onedit`, or whatever `children` puts there.
	import type { Snippet } from 'svelte';

	let { name, about, onedit, children }: { name: string; about: string; onedit?: () => void; children?: Snippet } = $props();
</script>

<div class="tpl-row">
	<div class="name">
		<b>{name}</b>
		<span>{about}</span>
	</div>
	{#if children}{@render children()}{:else if onedit}<button class="btn small" onclick={onedit}>Edit</button>{/if}
</div>
