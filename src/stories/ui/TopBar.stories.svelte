<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import TopBar from '$lib/components/TopBar.svelte';

	const { Story } = defineMeta({ title: 'UI/TopBar', component: TopBar, parameters: { layout: 'fullscreen', sveltekit_experimental: { state: { page: { params: { project: 'smart-greenhouse', thread: 'sensor-research' }, url: new URL('http://localhost/projects/smart-greenhouse/threads/sensor-research') } } } } });
</script>

<!-- The crumbs come from the page (here a thread page); search and the capture button work against the sample vault. -->
<Story name="On a thread page" />
