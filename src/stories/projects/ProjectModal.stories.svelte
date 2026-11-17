<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ProjectModal from '$lib/components/ProjectModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Projects/ProjectModal', component: ProjectModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="New project" asChild>
	<ProjectModal onclose={() => {}} />
</Story>

<Story name="Editing" asChild>
	<ProjectModal project={{ slug: fx.PROJECT, name: 'Smart Greenhouse', meta: fx.projectMeta }} onclose={() => {}} />
</Story>
