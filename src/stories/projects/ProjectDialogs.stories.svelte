<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ProjectDialogs from '$lib/components/ProjectDialogs.svelte';
	import Setup from '../Setup.svelte';
	import * as fx from '../fixtures';
	import { projectDialogs } from '$lib/dialogs.svelte';

	const { Story } = defineMeta({ title: 'Projects/ProjectDialogs', component: ProjectDialogs, parameters: { layout: 'fullscreen' } });
</script>

<!-- The app-wide host of the new-project and new-deliverable dialogs, opened through projectDialogs. -->
<Story name="New project" asChild>
	<Setup run={() => (projectDialogs.project = 'new')} reset={() => (projectDialogs.project = null)}>
		<ProjectDialogs />
	</Setup>
</Story>

<Story name="New deliverable" asChild>
	<Setup run={() => (projectDialogs.deliverable = { project: fx.PROJECT })} reset={() => (projectDialogs.deliverable = null)}>
		<ProjectDialogs />
	</Setup>
</Story>
