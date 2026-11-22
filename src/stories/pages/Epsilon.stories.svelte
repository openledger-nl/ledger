<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PageStory from '../PageStory.svelte';
	import RootLayout from '../../routes/+layout.svelte';
	import Page from '../../routes/epsilon/+page.svelte';
	import { load as load0 } from '../../routes/+layout';

	const params = {  };
	const url = '/epsilon';

	const { Story } = defineMeta({
		title: 'Pages/Epsilon+',
		component: PageStory,
		parameters: {
			layout: 'fullscreen',
			sveltekit_experimental: {
				state: { page: { params, url: new URL('http://localhost/epsilon'), route: { id: '/epsilon' } } },
				// The root layout notes each page it arrives at (afterNavigate): here it arrived at this one.
				navigation: { afterNavigate: { to: { params, url: new URL('http://localhost/epsilon'), route: { id: '/epsilon' } } } }
			}
		}
	});
</script>

<!-- The page at /epsilon, inside its layouts, with the data its load functions get from the sample backend. -->
<Story name="Default" asChild>
	<PageStory page={Page} layouts={[RootLayout]} loads={[load0]} {url} {params} />
</Story>
