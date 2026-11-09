<script lang="ts">
	// What the sidebar becomes when it folds: when you hide it (Ctrl \), and on a window narrower than 1200 px (Ledger
	// snapped to half the screen). Kept in the vault's .ledger/settings.json, which never syncs: each machine has its own.
	import { layout, setFold, type Fold } from '$lib/layout.svelte';
	import { prettyHotkey } from '$lib/hotkey';

	const choices: { value: Fold; label: string }[] = [
		{ value: 'rail', label: 'Icon rail' },
		{ value: 'hide', label: 'Hide it' }
	];
	const key = prettyHotkey('CommandOrControl+\\');
</script>

<div class="panel">
	<h2>Sidebar</h2>
	<p class="sub" style="margin:0">
		What the sidebar becomes when it folds: when you hide it ({key}) and when the window is narrower than 1200 px, for example snapped to half the
		screen. Saved in this vault’s <code>.ledger/settings.json</code>, which never syncs, so each machine can choose its own.
	</p>

	<div class="field">
		<div class="seg" role="group" aria-label="Folded sidebar" style="align-self:flex-start">
			{#each choices as c (c.value)}
				<button class:on={layout.fold === c.value} aria-pressed={layout.fold === c.value} onclick={() => setFold(c.value)}>
					{c.label}
				</button>
			{/each}
		</div>
		<span class="hint">
			{#if layout.fold === 'hide'}
				The page gets the whole width. The button at the left of the top bar brings the sidebar back, over the page on a narrow window.
			{:else}
				A narrow strip stays: the main pages as icons and each project as its initial. The button at the left of the top bar brings the whole sidebar
				back. Below 720 px there is no room for the rail, and the sidebar is hidden either way.
			{/if}
		</span>
	</div>
</div>
