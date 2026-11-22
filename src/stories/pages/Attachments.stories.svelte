<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PageStory from '../PageStory.svelte';
	import RootLayout from '../../routes/+layout.svelte';
	import Page from '../../routes/projects/[project]/attachments/+page.svelte';
	import Layout0 from '../../routes/projects/[project]/+layout.svelte';
	import { load as load0 } from '../../routes/+layout';
	import { load as load1 } from '../../routes/projects/[project]/+layout';
	import { load as load2 } from '../../routes/projects/[project]/attachments/+page';

	const params = { project: 'smart-greenhouse' };
	const url = '/projects/smart-greenhouse/attachments';

	const { Story } = defineMeta({
		title: 'Pages/Project/Attachments',
		component: PageStory,
		parameters: {
			layout: 'fullscreen',
			sveltekit_experimental: {
				state: { page: { params, url: new URL('http://localhost/projects/smart-greenhouse/attachments'), route: { id: '/projects/[project]/attachments' } } },
				// The root layout notes each page it arrives at (afterNavigate): here it arrived at this one.
				navigation: { afterNavigate: { to: { params, url: new URL('http://localhost/projects/smart-greenhouse/attachments'), route: { id: '/projects/[project]/attachments' } } } }
			}
		}
	});
</script>

<!-- The page at /projects/smart-greenhouse/attachments, inside its layouts, with the data its load functions get from the sample backend. -->
<Story name="Default" asChild>
	<PageStory page={Page} layouts={[RootLayout, Layout0]} loads={[load0, load1, load2]} {url} {params} />
</Story>
