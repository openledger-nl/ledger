<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import LiveChangesDemo from './LiveChangesDemo.svelte';

	const { Story } = defineMeta({ title: 'Deliverables/LiveChanges', component: LiveChangesDemo });
</script>

<Story name="In the text" asChild>
	<div style="max-width: 1240px"><LiveChangesDemo /></div>
</Story>

<Story name="A change opened" asChild>
	<div style="max-width: 1240px"><LiveChangesDemo open /></div>
</Story>

<Story name="Only in the margin" asChild>
	<div style="max-width: 1240px"><LiveChangesDemo inline={false} /></div>
</Story>

<Story name="Reviewing what AI changed" asChild>
	<div style="max-width: 1240px"><LiveChangesDemo review /></div>
</Story>
