<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PromptsPanel from '$lib/components/PromptsPanel.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Prompts/PromptsPanel', component: PromptsPanel });
</script>

<Story name="On a deliverable" asChild>
	<div style="max-width: 420px">
		<PromptsPanel project={fx.PROJECT} doc={fx.deliverable} prompts={[fx.prompt]} threads={fx.threads} thread={fx.THREAD} threadTitle="Sensor research" onfeedback={() => {}} />
	</div>
</Story>

<Story name="No prompts" asChild>
	<div style="max-width: 420px">
		<PromptsPanel project={fx.PROJECT} doc={fx.deliverable} prompts={[]} threads={fx.threads} thread={fx.THREAD} threadTitle="Sensor research" onfeedback={() => {}} />
	</div>
</Story>
