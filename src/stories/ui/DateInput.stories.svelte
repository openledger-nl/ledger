<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import DateInput from '$lib/components/DateInput.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'UI/DateInput', component: DateInput });
</script>

<script lang="ts">
	let day = $state('2026-11-16');
	let empty = $state('');
</script>

<Story name="With a date" asChild>
	<DateInput bind:value={day} label="Hand-in date" />
</Story>

<Story name="Empty" asChild>
	<DateInput bind:value={empty} label="Hand-in date" />
</Story>

<Story name="Within a semester" asChild>
	<DateInput bind:value={day} label="Sprint start" range={{ start: fx.semester.start, end: fx.semester.end }} min={fx.semester.start} max={fx.semester.end} />
</Story>
