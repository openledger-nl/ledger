<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import TimeInput from '$lib/components/TimeInput.svelte';

	const { Story } = defineMeta({ title: 'UI/TimeInput', component: TimeInput });
</script>

<script lang="ts">
	let time = $state('10:00');
</script>

<Story name="Default" asChild>
	<TimeInput bind:value={time} label="Time" />
</Story>

<Story name="Empty" asChild>
	<TimeInput value="" label="Time" />
</Story>
