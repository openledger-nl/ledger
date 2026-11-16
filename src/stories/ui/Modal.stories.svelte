<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Modal from '$lib/components/Modal.svelte';

	const { Story } = defineMeta({ title: 'UI/Modal', component: Modal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="With a footer" asChild>
	<Modal title="Save the file" onclose={() => {}}>
		<p>Files are kept in a thread of a project.</p>
		{#snippet footer()}
			<span class="spacer-h"></span>
			<button class="btn ghost">Cancel</button>
			<button class="btn primary">Save</button>
		{/snippet}
	</Modal>
</Story>

<Story name="Wide" asChild>
	<Modal title="New project" wide onclose={() => {}}>
		<p>A wide dialog, for forms with two columns.</p>
	</Modal>
</Story>
