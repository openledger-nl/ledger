<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import TemplatesPanel from '$lib/components/TemplatesPanel.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Templates/TemplatesPanel', component: TemplatesPanel });
</script>

<Story name="One template" asChild>
	<TemplatesPanel scope="" templates={[fx.template]} />
</Story>

<Story name="Empty" asChild>
	<TemplatesPanel scope={fx.PROJECT} templates={[]} />
</Story>
