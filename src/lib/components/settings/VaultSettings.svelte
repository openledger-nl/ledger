<script lang="ts">
	// Where the vault lives on this machine: the folder, switching to another vault (the vault picker), moving this one,
	// and reloading it from disk.
	import { open } from '@tauri-apps/plugin-dialog';
	import { goto } from '$app/navigation';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Vault from '@lucide/svelte/icons/vault';
	import { api } from '$lib/api';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import { loadLayoutSettings } from '$lib/layout.svelte';
	import { openVaultPicker } from '$lib/vaults.svelte';
	import type { Settings } from '$lib/types';

	let { settings }: { settings: Settings } = $props();

	/** The vault is called by its folder's name, as in the picker. */
	const vaultName = $derived(settings.vaultPath.split(/[\\/]/).filter(Boolean).at(-1) ?? 'Vault');

	/** The folder the form starts with, read once; an edit stays local until it is used. */
	const initialPath = () => settings.vaultPath;

	let vaultInput = $state(initialPath());
	let copy = $state(true);
	let busy = $state(false);

	const vaultChanged = $derived(vaultInput.trim() !== '' && vaultInput.trim() !== settings.vaultPath);

	async function browse() {
		const chosen = await attempt(() => open({ directory: true, multiple: false, defaultPath: settings.vaultPath }));
		if (typeof chosen === 'string') vaultInput = chosen;
	}

	async function useFolder() {
		const ok = await confirmDialog({
			title: 'Switch vault?',
			message: `Ledger will use ${vaultInput.trim()} from now on. ${copy ? 'If the folder is empty, your current vault is copied into it first. ' : ''}Your current vault is left where it is.`,
			confirmLabel: 'Switch vault'
		});
		if (!ok) return;
		busy = true;
		const next = await attempt(() => api.setVaultPath(vaultInput.trim(), copy));
		busy = false;
		if (!next) return;
		toast('Vault changed');
		await Promise.all([refresh(), loadLayoutSettings()]);
	}

	/** The home page's setup guide, from the start. */
	async function showGuide() {
		if (!(await attempt(() => api.restartOnboarding()))) return;
		await refresh();
		await goto('/');
	}

	async function reload() {
		if ((await attempt(() => api.reloadVault())) !== undefined) {
			toast('Reloaded from disk');
			await refresh();
		}
	}
</script>

<div class="panel">
	<h2>Vault</h2>
	<p class="sub" style="margin:0">
		Everything you write lives in a folder of plain markdown files with YAML frontmatter, much like an Obsidian vault. Keep it in a folder that your
		cloud storage keeps in sync (OneDrive, SharePoint, Nextcloud): it carries the vault to your other computers, keeps earlier versions of every file
		and can put the whole folder back as it was on a day. What the cloud brings in from another computer is read in as soon as it arrives.
	</p>
	<div class="vault-now">
		<div class="vault-now-text">
			<strong>{vaultName}</strong>
			<span class="mono">{settings.vaultPath}</span>
		</div>
		<button class="btn" onclick={openVaultPicker} disabled={settings.vaultLocked}><Vault size={14} />Switch vault</button>
	</div>
	{#if settings.vaultLocked}
		<div class="notice info">The location is fixed by the <code>LEDGER_VAULT</code> environment variable.</div>
	{/if}
	<div class="form-row">
		<button class="btn small" onclick={reload}>Reload from disk</button>
		<button class="btn small" onclick={showGuide} title="Bring back the guide that helps a new client set up">Show the setup guide again</button>
	</div>

	{#if !settings.vaultLocked}
		<details class="more">
			<summary><ChevronRight size={14} class="disclosure" />Move this vault to another folder</summary>
			<label class="field">
				New folder
				<div class="form-row" style="flex-wrap:nowrap">
					<input class="mono" bind:value={vaultInput} spellcheck="false" />
					<button class="btn" onclick={browse}>Browse</button>
				</div>
			</label>
			{#if vaultChanged}
				<label class="check">
					<input type="checkbox" bind:checked={copy} /> If that folder is empty, copy my current vault into it
				</label>
				<div class="row-end">
					<button class="btn ghost" onclick={() => (vaultInput = settings.vaultPath)}>Cancel</button>
					<button class="btn primary" onclick={useFolder} disabled={busy}>Use this folder</button>
				</div>
			{/if}
		</details>
	{/if}

	<details class="more">
		<summary><ChevronRight size={14} class="disclosure" />What is in the folder</summary>
		<p class="sub">
			Ledger loads the vault into memory when it starts and writes every change straight back to the files.
		</p>
		<pre class="mono tree">projects/&lt;project&gt;/project.md                     a project
projects/&lt;project&gt;/README.md                      its reading guide
projects/&lt;project&gt;/threads/&lt;thread&gt;/thread.md      a thread, with posts/ prompts/
                                                 feedback/ uploads/ (&lt;id&gt;.md each)
projects/&lt;project&gt;/userstories/&lt;id&gt;.md          a user story
projects/&lt;project&gt;/tasks/&lt;id&gt;.md                a task on the board
projects/&lt;project&gt;/deliverables/                  what prompts produce, the live files
projects/&lt;project&gt;/versions/                      their published versions (name@v1.0.md)
projects/&lt;project&gt;/versions/&lt;id&gt;.md              how a published version came to be
projects/&lt;project&gt;/attachments/                   uploaded files and screenshots
projects/&lt;project&gt;/templates/prompts/&lt;id&gt;.md    templates for this project
projects/&lt;project&gt;/templates/pdf/layout.typ     this project's own PDF layout
meetings/&lt;YYYY&gt;/&lt;MM&gt;/&lt;date&gt;.md                a day's meeting notes
shared/templates/prompts/&lt;id&gt;.md               templates for every project
shared/templates/pdf/layout.typ                the vault's own PDF layout
shared/templates/semesters/&lt;id&gt;.md             what a new semester starts from
shared/semesters/&lt;id&gt;.md                       semesters and their sprints
shared/people/&lt;id&gt;.md                          the people of the vault
shared/clients/&lt;id&gt;.md                         the clients of work projects
shared/portflow/                                what Portflow mirrors
ledger.json                                    says that this folder is a vault</pre>
		<p class="sub">
			This machine's settings for the vault and its secrets are kept outside it, in Ledger's own folder and the system's keychain, so the cloud never
			sees them.
		</p>
	</details>
</div>

<style>
	.vault-now {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		flex-wrap: wrap;
	}
	.vault-now-text {
		display: flex;
		flex-direction: column;
		gap: 3px;
		min-width: 0;
	}
	.vault-now-text .mono {
		color: var(--muted);
		overflow-wrap: anywhere;
	}
	.vault-now .btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
</style>
