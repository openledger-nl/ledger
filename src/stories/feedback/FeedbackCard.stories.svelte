<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import FeedbackCard from '$lib/components/FeedbackCard.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Feedback/FeedbackCard', component: FeedbackCard });
</script>

<Story name="Open" asChild>
	<FeedbackCard project={fx.PROJECT} thread={fx.THREAD} feedback={fx.feedbackOpen} />
</Story>

<Story name="Resolved" asChild>
	<FeedbackCard project={fx.PROJECT} thread={fx.THREAD} feedback={fx.feedbackResolved} />
</Story>

<Story name="On a passage, on the deliverable" asChild>
	<FeedbackCard project={fx.PROJECT} thread={fx.THREAD} feedback={fx.feedbackQuoted} onDeliverable version="1.0" threadTitle="Sensor research" onlocate={() => {}} />
</Story>

<Story name="With a reply" asChild>
	<FeedbackCard
		project={fx.PROJECT}
		thread={fx.THREAD}
		feedback={fx.feedbackOpen}
		replies={[{ ...fx.feedbackResolved, id: 'f-9', replyTo: 'f-1', body: 'Added it, thanks!', html: '<p>Added it, thanks!</p>', kind: 'own', from: null }]}
	/>
</Story>

<Story name="From Portflow" asChild>
	<FeedbackCard project={fx.PROJECT} thread="project-plan" feedback={{ ...fx.feedbackOpen, portflowComment: 9001, evidence: 'project-plan' }} evidenceName="Project plan" portflowItem="Project plan" />
</Story>
