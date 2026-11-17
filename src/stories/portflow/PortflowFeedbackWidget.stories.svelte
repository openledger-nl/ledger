<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PortflowFeedbackWidget from '$lib/components/PortflowFeedbackWidget.svelte';

	const { Story } = defineMeta({ title: 'Portflow/PortflowFeedbackWidget', component: PortflowFeedbackWidget });
</script>

<!-- Recent feedback from Portflow; the sample backend has none, so it shows its empty state. -->
<Story name="Default" args={{ version: 0 }} />
