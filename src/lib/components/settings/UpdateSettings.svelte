<script lang="ts">
	// Updating Ledger from inside the app: whether it looks by itself, and looking now. A release build looks for releases; a
	// development build hears what `pnpm run dev:app` holds back of the changes in its checkout.
	import { onMount } from 'svelte';
	import Switch from '$lib/components/Switch.svelte';
	import { updatesApi } from '$lib/api';
	import { ago, dateTime } from '$lib/format';
	import { attempt } from '$lib/ui.svelte';
	import { showUpdate, updates } from '$lib/updates.svelte';
	import type { UpdateView } from '$lib/types';

	let view = $state<UpdateView | null>(null);
	let checking = $state(false);
	let result = $state<{ kind: 'ok' | 'info' | 'bad'; message: string } | null>(null);

	onMount(async () => {
		view = (await attempt(() => updatesApi.status())) ?? null;
	});

	const available = $derived(updates.available);
	const statusText = $derived.by(() => {
		if (!view) return '';
		if (available?.kind === 'branch')
			return available.behind
				? `${available.branch} has ${available.behind} new ${available.behind === 1 ? 'commit' : 'commits'}`
				: 'Files in the checkout changed';
		if (available) return `Ledger ${available.version} is available`;
		if (view.unavailable) return 'This build can’t look for updates';
		if (view.checkedAt) return view.following ? `Runs what ${view.following} has` : 'Up to date';
		if (!view.automatic) return 'Automatic checks are off';
		return view.devBuild ? 'Hears of changes in the checkout as they come' : 'Looks shortly after starting, then every few hours';
	});

	async function toggle(automatic: boolean) {
		const saved = await attempt(() => updatesApi.saveSettings(automatic));
		if (saved) view = saved;
	}

	async function check() {
		checking = true;
		result = null;
		try {
			const found = await updatesApi.check();
			result = { kind: view?.unavailable ? 'info' : 'ok', message: found.message };
			if (found.available) showUpdate(found.available);
		} catch (e) {
			result = { kind: 'bad', message: e instanceof Error ? e.message : String(e) };
		}
		checking = false;
		view = (await attempt(() => updatesApi.status())) ?? view;
	}
</script>

<div class="panel">
	<div class="panel-head">
		{#if view}<Switch checked={view.automatic} label="Check for updates automatically" onchange={toggle} />{/if}
		<h2>Updates</h2>
	</div>
	{#if view}
		<div class="kv">
			<dt>Version</dt>
			<dd>Ledger {view.currentVersion}</dd>
			<dt>Updates</dt>
			<dd>
				<span class="dot" class:ok={!available && !!view.checkedAt} class:warn={!!available} class:off={!available && !view.checkedAt}></span>
				{statusText}{#if view.checkedAt}<span class="hint" title={dateTime(view.checkedAt)}> · checked {ago(view.checkedAt)}</span>{/if}
			</dd>
		</div>
		{#if view.unavailable}
			<div class="notice info">{view.unavailable}</div>
		{/if}

		{#if view.devBuild}
			<div class="hint">
				This is a development build: it runs what its checkout{view.following ? ` (${view.following})` : ''} had when it was built, instead of the releases.
				Started with <code>pnpm run dev:app</code>, what changes in the checkout (commits merged into the branch, or files you edit) is built in the
				background and waits for you: with the switch on, Ledger says what is new as it comes, and only <b>Update</b> brings it in (the window reloads, or
				Ledger restarts into the new build), after a sync or a Claude Code run has finished. Nothing is fetched or pulled. With this off, Ledger says
				nothing by itself; <b>Check for updates</b> shows what is waiting. The choice is kept for this computer, in <code>{view.configFile}</code>, not in the vault.
			</div>
		{:else}
			<div class="hint">
				With the switch on, Ledger looks shortly after it starts and every few hours, and tells you when a new version is out. It never installs one until you click
				<b>Install and restart</b>, and then waits for a sync or a Claude Code run to finish first. With this off, Ledger only looks when you click
				<b>Check for updates</b>. The choice is kept for this computer, in <code>{view.configFile}</code>, not in the vault.
			</div>
		{/if}

		{#if result}
			<div class="notice" class:ok={result.kind === 'ok'} class:info={result.kind === 'info'} class:bad={result.kind === 'bad'}>{result.message}</div>
		{/if}
		<div class="row-end" style="justify-content:flex-start">
			<button class="btn" onclick={check} disabled={checking || !!view.unavailable} title={view.unavailable ?? undefined}>
				{checking ? 'Checking…' : 'Check for updates'}
			</button>
			{#if available && updates.hidden}
				<button class="btn ghost" onclick={() => showUpdate(available, true)}>
					{available.kind === 'branch' ? 'Show what is waiting' : `Show Ledger ${available.version}`}
				</button>
			{/if}
		</div>
	{/if}
</div>
