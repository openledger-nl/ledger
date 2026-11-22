<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Wordmark from '$lib/components/Wordmark.svelte';

	const { Story } = defineMeta({
		title: 'Brand/Wordmark',
		component: Wordmark,
		args: { height: 24, on: 'page', mark: false },
		argTypes: { on: { control: 'inline-radio', options: ['sidebar', 'page'] } }
	});
</script>

<Story name="On a page" />
<Story name="In the sidebar" args={{ on: 'sidebar' }} />
<Story name="Mark only" args={{ mark: true, height: 40 }} />
<Story name="Dark theme" globals={{ theme: 'dark' }} />
