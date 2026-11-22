<script lang="ts">
	// Shown on every screen while a debug capture runs, so it is never forgotten and can be stopped from anywhere.
	// Its shortcut (Ctrl+Alt+Shift+D unless changed, Settings > Tools > Keyboard shortcuts) starts a capture from any
	// screen, and stops it again.
	import { goto } from '$app/navigation';
	import CircleStop from '@lucide/svelte/icons/circle-stop';
	import { prettyKeyOf, provide } from '$lib/commands.svelte';
	import { capture, startCapture, stopCapture } from '$lib/log.svelte';
	import { attempt, toast } from '$lib/ui.svelte';

	const key = $derived(prettyKeyOf('debug.capture'));

	async function stop() {
		if ((await attempt(stopCapture)) !== undefined) await goto('/settings/logs');
	}

	async function start() {
		if ((await attempt(startCapture)) === undefined) return;
		toast(key ? `Recording. Do what goes wrong, then press ${key} again.` : 'Recording. Do what goes wrong, then stop it at the top.');
	}

	$effect(() =>
		provide('debug.capture', () => {
			if (!capture.busy) void (capture.active ? stop() : start());
		})
	);
</script>

{#if capture.active}
	<div class="capture-bar" role="status">
		<span class="rec"></span>
		<span>Recording a debug capture | {capture.events} steps</span>
		{#if key}<span class="key">{key} stops</span>{/if}
		<button class="btn small" onclick={stop} disabled={capture.busy}><CircleStop size={13} />Stop</button>
	</div>
{/if}

<style>
	.capture-bar {
		position: fixed;
		top: 10px;
		left: 50%;
		transform: translateX(-50%);
		z-index: 70;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 6px 6px 12px;
		background: var(--card);
		border: 1px solid var(--red);
		border-radius: 999px;
		box-shadow: var(--shadow-toast);
		font-size: 12.5px;
		color: var(--ink);
	}
	.rec {
		width: 9px;
		height: 9px;
		border-radius: 50%;
		background: var(--red);
		animation: pulse 1.4s ease-in-out infinite;
	}
	.capture-bar :global(.lucide-icon) {
		margin-right: 4px;
		vertical-align: -2px;
	}
	@keyframes pulse {
		50% {
			opacity: 0.35;
		}
	}
	.key {
		color: var(--muted);
		font-size: 12px;
	}
</style>
