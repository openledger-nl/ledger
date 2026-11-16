<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PageStory from '../PageStory.svelte';
	import RootLayout from '../../routes/+layout.svelte';
	import Page from '../../routes/projects/[project]/threads/+page.svelte';
	import Layout0 from '../../routes/projects/[project]/+layout.svelte';
	import { load as load0 } from '../../routes/+layout';
	import { load as load1 } from '../../routes/projects/[project]/+layout';

	const params = { project: 'smart-greenhouse' };
	const url = '/projects/smart-greenhouse/threads';

	const { Story } = defineMeta({
		title: 'Pages/Project/Threads',
		component: PageStory,
		parameters: {
			layout: 'fullscreen',
			sveltekit_experimental: {
				state: { page: { params, url: new URL('http://localhost/projects/smart-greenhouse/threads'), route: { id: '/projects/[project]/threads' } } },
				// The root layout notes each page it arrives at (afterNavigate): here it arrived at this one.
				navigation: { afterNavigate: { to: { params, url: new URL('http://localhost/projects/smart-greenhouse/threads'), route: { id: '/projects/[project]/threads' } } } }
			}
		}
	});
</script>

<!-- The page at /projects/smart-greenhouse/threads, inside its layouts, with the data its load functions get from the sample backend. -->
<Story name="Default" asChild>
	<PageStory page={Page} layouts={[RootLayout, Layout0]} loads={[load0, load1]} {url} {params} />
</Story>
