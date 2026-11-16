<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ClaudeRunPanel from '$lib/components/ClaudeRunPanel.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Claude/ClaudeRunPanel', component: ClaudeRunPanel });
</script>

<!-- Runs a prompt with Claude Code on this machine; in Storybook the run itself isn't faked, so it shows its idle state. -->
<Story name="Ready to run" asChild>
	<ClaudeRunPanel project={fx.PROJECT} thread={fx.THREAD} prompt={fx.prompt} />
</Story>
