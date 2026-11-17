<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'UI/MarkdownEditor', component: MarkdownEditor });
</script>

<script lang="ts">
	let note = $state('Capacitive sensors **corrode** far less. See [[Project plan]].');
	let doc = $state(fx.deliverable.body ?? '');
	let notes = $state('Discussed the plan with ');
</script>

<Story name="Field" asChild>
	<MarkdownEditor bind:value={note} label="Note" placeholder="What did you find?" />
</Story>

<Story name="Document" asChild>
	<MarkdownEditor bind:value={doc} variant="doc" minHeight="50vh" label="sensor-research.md" marks={fx.marks} />
</Story>

<Story name="Composer" asChild>
	<MarkdownEditor value="" variant="composer" placeholder="Write a post... Ctrl+Enter sends" submitOnEnter />
</Story>

<Story name="Mention people with @" asChild>
	<MarkdownEditor
		bind:value={notes}
		rows={4}
		label="Notes"
		placeholder="Type @ to mention someone."
		people={fx.people.map((p) => ({ id: p.tag, label: p.name, detail: p.role, link: `person:${p.tag}` }))}
	/>
</Story>

<Story name="Source mode" asChild>
	<MarkdownEditor bind:value={note} source label="Note" />
</Story>
