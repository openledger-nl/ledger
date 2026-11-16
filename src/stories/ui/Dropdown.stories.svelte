<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Dropdown from '$lib/components/Dropdown.svelte';
	import type { DropdownOption } from '$lib/components/Dropdown.svelte';

	const { Story } = defineMeta({ title: 'UI/Dropdown', component: Dropdown });
</script>

<script lang="ts">
	const options: DropdownOption[] = [
		{ value: 'sensor-research', label: 'Sensor research', group: 'Threads' },
		{ value: 'project-plan', label: 'Project plan', group: 'Threads' },
		{ value: 'dashboard-prototype', label: 'Dashboard prototype', group: 'Threads', hint: '7 posts' },
		{ value: 'new', label: '+ New thread…' }
	];
	let value = $state('sensor-research');
</script>

<Story name="Small" asChild>
	<Dropdown bind:value {options} label="Thread" />
</Story>

<Story name="Field, full width" asChild>
	<div style="max-width: 360px"><Dropdown bind:value {options} label="Thread" size="field" full /></div>
</Story>

<Story name="Placeholder" asChild>
	<Dropdown value="" {options} label="Thread" placeholder="Pick a thread" />
</Story>

<Story name="Disabled" asChild>
	<Dropdown bind:value {options} label="Thread" disabled />
</Story>
