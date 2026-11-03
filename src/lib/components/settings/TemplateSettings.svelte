<script lang="ts">
	import { page } from '$app/state';
	import TemplatesPanel from '$lib/components/TemplatesPanel.svelte';
	import type { Template } from '$lib/types';

	// The templates every project shares, loaded by the settings layout so they stay current when one is saved.
	const templates = $derived(page.data.templates as Template[]);
</script>

<TemplatesPanel scope="" {templates} />
