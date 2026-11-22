<script lang="ts">
	// The templates every project shares, kept in shared/templates/prompts/, one file each; a project can replace any of
	// them with its own on its Templates tab. A new vault starts with one, a project plan, which is the vault's to
	// change or delete.
	import TemplatesPanel from '$lib/components/TemplatesPanel.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>Templates | Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner">
		<TemplatesPanel scope="" templates={data.templates} />
	</div>
</div>
