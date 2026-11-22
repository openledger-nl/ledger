<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'UI/Sidebar', component: Sidebar, parameters: { layout: 'fullscreen' } });
</script>

<Story name="Two projects" asChild>
	<div style="display:flex;height:640px"><Sidebar projects={fx.projects} /></div>
</Story>

<Story name="No projects yet" asChild>
	<div style="display:flex;height:640px"><Sidebar projects={[]} /></div>
</Story>
