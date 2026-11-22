<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PageStory from '../PageStory.svelte';
	import RootLayout from '../../routes/+layout.svelte';
	import Page from '../../routes/semesters/+page.svelte';
	import { load as load0 } from '../../routes/+layout';
	import { load as load1 } from '../../routes/semesters/+page';

	const params = {};
	const url = '/semesters';

	const { Story } = defineMeta({
		title: 'Pages/Semesters',
		component: PageStory,
		parameters: {
			layout: 'fullscreen',
			sveltekit_experimental: {
				state: { page: { params, url: new URL('http://localhost/semesters'), route: { id: '/semesters' } } },
				// The root layout notes each page it arrives at (afterNavigate): here it arrived at this one.
				navigation: { afterNavigate: { to: { params, url: new URL('http://localhost/semesters'), route: { id: '/semesters' } } } }
			}
		}
	});
</script>

<!-- The page at /semesters, inside its layouts, with the data its load functions get from the sample backend. -->
<Story name="Default" asChild>
	<PageStory page={Page} layouts={[RootLayout]} loads={[load0, load1]} {url} {params} />
</Story>
