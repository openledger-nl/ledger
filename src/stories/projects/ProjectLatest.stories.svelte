<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ProjectLatest from '$lib/components/ProjectLatest.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Projects/ProjectLatest', component: ProjectLatest });
</script>

<Story name="Default" asChild>
	<ProjectLatest project={fx.projects[0]} deliverables={fx.deliverables} />
</Story>

<Story name="Nothing yet" asChild>
	<ProjectLatest project={{ ...fx.projects[1], threads: [] }} deliverables={[]} />
</Story>
