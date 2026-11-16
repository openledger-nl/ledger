<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ScreenshotModal from '$lib/components/ScreenshotModal.svelte';
	import * as fx from '../fixtures';
	import Setup from '../Setup.svelte';
	import { screenshot } from '$lib/ui.svelte';

	const { Story } = defineMeta({ title: 'Screenshots/ScreenshotModal', component: ScreenshotModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="A screenshot to file" asChild>
	<Setup run={() => (screenshot.pending = { width: 1280, height: 800 })} reset={() => (screenshot.pending = null)}>
		<ScreenshotModal projects={fx.projects} />
	</Setup>
</Story>

<Story name="No projects yet" asChild>
	<Setup run={() => (screenshot.pending = { width: 1280, height: 800 })} reset={() => (screenshot.pending = null)}>
		<ScreenshotModal projects={[]} />
	</Setup>
</Story>
