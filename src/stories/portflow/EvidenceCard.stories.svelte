<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import EvidenceCard from '$lib/components/EvidenceCard.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Portflow/EvidenceCard', component: EvidenceCard });
</script>

<Story name="Two versions" asChild>
	<EvidenceCard evidence={fx.evidence} />
</Story>

<Story name="Not in a thread" asChild>
	<EvidenceCard evidence={{ ...fx.evidence, thread: null, threadTitle: null, feedbackCount: 0, openFeedback: 0 }} />
</Story>
