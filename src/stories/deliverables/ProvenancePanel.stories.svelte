<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ProvenancePanel from '$lib/components/ProvenancePanel.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Deliverables/ProvenancePanel', component: ProvenancePanel });
</script>

<Story name="One version" asChild>
	<div style="max-width: 440px"><ProvenancePanel project={fx.PROJECT} doc={fx.deliverable} provenance={fx.provenance} draft={null} /></div>
</Story>

<Story name="Unpublished changes" asChild>
	<div style="max-width: 440px">
		<ProvenancePanel project={fx.PROJECT} doc={fx.deliverable} provenance={fx.provenance} draft={{ since: '1.0', modified: '2026-11-16T11:00:00.000Z', stoppedRun: null }} />
	</div>
</Story>
