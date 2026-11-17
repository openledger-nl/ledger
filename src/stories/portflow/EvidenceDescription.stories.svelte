<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import EvidenceDescription from '$lib/components/EvidenceDescription.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Portflow/EvidenceDescription', component: EvidenceDescription });
</script>

<Story name="In the teachers' shape" asChild>
	<EvidenceDescription evidence={fx.evidence} />
</Story>

<Story name="Free text" asChild>
	<EvidenceDescription evidence={{ ...fx.evidence, description: 'A plan for the semester.', descriptionParts: null, descriptionHtml: '<p>A plan for the semester.</p>' }} />
</Story>

<Story name="Empty" asChild>
	<EvidenceDescription evidence={{ ...fx.evidence, description: '', descriptionParts: null, descriptionHtml: '' }} />
</Story>

<Story name="A deliverable's, not yet written" asChild>
	<EvidenceDescription deliverable={{ project: 'sample-project', path: 'Research plan.md', description: '', descriptionHtml: '', descriptionParts: null }} />
</Story>

<Story name="A deliverable's, in the shape" asChild>
	<EvidenceDescription
		deliverable={{
			project: 'sample-project',
			path: 'Research plan.md',
			description: fx.evidence.description,
			descriptionHtml: fx.evidence.descriptionHtml,
			descriptionParts: fx.evidence.descriptionParts
		}}
	/>
</Story>
