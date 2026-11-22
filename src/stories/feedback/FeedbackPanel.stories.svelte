<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import FeedbackPanel from '$lib/components/FeedbackPanel.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Feedback/FeedbackPanel', component: FeedbackPanel });
</script>

<Story name="On a deliverable" asChild>
	<div style="max-width: 420px">
		<FeedbackPanel
			project={fx.PROJECT}
			doc={fx.deliverable}
			prompts={[fx.prompt]}
			threads={fx.threads}
			thread={fx.THREAD}
			threadTitle="Sensor research"
			lost={new Set()}
			here={() => true}
			onlocate={() => {}}
			onopenprompts={() => {}}
		/>
	</div>
</Story>

<Story name="A passage selected" asChild>
	<div style="max-width: 420px">
		<FeedbackPanel
			project={fx.PROJECT}
			doc={fx.deliverable}
			prompts={[]}
			threads={fx.threads}
			thread={fx.THREAD}
			threadTitle="Sensor research"
			pending={fx.anchor}
			lost={new Set()}
			here={() => true}
			onlocate={() => {}}
			onopenprompts={() => {}}
		/>
	</div>
</Story>
