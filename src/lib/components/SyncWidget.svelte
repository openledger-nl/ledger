<script lang="ts">
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import GitBranch from '@lucide/svelte/icons/git-branch';
	import Lock from '@lucide/svelte/icons/lock';
	import LockOpen from '@lucide/svelte/icons/lock-open';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import Settings from '@lucide/svelte/icons/settings';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { api } from '$lib/api';
	import { ago, plural } from '$lib/format';
	import { attempt, sync } from '$lib/ui.svelte';
	import type { SyncOverview } from '$lib/types';

	// GitHub as the vault's backend, at a glance: where the files go, how the last sync went and what GitHub holds of the
	// vault. The status is the one the sidebar shows, kept current by the backend; what GitHub holds is counted from the
	// sync's bookkeeping (no request), again after every sync that finishes.
	//
	// On the dashboard it is a card of its own, with a way to the settings. `embedded` is for the settings panel itself:
	// no card, no title and no link back, and nothing at all while sync is off (the form is right there).

	let { embedded = false }: { embedded?: boolean } = $props();

	const status = $derived(sync.status);
	let overview = $state<SyncOverview | null>(null);
	/** How many commits the branch holds: asked of GitHub, so it is left out while GitHub can't be reached. */
	let commits = $state<number | null>(null);

	$effect(() => {
		void status?.lastSyncAt;
		void status?.configured;
		let stale = false;
		api.syncOverview()
			.then((o) => {
				if (!stale) overview = o;
			})
			.catch(() => {});
		api.syncCommits()
			.then((n) => {
				if (!stale) commits = n;
			})
			.catch(() => {});
		return () => (stale = true);
	});

	// "just now" and "5 min ago" are worked out when drawn, so the card redraws every half minute to keep them true.
	let tick = $state(0);
	$effect(() => {
		const timer = setInterval(() => tick++, 30_000);
		return () => clearInterval(timer);
	});

	const standing = $derived.by(() => {
		const s = status;
		if (s?.state === 'syncing') return { dot: 'warn pulse', label: 'Syncing…' };
		if (s?.state === 'error') return { dot: 'bad', label: 'Sync problem' };
		if (s?.pending) return { dot: 'warn', label: 'Changes waiting' };
		return { dot: 'ok', label: 'Synced' };
	});
	const last = $derived.by(() => {
		void tick;
		return status?.lastSyncAt ? ago(status.lastSyncAt) : 'Not yet';
	});
	const repoUrl = $derived(status?.repo ? `https://github.com/${status.repo}` : '');
	const head = $derived(overview?.head ?? '');
	const breakdown = $derived(
		overview ? `${plural(overview.notes, 'note')} · ${plural(overview.deliverables, 'deliverable')} · ${plural(overview.attachments, 'attachment')}` : 'Counted after the first sync'
	);

	const open = (url: string) => openUrl(url).catch(() => {});
	const syncNow = () => attempt(() => api.syncNow());
</script>

{#if !embedded || status?.configured}
	<section class="sync-widget" class:panel={!embedded} class:embedded aria-label="GitHub sync">
		<div class="header-top">
			<div class="sw-title">
				{#if !embedded}<h2><GitBranch size={16} />GitHub sync</h2>{/if}
				{#if status?.configured}
					<span class="sw-state"><span class="dot {standing.dot}"></span>{standing.label}{#if status.lastSyncAt} · {last}{/if}</span>
				{/if}
			</div>
			{#if status?.configured}
				<div class="sw-actions">
					<button class="btn small" onclick={syncNow} disabled={status.state === 'syncing'}><RefreshCw size={14} />Sync now</button>
					{#if !embedded}<a class="btn small" href="/settings/sync"><Settings size={14} />Settings</a>{/if}
				</div>
			{/if}
		</div>

		{#if !status?.configured}
			<div class="sw-off">
				<p class="sub">
					The vault lives only on this computer. Connect a private GitHub repository to keep a copy there, and to carry the vault between machines.
				</p>
				<a class="btn primary small" href="/settings/sync">Set up GitHub sync</a>
			</div>
		{:else}
			<div class="sw-repo">
				<code>{status.repo}@{status.branch}</code>
				<button type="button" class="link accent" onclick={() => open(repoUrl)}><ExternalLink size={12} /> Open on GitHub</button>
			</div>

			<div class="sw-stats">
				<div class="sw-stat">
					<span class="k">Last sync</span>
					<b>{last}</b>
					<span class="s">{status.summary ?? ''}</span>
				</div>
				<div class="sw-stat">
					<span class="k">Files on GitHub</span>
					<b>{overview ? overview.files.toLocaleString('en-GB') : '—'}</b>
					<span class="s">{breakdown}</span>
				</div>
				<div class="sw-stat">
					<span class="k">Last commit</span>
					{#if head}
						<button type="button" class="sw-sha" title="Open the commit on GitHub" onclick={() => open(`${repoUrl}/commit/${head}`)}>{head.slice(0, 7)}</button>
						<span class="s">{commits === null ? '' : `${plural(commits, 'commit')} in total`}</span>
					{:else}
						<b>—</b>
						<span class="s">Not synced yet</span>
					{/if}
				</div>
				<div class="sw-stat">
					<span class="k">Waiting to push</span>
					<b>{status.pending ? 'Changes' : 'Nothing'}</b>
					<span class="s">{status.pending ? 'Pushed shortly' : 'GitHub has it all'}</span>
				</div>
				<div class="sw-stat">
					<span class="k">Encryption</span>
					<b class="sw-lock">{#if status.encrypted}<Lock size={15} />Encrypted{:else}<LockOpen size={15} />Not encrypted{/if}</b>
					<span class="s">{overview?.key ? `Key ${overview.key}` : ''}</span>
				</div>
			</div>

			{#if status.state === 'error' && status.lastError}<div class="sw-error">{status.lastError}</div>{/if}
		{/if}
	</section>
{/if}
