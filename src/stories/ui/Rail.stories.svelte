<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Rail from '$lib/components/Rail.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'UI/Rail', component: Rail, parameters: { layout: 'fullscreen' } });
</script>

<Story name="Two projects" asChild>
	<div style="display:flex;height:560px"><Rail projects={fx.projects} /></div>
</Story>
