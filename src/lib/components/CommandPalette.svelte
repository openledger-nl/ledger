<script lang="ts">
	// The command palette (Ctrl+P): type a few letters of a command or a page and press Enter. Before anything is typed it
	// shows what you ran last and the main commands; typing finds every command, project and thread. Each row shows its
	// shortcut; Settings > Tools > Keyboard shortcuts gives any of them one.
	import Keyboard from '@lucide/svelte/icons/keyboard';
	import Search from '@lucide/svelte/icons/search';
	import { goto } from '$app/navigation';
	import { commands, keyOf, noteRecent, palette, recent, type Command } from '$lib/commands.svelte';
	import { prettyHotkey } from '$lib/hotkey';
	import { toast } from '$lib/ui.svelte';

	let query = $state('');
	let selected = $state(0);
	let box = $state<HTMLInputElement>();
	let list = $state<HTMLUListElement>();
	let before: Element | null = null;

	const all = $derived(palette.open ? commands().filter((c) => c.available !== false) : []);

	/** How well a command matches: every word has to be in it; a word at the start of the name (or of a word in it) counts most. */
	function score(c: Command, words: string[]): number | null {
		const title = c.title.toLowerCase();
		const hay = `${title} ${c.hint ?? ''} ${c.group} ${c.keywords ?? ''}`.toLowerCase();
		let total = 0;
		for (const w of words) {
			if (!hay.includes(w)) return null;
			const at = title.indexOf(w);
			total += at === 0 ? 4 : at > 0 && /[^a-z0-9]/.test(title[at - 1]) ? 3 : at > 0 ? 1 : 0;
		}
		return total;
	}

	const results = $derived.by(() => {
		const words = query.toLowerCase().split(/\s+/).filter(Boolean);
		const recentRank = (c: Command) => {
			const i = recent.ids.indexOf(c.id);
			return i < 0 ? recent.ids.length : i;
		};
		if (!words.length) {
			// The recent ones first, then the commands and main pages; projects and threads wait for a word.
			const used = recent.ids.map((id) => all.find((c) => c.id === id)).filter((c): c is Command => !!c);
			const rest = all.filter((c) => !recent.ids.includes(c.id) && (c.group === 'Create' || c.group === 'Go to' || c.group === 'App'));
			return [...used, ...rest];
		}
		return all
			.map((c, i) => ({ c, i, s: score(c, words) }))
			.filter((r): r is { c: Command; i: number; s: number } => r.s !== null)
			.sort((a, b) => b.s - a.s || recentRank(a.c) - recentRank(b.c) || a.i - b.i)
			.slice(0, 80)
			.map((r) => r.c);
	});

	$effect(() => {
		if (!palette.open) return;
		before = document.activeElement;
		query = '';
		selected = 0;
		queueMicrotask(() => box?.focus());
	});

	$effect(() => {
		void results;
		selected = 0;
	});

	$effect(() => {
		list?.querySelector(`[data-index="${selected}"]`)?.scrollIntoView({ block: 'nearest' });
	});

	function close(restore = true) {
		palette.open = false;
		if (restore && before instanceof HTMLElement) before.focus();
	}

	function run(c: Command | undefined) {
		if (!c) return;
		close(false);
		noteRecent(c.id);
		Promise.resolve()
			.then(c.run)
			.catch((err) => toast(err instanceof Error ? err.message : String(err), true));
	}

	/**
	 * The palette's keys, heard on the whole window while it is open (wherever the focus went: a click on the list's
	 * scrollbar, an editor taking it back) and before anything else: Escape closes the palette, not a modal under it.
	 */
	function onkeydown(e: KeyboardEvent) {
		if (!palette.open) return;
		if (e.key !== 'Escape' && e.target instanceof HTMLElement && e.target !== box && e.target.closest('.cp button')) return;
		if (e.key === 'ArrowDown' || (e.ctrlKey && e.key === 'n')) {
			e.preventDefault();
			selected = Math.min(selected + 1, results.length - 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			selected = Math.max(selected - 1, 0);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			run(results[selected]);
		} else if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			close();
			return;
		} else {
			return;
		}
		// The page under the palette doesn't hear the arrows or Enter either.
		e.stopPropagation();
		if (document.activeElement !== box) box?.focus();
	}

	$effect(() => {
		if (!palette.open) return;
		window.addEventListener('keydown', onkeydown, { capture: true });
		return () => window.removeEventListener('keydown', onkeydown, { capture: true });
	});

	function changeShortcuts() {
		close(false);
		void goto('/settings/shortcuts');
	}
