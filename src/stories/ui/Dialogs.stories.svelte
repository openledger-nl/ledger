<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Dialogs from '$lib/components/Dialogs.svelte';
	import Setup from '../Setup.svelte';
	import { confirmDialog, dialog, promptDialog } from '$lib/ui.svelte';

	const { Story } = defineMeta({ title: 'UI/Dialogs', component: Dialogs, parameters: { layout: 'fullscreen' } });
</script>

<!-- Ledger's own confirm and prompt dialogs, opened through confirmDialog() and promptDialog(). -->
<Story name="Confirm, dangerous" asChild>
	<Setup
		run={() => confirmDialog({ title: 'Delete this thread?', message: 'Its posts, prompts and feedback go too.', confirmLabel: 'Delete', danger: true })}
		reset={() => (dialog.current = null)}
	>
		<Dialogs />
	</Setup>
</Story>

<Story name="Prompt" asChild>
	<Setup run={() => promptDialog({ title: 'Rename thread', label: 'Title', initial: 'Sensor research' })} reset={() => (dialog.current = null)}>
		<Dialogs />
	</Setup>
</Story>
