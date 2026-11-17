<script lang="ts">
	// For stories of components that show what is in Ledger's shared state (a dialog, a toast, a pending screenshot):
	// `run` puts it there as the story opens, `reset` takes it out again when the story closes.
	import { onMount, type Snippet } from 'svelte';

	let { run, reset, children }: { run: () => unknown; reset?: () => void; children: Snippet } = $props();

	onMount(() => {
		void run();
		return () => reset?.();
	});
</script>

{@render children()}
