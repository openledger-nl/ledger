<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import IntendedDevelopmentModal from '$lib/components/IntendedDevelopmentModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Portflow/IntendedDevelopmentModal', component: IntendedDevelopmentModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="Default" asChild>
	<IntendedDevelopmentModal view={fx.epsilon} semesterName="S3 - najaar 2026" onclose={() => {}} onsaved={() => {}} />
</Story>
