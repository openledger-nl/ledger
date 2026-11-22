<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Markdown from '$lib/components/Markdown.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'UI/Markdown', component: Markdown });
</script>

<Story name="A document" asChild>
	<Markdown html={fx.deliverable.html ?? ''} />
</Story>

<Story name="With a lit-up passage" asChild>
	<Markdown html={fx.deliverable.html ?? ''} marks={fx.marks} onmark={() => {}} />
</Story>

<Story name="With mentions" asChild>
	<Markdown
		html={'<p>See <a class="mention" data-mention="evidence-2100001" data-mention-type="evidence">@Project plan</a>, discussed with <a class="mention person" data-person="coach/frank">@Frank de Vries</a>.</p>'}
		mentions={fx.mentions}
	/>
</Story>
