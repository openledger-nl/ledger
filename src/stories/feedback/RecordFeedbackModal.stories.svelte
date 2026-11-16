<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import RecordFeedbackModal from '$lib/components/RecordFeedbackModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Feedback/RecordFeedbackModal', component: RecordFeedbackModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="In a project" asChild>
	<RecordFeedbackModal
		projects={fx.projects}
		peopleOptions={fx.peopleOptions}
		people={fx.people}
		notes={[fx.dailyNote]}
		project={fx.PROJECT}
		onclose={() => {}}
		onsaved={() => {}}
	/>
</Story>

<Story name="Any project" asChild>
	<RecordFeedbackModal projects={fx.projects} peopleOptions={fx.peopleOptions} people={fx.people} notes={[]} onclose={() => {}} onsaved={() => {}} />
</Story>
