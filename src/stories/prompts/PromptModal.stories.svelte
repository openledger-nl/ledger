<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PromptModal from '$lib/components/PromptModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Prompts/PromptModal', component: PromptModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="New" asChild>
	<PromptModal project={fx.PROJECT} thread={fx.THREAD} prompt={null} onclose={() => {}} />
</Story>

<Story name="Editing" asChild>
	<PromptModal project={fx.PROJECT} thread={fx.THREAD} prompt={fx.prompt} onclose={() => {}} />
</Story>

<Story name="From feedback" asChild>
	<PromptModal project={fx.PROJECT} thread={fx.THREAD} prompt={null} deliverable="sensor-research.md" feedback={['f-1']} onclose={() => {}} />
</Story>
