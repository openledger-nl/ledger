<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import CaptureBar from '$lib/components/CaptureBar.svelte';
	import Setup from '../Setup.svelte';
	import { capture } from '$lib/log.svelte';

	const { Story } = defineMeta({ title: 'UI/CaptureBar', component: CaptureBar });
</script>

<!-- Shown only while a debug capture runs (Ctrl+Alt+Shift+D unless changed); the story starts one in Ledger's state, without the backend. -->
<Story name="Recording" asChild>
	<Setup
		run={() => Object.assign(capture, { active: true, startedAt: new Date().toISOString(), events: 12 })}
		reset={() => Object.assign(capture, { active: false, startedAt: null, events: 0 })}
	>
		<CaptureBar />
	</Setup>
</Story>
