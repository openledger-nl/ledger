<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PromptCard from '$lib/components/PromptCard.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Prompts/PromptCard', component: PromptCard });
</script>

<Story name="In a thread" asChild>
	<PromptCard project={fx.PROJECT} thread={fx.THREAD} prompt={fx.prompt} onedit={() => {}} feedback={[fx.feedbackOpen]} />
</Story>

<Story name="On a deliverable" asChild>
	<PromptCard project={fx.PROJECT} thread={fx.THREAD} prompt={fx.prompt} onedit={() => {}} onDeliverable />
</Story>