</script>

{#if palette.open}
	<div class="cp-scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && close()}>
		<div class="cp" role="dialog" aria-modal="true" aria-label="Command palette">
			<label class="cp-search">
				<Search size={16} />
				<input
					bind:this={box}
					bind:value={query}
					placeholder="Type a command or a page..."
					aria-label="Command"
					aria-controls="cp-list"
					aria-activedescendant={results.length ? `cp-${selected}` : undefined}
					autocomplete="off"
					spellcheck="false"
				/>
			</label>
			<ul class="cp-list" id="cp-list" role="listbox" bind:this={list}>
				{#each results as c, i (c.id)}
					{@const key = keyOf(c)}
					<li
						id="cp-{i}"
						data-index={i}
						role="option"
						aria-selected={i === selected}
						class:on={i === selected}
						onmousemove={() => (selected = i)}
						onmousedown={(e) => e.preventDefault()}
						onclick={() => run(c)}
						onkeydown={() => {}}
					>
						<span class="cp-group">{c.group}</span>
						<span class="cp-title">{c.title}</span>
						{#if c.hint}<span class="cp-hint">{c.hint}</span>{/if}
						{#if key}<kbd>{prettyHotkey(key)}</kbd>{/if}
					</li>
				{:else}
					<li class="cp-empty">Nothing matches "{query.trim()}".</li>
				{/each}
			</ul>
			<div class="cp-foot">
				<span><kbd>Enter</kbd> run</span>
				<span><kbd>Up</kbd> <kbd>Down</kbd> choose</span>
				<span><kbd>Esc</kbd> close</span>
				<button class="btn small ghost" onclick={changeShortcuts}><Keyboard size={13} />Shortcuts</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.cp-scrim {
		position: fixed;
		inset: 0;
		z-index: 60;
		background: var(--scrim-faint);
		display: flex;
		justify-content: center;
		align-items: flex-start;
		padding: 12vh 16px 16px;
	}
	.cp {
		width: min(620px, 100%);
		max-height: 70vh;
		display: flex;
		flex-direction: column;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 14px;
		box-shadow: var(--shadow-modal);
		overflow: hidden;
	}
	.cp-search {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 12px 16px;
		border-bottom: 1px solid var(--line);
		color: var(--muted);
	}
	.cp-search input {
		flex: 1;
		border: 0;
		background: transparent;
		color: var(--ink);
		font: 15px var(--font-sans);
		outline: none;
		padding: 0;
	}
	.cp-list {
		list-style: none;
		margin: 0;
		padding: 6px;
		overflow-y: auto;
		min-height: 0;
	}
	.cp-list li {
		display: flex;
		align-items: baseline;
		gap: 10px;
		padding: 8px 10px;
		border-radius: 8px;
		cursor: pointer;
		font-size: 13.5px;
		color: var(--ink);
	}
	.cp-list li.on {
		background: var(--accent-soft);
	}
	.cp-group {
		flex: 0 0 64px;
		font-size: 11px;
		color: var(--faint);
	}
	.cp-title {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.cp-hint {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 12px;
		color: var(--muted);
	}
	.cp-list li.cp-empty {
		cursor: default;
		color: var(--muted);
	}
	kbd {
		font: 500 11px var(--font-sans);
		color: var(--muted);
		border: 1px solid var(--line);
		border-radius: 5px;
		padding: 1px 6px;
		white-space: nowrap;
		margin-left: auto;
	}
	.cp-foot {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 8px 12px 8px 16px;
		border-top: 1px solid var(--line);
		font-size: 11.5px;
		color: var(--faint);
	}
	.cp-foot kbd {
		margin-left: 0;
	}
	.cp-foot button {
		margin-left: auto;
	}
	@media (max-width: 720px) {
		.cp-foot span {
			display: none;
		}
	}
</style>
