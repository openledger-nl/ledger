<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PersonPicker from '$lib/components/PersonPicker.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'People/PersonPicker', component: PersonPicker });
</script>

<script lang="ts">
	import type { FeedbackFrom } from '$lib/types';
	let from = $state<FeedbackFrom>({ name: 'Frank de Vries', portflowUser: 101, role: 'coach', given: null });
	let nobody = $state<FeedbackFrom>({ name: null, portflowUser: null, role: null, given: null });
</script>

<Story name="Someone picked" asChild>
	<PersonPicker bind:value={from} project={fx.PROJECT} />
</Story>

<Story name="Nobody yet" asChild>
	<PersonPicker bind:value={nobody} project={fx.PROJECT} />
</Story>
