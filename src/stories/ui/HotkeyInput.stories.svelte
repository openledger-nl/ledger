<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import HotkeyInput from '$lib/components/HotkeyInput.svelte';

	const { Story } = defineMeta({ title: 'Settings/HotkeyInput', component: HotkeyInput });
</script>

<script lang="ts">
	let hotkey = $state('CommandOrControl+Shift+S');
</script>

<!-- Click it and press a key combination to record a new one. -->
<Story name="Screenshot hotkey" asChild>
	<HotkeyInput bind:value={hotkey} />
</Story>

<Story name="Disabled" asChild>
	<HotkeyInput value="CommandOrControl+Shift+S" disabled />
</Story>
