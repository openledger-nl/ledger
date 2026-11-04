<script lang="ts">
	// Bringing in a vault that exists: from a folder on this computer, or from the GitHub repository it is synced to. Part of
	// the setup guide (Onboarding), which closes as soon as the vault has something in it.
	import { open } from '@tauri-apps/plugin-dialog';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { api } from '$lib/api';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { ProjectSummary, Semester, Settings } from '$lib/types';
	import EncryptionSettings from './settings/EncryptionSettings.svelte';
	import GitHubSettings from './settings/GitHubSettings.svelte';

	/** `ondone`: the vault has something in it; `onempty`: the folder chosen is empty, so it is a new vault after all; `onback`: back to the choice. */
	let {
		settings,
		projects,
		semesters,
		ondone,
		onempty,
		onback
	}: {
		settings: Settings;
		projects: ProjectSummary[];
		semesters: Semester[];
		ondone: () => void;
		onempty: () => void;
		onback: () => void;
	} = $props();

	let from = $state<'folder' | 'github'>('folder');
	let folder = $state('');
	let busy = $state(false);

	async function browse() {
		const chosen = await attempt(() => open({ directory: true, multiple: false, defaultPath: settings.vaultPath }));
		if (typeof chosen === 'string') folder = chosen;
	}

	async function useFolder() {
		busy = true;
		const next = await attempt(() => api.setVaultPath(folder.trim(), false));
		if (next) {
			const [found, plans] = await Promise.all([api.listProjects(), api.listSemesters()]);
			await refresh();
			if (found.length || plans.length) {
				toast('Vault imported');
				ondone();
			} else {
				toast('That folder is empty, so it is your new vault');
				onempty();
			}
		}
		busy = false;
	}

	// From GitHub the files arrive in the background, once the sync is saved: the vault fills up and the guide is done.
	let finished = false;
	$effect(() => {
		if (from !== 'github' || finished || !(projects.length || semesters.length)) return;
		finished = true;
		toast('Vault imported');
		ondone();
	});
</script>

<div class="onb-import">
	<button class="link onb-back" onclick={onback}><ArrowLeft size={13} />Back</button>
	<h2>Import an existing vault</h2>
	<p class="sub">Pick where it is now. Your files are not changed: Ledger starts using them where they are.</p>

	<div class="onb-tabs" role="tablist" aria-label="Where the vault is">
		<button role="tab" aria-selected={from === 'folder'} class:on={from === 'folder'} onclick={() => (from = 'folder')}>A folder on this computer</button>
		<button role="tab" aria-selected={from === 'github'} class:on={from === 'github'} onclick={() => (from = 'github')}>A GitHub repository</button>
	</div>

	{#if from === 'folder'}
		{#if settings.vaultLocked}
			<div class="notice info">The vault's location is fixed by the <code>LEDGER_VAULT</code> environment variable, so it can't be changed here.</div>
		{:else}
			<label class="field">
				The vault's folder
				<div class="form-row" style="flex-wrap:nowrap">
					<input class="mono" bind:value={folder} placeholder={settings.vaultPath} spellcheck="false" />
					<button class="btn" onclick={browse}>Browse…</button>
				</div>
			</label>
			<div class="hint">The folder with <code>projects/</code> in it, such as a vault you opened in Obsidian or cloned from GitHub.</div>
			<div class="row-end">
				<button class="btn primary" onclick={useFolder} disabled={busy || !folder.trim()}>Use this vault</button>
			</div>
		{/if}
	{:else}
		<p class="hint">
			Ledger downloads the repository's files into the vault on this computer. The guide closes by itself once they have arrived.
		</p>
		<details class="more">
			<summary><ChevronRight size={14} class="disclosure" />The repository is encrypted</summary>
			<p class="hint">Import the key from your other computer first, so the files can be opened. Then turn encryption on.</p>
			<EncryptionSettings {settings} />
		</details>
		<GitHubSettings {settings} bare />
	{/if}
</div>
