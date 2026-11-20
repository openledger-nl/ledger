<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import ModelPicker from '$lib/components/ModelPicker.svelte';

	const { Story } = defineMeta({ title: 'Claude/ModelPicker', component: ModelPicker });
</script>

<script lang="ts">
	let model = $state('sonnet');
	let own = $state('claude-opus-4-1');
</script>

<!-- Opus, Sonnet, Haiku, Claude Code's default, or "Other..." for a name of your own. -->
<Story name="An alias" asChild>
	<ModelPicker bind:value={model} defaultLabel="Claude Code's default" />
</Story>

<Story name="A name of your own" asChild>
	<ModelPicker bind:value={own} defaultLabel="Claude Code's default" />
</Story>
