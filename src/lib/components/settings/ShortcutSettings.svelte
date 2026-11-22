<script lang="ts">
	// Keyboard shortcuts, on a tab of their own: how they work on top, then every command with its key, as long as the
	// list is (the page scrolls, not a box in it). A key for any command in the palette, a page included. Click a
	// shortcut and press the new one; a key another command had moves over. Kept on this machine.
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import Search from '@lucide/svelte/icons/search';
	import X from '@lucide/svelte/icons/x';
	import HotkeyInput from '$lib/components/HotkeyInput.svelte';
	import { commands, keyOf, prettyKeyOf, resetKey, setKey, shortcuts, type Command } from '$lib/commands.svelte';
	import { prettyHotkey } from '$lib/hotkey';
	import { toast } from '$lib/ui.svelte';

	let query = $state('');
	const all = $derived(commands());

	/** Before anything is typed: the commands and main pages, and any project or thread that has a key. Typing finds all of them. */
	const shown = $derived.by(() => {
		const words = query.toLowerCase().split(/\s+/).filter(Boolean);
		if (!words.length) return all.filter((c) => (c.group !== 'Project' && c.group !== 'Thread') || keyOf(c));
		return all.filter((c) => {
			const hay = `${c.title} ${c.hint ?? ''} ${c.group} ${c.keywords ?? ''}`.toLowerCase();
			return words.every((w) => hay.includes(w));
		});
	});
	const paletteKey = $derived(prettyKeyOf('palette'));
	const changed = $derived(Object.keys(shortcuts.keys).length > 0);

	function set(c: Command, key: string) {
		const taken = setKey(c, key, all);
		if (taken) toast(`${prettyHotkey(key)} now starts "${c.title}", no longer "${taken.title}"`);
	}

	function resetAll() {
		shortcuts.keys = {};
		try {
			localStorage.removeItem('ledger-shortcuts');
		} catch {
			// Reset for this session.
		}
	}
</script>

<div class="panel">
	<h2>Keyboard shortcuts</h2>
	<p class="sub" style="margin:0">
		{#if paletteKey}Press {paletteKey} on any screen for{:else}Open{/if} the command palette: type a few letters of a command or a page and
		press Enter. Any of them can have a key of its own, kept on this machine. Click a shortcut and press the new one; Esc leaves it
		as it was.
	</p>
	<p class="sub" style="margin:0">
		A key with Ctrl, Alt or Super works everywhere, also while typing; a plain key (F1 to F12) only when no text field has the focus. The screenshot
		hotkey works outside Ledger too and is set under Tools, Screenshots.
	</p>
	<div class="form-row">
		<label class="sc-search">
			<Search size={14} />
			<input type="search" bind:value={query} placeholder="Find a command, project or thread" aria-label="Find a command" spellcheck="false" autocomplete="off" />
		</label>
		<button class="btn ghost" onclick={resetAll} disabled={!changed}><RotateCcw size={14} />Reset all</button>
	</div>

	<ul class="sc-list">
		{#each shown as c (c.id)}
			{@const key = keyOf(c)}
			<li>
				<span class="sc-name">
					{c.title}
					<span class="hint">{c.group}{c.hint ? ` | ${c.hint}` : ''}</span>
				</span>
				<span class="sc-key">
					<HotkeyInput bind:value={() => key, (next) => set(c, next)} placeholder="None" label="Shortcut for {c.title}" />
				</span>
				<button class="icon-btn" title="No shortcut" aria-label="No shortcut for {c.title}" onclick={() => setKey(c, '', all)} disabled={!key}>
					<X size={14} />
				</button>
				<button
					class="icon-btn"
					title={c.defaultKey ? `Back to ${prettyHotkey(c.defaultKey)}` : 'Back to none'}
					aria-label="Reset the shortcut for {c.title}"
					onclick={() => resetKey(c, all)}
					disabled={!(c.id in shortcuts.keys)}
				>
					<RotateCcw size={14} />
				</button>
			</li>
		{:else}
			<li class="hint">No command matches "{query.trim()}".</li>
		{/each}
	</ul>
</div>

<style>
	.sc-search {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 10px;
		border: 1px solid var(--line);
		border-radius: 8px;
		background: var(--field);
		color: var(--muted);
		min-width: 0;
	}
	.sc-search input {
		flex: 1;
		min-width: 0;
		border: 0;
		background: transparent;
		padding: 8px 0;
		outline: none;
	}
	.sc-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
	}
	.sc-list li {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 0;
		border-bottom: 1px solid var(--line);
	}
	.sc-list li:last-child {
		border-bottom: 0;
	}
	.sc-name {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		font-size: 13.5px;
	}
	.sc-name .hint {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.sc-key {
		width: 190px;
		flex-shrink: 0;
	}
	.sc-key :global(input) {
		width: 100%;
	}
	@media (max-width: 720px) {
		.sc-key {
			width: 130px;
		}
	}
</style>
