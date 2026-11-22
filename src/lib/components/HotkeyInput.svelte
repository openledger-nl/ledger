<script lang="ts">
	import { accelerator, hotkeyRecorder, prettyHotkey } from '$lib/hotkey';

	/** A key recorder: focus it and press the shortcut you want; Esc leaves it as it was. */
	let {
		value = $bindable(),
		disabled = false,
		placeholder = '',
		label = 'Shortcut',
		onrecording
	}: { value: string; disabled?: boolean; placeholder?: string; label?: string; onrecording?: (recording: boolean) => void } = $props();

	let recording = $state(false);

	function setRecording(next: boolean) {
		if (next === recording) return;
		recording = next;
		hotkeyRecorder.active = next;
		onrecording?.(next);
	}

	function onkeydown(e: KeyboardEvent) {
		if (!recording) return;
		// Nothing typed while recording should reach the page, whatever it would normally do.
		e.preventDefault();
		e.stopPropagation();
		if (e.key === 'Escape') return (e.currentTarget as HTMLElement).blur();
		const next = accelerator(e);
		if (next) {
			value = next;
			(e.currentTarget as HTMLElement).blur();
		}
	}
</script>

<input
	class="hotkey"
	class:recording
	readonly
	{disabled}
	value={recording ? 'Press the new shortcut...' : value ? prettyHotkey(value) : ''}
	{placeholder}
	aria-label={label}
	onfocus={() => setRecording(true)}
	onblur={() => setRecording(false)}
	{onkeydown}
/>
