<script lang="ts">
	import SettingsGroup from '$lib/components/settings/SettingsGroup.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>Shortcuts | Settings | Ledger</title></svelte:head>

<SettingsGroup group="shortcuts" settings={data.settings} />
