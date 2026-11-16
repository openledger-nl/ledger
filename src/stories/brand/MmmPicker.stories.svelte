<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import MmmPicker from '$lib/components/MmmPicker.svelte';

	const { Story } = defineMeta({ title: 'Brand/MmmPicker', component: MmmPicker });
</script>

<script lang="ts">
	import type { MmmLevel } from '$lib/types';
	let level = $state<MmmLevel | ''>('cyborg');
</script>

<Story name="Picked" asChild>
	<MmmPicker bind:value={level} />
</Story>

<Story name="Not set" asChild>
	<MmmPicker value="" />
</Story>

<Story name="Disabled" asChild>
	<MmmPicker value="handyman" disabled />
</Story>
