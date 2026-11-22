<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import UpdateNotice from '$lib/components/UpdateNotice.svelte';
	import Setup from '../Setup.svelte';
	import { updates } from '$lib/updates.svelte';

	const { Story } = defineMeta({ title: 'Updates/UpdateNotice', component: UpdateNotice, parameters: { layout: 'fullscreen' } });
</script>

<Story name="A new release" asChild>
	<Setup
		run={() =>
			(updates.available = {
				kind: 'release',
				version: '0.9.0',
				currentVersion: '0.8.0',
				date: '2026-11-20',
				major: false,
				notesHtml: '<ul><li>Meeting notes: mention someone by typing @</li><li>Drop pictures and see them before saving</li></ul>',
				breakingHtml: null
			})}
		reset={() => (updates.available = null)}
	>
		<UpdateNotice />
	</Setup>
</Story>

<Story name="A major release" asChild>
	<Setup
		run={() =>
			(updates.available = {
				kind: 'release',
				version: '1.0.0',
				currentVersion: '0.9.0',
				date: '2026-12-01',
				major: true,
				notesHtml: '<ul><li>Meeting notes live in Meetings/</li></ul>',
				breakingHtml: '<ul><li>Update Ledger on every machine that shares the vault.</li></ul>'
			})}
		reset={() => (updates.available = null)}
	>
		<UpdateNotice />
	</Setup>
</Story>
