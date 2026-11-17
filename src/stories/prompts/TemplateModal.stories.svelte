<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import TemplateModal from '$lib/components/TemplateModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Templates/TemplateModal', component: TemplateModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="New" asChild>
	<TemplateModal scope="" template={null} onclose={() => {}} />
</Story>

<Story name="Editing" asChild>
	<TemplateModal scope="" template={fx.template} onclose={() => {}} />
</Story>
