<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ProjectBranding from '$lib/components/ProjectBranding.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Projects/ProjectBranding', component: ProjectBranding });
</script>

<Story name="Default" asChild>
	<ProjectBranding project={fx.projects[0].slug} others={fx.projects.slice(1).map((p) => ({ slug: p.slug, name: p.name }))} client="Police" />
</Story>
