<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ContributionGraph from '$lib/components/ContributionGraph.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Dashboard/ContributionGraph', component: ContributionGraph });
</script>

<Story name="A semester's activity" asChild>
	<ContributionGraph events={fx.activity} projects={fx.projects} semesters={[fx.semester]} />
</Story>

<Story name="Nothing yet" asChild>
	<ContributionGraph events={[]} projects={[]} semesters={[]} />
</Story>
