<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ProjectPortflow from '$lib/components/ProjectPortflow.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Portflow/ProjectPortflow', component: ProjectPortflow });
</script>

<Story name="A linked study project" asChild>
	<ProjectPortflow project={fx.PROJECT} threads={fx.threads} version={0} />
</Story>
