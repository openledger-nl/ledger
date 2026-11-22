<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import FileDrop from '$lib/components/FileDrop.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Uploads/FileDrop', component: FileDrop, parameters: { layout: 'fullscreen' } });
</script>

<!-- Nothing shows until something is dragged in: drag a file from your computer onto this story to see the overlay
     and the dialog that asks where it goes (with a preview for pictures). Saving goes to the sample backend. -->
<Story name="Drop a file here" asChild>
	<div style="min-height: 70vh; display: grid; place-items: center; color: var(--muted)">Drag a file onto this page.</div>
	<FileDrop projects={fx.projects} />
</Story>
