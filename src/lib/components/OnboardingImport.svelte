<script lang="ts">
	// Bringing in a vault that exists: a folder on this computer, which cloud storage keeps in sync with your other
	// computers. Part of the setup guide (Onboarding), which closes as soon as the vault has something in it.
	import { open } from '@tauri-apps/plugin-dialog';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import { api } from '$lib/api';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import type { Settings } from '$lib/types';

	/** `ondone`: the vault has something in it; `onempty`: the folder chosen is empty, so it is a new vault after all; `onback`: back to the choice. */
	let {
		settings,
		ondone,
		onempty,
		onback
	}: {
		settings: Settings;
		ondone: () => void;
		onempty: () => void;
		onback: () => void;
	} = $props();

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

</script>

<div class="onb-import">
	<button class="link onb-back" onclick={onback}><ArrowLeft size={13} />Back</button>
	<h2>Import an existing vault</h2>
	<p class="sub">Pick its folder: the one your cloud storage (OneDrive, SharePoint, Nextcloud) keeps in sync with your other computers. Your files are not changed: Ledger starts using them where they are.</p>

	{#if settings.vaultLocked}
		<div class="notice info">The vault's location is fixed by the <code>LEDGER_VAULT</code> environment variable, so it can't be changed here.</div>
	{:else}
		<label class="field">
			The vault's folder
			<div class="form-row" style="flex-wrap:nowrap">
				<input class="mono" bind:value={folder} placeholder={settings.vaultPath} spellcheck="false" />
				<button class="btn" onclick={browse}>Browse</button>
			</div>
		</label>
		<div class="hint">The folder with <code>ledger.json</code> in it. A vault made by Ledger 0.x can't be opened: make a new one.</div>
		<div class="row-end">
			<button class="btn primary" onclick={useFolder} disabled={busy || !folder.trim()}>Use this vault</button>
		</div>
	{/if}
</div>
