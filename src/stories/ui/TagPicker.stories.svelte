<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import TagPicker from '$lib/components/TagPicker.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'UI/TagPicker', component: TagPicker });
</script>

<script lang="ts">
	let people = $state(['coach/frank']);
	let projects = $state<string[]>([fx.PROJECT]);
</script>

<Story name="People, with a quick pick" asChild>
	<TagPicker
		bind:value={people}
		options={fx.peopleOptions}
		label="With"
		placeholder="Who was there"
		quick={[{ label: 'Whole team', tags: ['teammate/sam', 'client/ries'], title: 'Adds Sam Bakker, Ries van Dam' }]}
	/>
</Story>

<Story name="Projects" asChild>
	<TagPicker bind:value={projects} options={fx.projectOptions} label="Projects" placeholder="Couple to a project" prefix="project/" />
</Story>

<Story name="At most one, no new tags" asChild>
	<TagPicker value={[]} options={fx.peopleOptions} label="Coach" max={1} allowNew={false} />
</Story>
