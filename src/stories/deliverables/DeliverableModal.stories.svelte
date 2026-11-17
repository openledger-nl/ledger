<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import DeliverableModal from '$lib/components/DeliverableModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Deliverables/DeliverableModal', component: DeliverableModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="New deliverable" asChild>
	<DeliverableModal project={fx.PROJECT} onclose={() => {}} />
</Story>

<Story name="Answering the main question" asChild>
	<DeliverableModal project={fx.PROJECT} answers={fx.projectMeta.mainQuestion} onclose={() => {}} />
</Story>
