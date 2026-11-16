<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PageStory from '../PageStory.svelte';
	import RootLayout from '../../routes/+layout.svelte';
	import Page from '../../routes/settings/semesters/+page.svelte';
	import Layout0 from '../../routes/settings/+layout.svelte';
	import { load as load0 } from '../../routes/+layout';
	import { load as load1 } from '../../routes/settings/+layout';

	const params = {  };
	const url = '/settings/semesters';

	const { Story } = defineMeta({
		title: 'Pages/Settings/Semesters',
		component: PageStory,
		parameters: {
			layout: 'fullscreen',
			sveltekit_experimental: {
				state: { page: { params, url: new URL('http://localhost/settings/semesters'), route: { id: '/settings/semesters' } } },
				// The root layout notes each page it arrives at (afterNavigate): here it arrived at this one.
				navigation: { afterNavigate: { to: { params, url: new URL('http://localhost/settings/semesters'), route: { id: '/settings/semesters' } } } }
			}
		}
	});
</script>

<!-- The page at /settings/semesters, inside its layouts, with the data its load functions get from the sample backend. -->
<Story name="Default" asChild>
	<PageStory page={Page} layouts={[RootLayout, Layout0]} loads={[load0, load1]} {url} {params} />
</Story>
