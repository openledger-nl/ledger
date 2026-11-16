<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Toasts from '$lib/components/Toasts.svelte';
	import Setup from '../Setup.svelte';
	import { toast, toasts } from '$lib/ui.svelte';

	const { Story } = defineMeta({ title: 'UI/Toasts', component: Toasts, parameters: { layout: 'fullscreen' } });
</script>

<!-- Toasts leave by themselves after a few seconds; reopen the story to see them again. -->
<Story name="Message and error" asChild>
	<Setup
		run={() => {
			toast('3 files attached to “Sensor research”');
			toast('Ledger got no file from that drop.', true);
		}}
		reset={() => toasts.splice(0)}
	>
		<Toasts />
	</Setup>
</Story>
