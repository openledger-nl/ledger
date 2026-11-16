<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import SelectionComment from '$lib/components/SelectionComment.svelte';

	const { Story } = defineMeta({ title: 'Feedback/SelectionComment', component: SelectionComment });
</script>

<script lang="ts">
	let root = $state<HTMLElement>();
</script>

<!-- Select a few words in the text: a comment button appears beside the selection. -->
<Story name="Select text to comment" asChild>
	<div bind:this={root} class="md" style="max-width: 560px">
		<p>Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.</p>
		<p>Order three capacitive sensors and test them for two sprints.</p>
	</div>
	<SelectionComment {root} oncomment={() => {}} />
</Story>
