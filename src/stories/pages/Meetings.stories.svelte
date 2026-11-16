<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PageStory from '../PageStory.svelte';
	import RootLayout from '../../routes/+layout.svelte';
	import Page from '../../routes/meetings/+page.svelte';
	import { load as load0 } from '../../routes/+layout';
	import { load as load1 } from '../../routes/meetings/+page';

	const params = {  };
	const url = '/meetings?day=2026-11-16';

	const { Story } = defineMeta({
		title: 'Pages/Meetings',
		component: PageStory,
		parameters: {
			layout: 'fullscreen',
			sveltekit_experimental: {
				state: { page: { params, url: new URL('http://localhost/meetings?day=2026-11-16'), route: { id: '/meetings' } } },
				// The root layout notes each page it arrives at (afterNavigate): here it arrived at this one.
				navigation: { afterNavigate: { to: { params, url: new URL('http://localhost/meetings?day=2026-11-16'), route: { id: '/meetings' } } } }
			}
		}
	});
</script>

<!-- The page at /meetings, inside its layouts, with the data its load functions get from the sample backend. -->
<Story name="Default" asChild>
	<PageStory page={Page} layouts={[RootLayout]} loads={[load0, load1]} {url} {params} />
</Story>
