<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PageStory from '../PageStory.svelte';
	import RootLayout from '../../routes/+layout.svelte';
	import Page from '../../routes/projects/[project]/threads/[thread]/+page.svelte';
	import Layout0 from '../../routes/projects/[project]/+layout.svelte';
	import { load as load0 } from '../../routes/+layout';
	import { load as load1 } from '../../routes/projects/[project]/+layout';
	import { load as load2 } from '../../routes/projects/[project]/threads/[thread]/+page';

	const params = { project: 'smart-greenhouse', thread: 'sensor-research' };
	const url = '/projects/smart-greenhouse/threads/sensor-research';

	const { Story } = defineMeta({
		title: 'Pages/Project/Thread',
		component: PageStory,
		parameters: {
			layout: 'fullscreen',
			sveltekit_experimental: {
				state: { page: { params, url: new URL('http://localhost/projects/smart-greenhouse/threads/sensor-research'), route: { id: '/projects/[project]/threads/[thread]' } } },
				// The root layout notes each page it arrives at (afterNavigate): here it arrived at this one.
				navigation: { afterNavigate: { to: { params, url: new URL('http://localhost/projects/smart-greenhouse/threads/sensor-research'), route: { id: '/projects/[project]/threads/[thread]' } } } }
			}
		}
	});
</script>

<!-- The page at /projects/smart-greenhouse/threads/sensor-research, inside its layouts, with the data its load functions get from the sample backend. -->
<Story name="Default" asChild>
	<PageStory page={Page} layouts={[RootLayout, Layout0]} loads={[load0, load1, load2]} {url} {params} />
</Story>
