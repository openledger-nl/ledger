<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import SyncWidget from '$lib/components/SyncWidget.svelte';

	const { Story } = defineMeta({ title: 'Settings/SyncWidget', component: SyncWidget });
</script>

<Story name="In the sidebar" />
<Story name="Embedded in Settings" args={{ embedded: true }} />
