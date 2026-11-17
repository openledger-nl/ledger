<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import FeedbackModal from '$lib/components/FeedbackModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Feedback/FeedbackModal', component: FeedbackModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="On a deliverable" asChild>
	<FeedbackModal project={fx.PROJECT} thread={fx.THREAD} path="sensor-research.md" onclose={() => {}} />
</Story>

<Story name="In a thread" asChild>
	<FeedbackModal project={fx.PROJECT} thread={fx.THREAD} onclose={() => {}} />
</Story>
