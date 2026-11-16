<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import SprintTimeline from '$lib/components/SprintTimeline.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Planning/SprintTimeline', component: SprintTimeline });
</script>

<Story name="Four sprints and a holiday" asChild>
	<SprintTimeline start={fx.semester.start} end={fx.semester.end} sprints={fx.semester.sprints} vacations={fx.semester.vacations} />
</Story>

<Story name="No sprints" asChild>
	<SprintTimeline start={fx.semester.start} end={fx.semester.end} sprints={[]} />
</Story>
