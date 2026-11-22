<script lang="ts">
	// Which Claude Code model: Opus, Sonnet, Haiku, the default, or a name of your own (Other..., a text field beside it).
	// Used in Settings > Claude Code and in a run's confirmation.
	import Dropdown from './Dropdown.svelte';
	import { isKnownModel, modelOptions, OTHER_MODEL } from '$lib/claudeModels';

	let {
		value = $bindable(),
		defaultLabel,
		label = 'Model',
		disabled = false
	}: {
		/** The model name; "" for the default. */
		value: string;
		/** What "" means here. */
		defaultLabel: string;
		label?: string;
		disabled?: boolean;
	} = $props();

	/** Typing a name of your own: the text field shows, and what is typed is the value. */
	let typing = $state(false);
	let typed = $state('');
	let field = $state<HTMLInputElement>();

	// The options as they were when Other was chosen, so the typed name doesn't add an option with every letter.
	const options = $derived(modelOptions(defaultLabel, typing ? '' : value));
	const chosen = $derived(typing ? OTHER_MODEL : value);

	function choose(next: string) {
		if (next === OTHER_MODEL) {
			typing = true;
			typed = isKnownModel(value) ? '' : value;
			queueMicrotask(() => field?.focus());
			return;
		}
		typing = false;
		value = next;
	}

	function type(next: string) {
		typed = next;
		value = next.trim();
	}
</script>

<div class="model-picker">
	<Dropdown bind:value={() => chosen, choose} {options} {label} {disabled} size="field" />
	{#if typing}
		<input
			bind:this={field}
			bind:value={() => typed, type}
			placeholder="e.g. claude-opus-4-1 or sonnet[1m]"
			aria-label="Model name"
			spellcheck="false"
			autocomplete="off"
			{disabled}
			onblur={() => {
				if (!typed.trim()) choose('');
			}}
		/>
	{/if}
</div>

<style>
	.model-picker {
		display: flex;
		gap: 8px;
		align-items: center;
		flex-wrap: wrap;
	}
	.model-picker input {
		flex: 1;
		min-width: 180px;
	}
</style>
