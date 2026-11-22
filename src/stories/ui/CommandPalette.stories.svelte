<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import CommandPalette from '$lib/components/CommandPalette.svelte';
	import Setup from '../Setup.svelte';
	import { palette } from '$lib/commands.svelte';

	const { Story } = defineMeta({ title: 'UI/CommandPalette', component: CommandPalette });
</script>

<!-- Ctrl+P on any screen. Type to find a command or a page; Enter runs it, Esc closes it. -->
<Story name="Open" asChild>
	<Setup run={() => (palette.open = true)} reset={() => (palette.open = false)}>
		<CommandPalette />
	</Setup>
</Story>
