<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import FeedbackPicker from '$lib/components/FeedbackPicker.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Feedback/FeedbackPicker', component: FeedbackPicker, parameters: { layout: 'fullscreen' } });
</script>

<Story name="Pick feedback" asChild>
	<FeedbackPicker title="Feedback for this prompt" items={fx.feedbackList} threadTitle={() => 'Sensor research'} onpick={() => {}} onclose={() => {}} />
</Story>
