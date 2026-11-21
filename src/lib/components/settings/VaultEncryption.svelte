<script lang="ts">
	// Settings > Vault: whether the vault is encrypted. An encrypted one can get a new passphrase, show its recovery key
	// and stop being remembered on this computer. One of plain files can't be encrypted where it is (its plain files
	// would stay in the cloud's version history), so it gets an encrypted copy in a new folder instead.
	import { open as pickFolder } from '@tauri-apps/plugin-dialog';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Lock from '@lucide/svelte/icons/lock';
	import LockOpen from '@lucide/svelte/icons/lock-open';
	import { onMount } from 'svelte';
	import { api } from '$lib/api';
	import RecoveryKey from '$lib/components/RecoveryKey.svelte';
	import { attempt, toast } from '$lib/ui.svelte';
	import type { EncryptionView, Settings } from '$lib/types';

	let { settings }: { settings: Settings } = $props();

	const MIN_PASSPHRASE = 10;
	let view = $state<EncryptionView | null>(null);
	onMount(async () => {
		view = await api.vaultEncryption().catch(() => null);
	});

	// ---- an encrypted vault
	let current = $state('');
	let next = $state('');
	let again = $state('');
	let busy = $state(false);
	const nextOk = $derived(next.length >= MIN_PASSPHRASE && next === again);

	async function changePassphrase() {
		if (!current || !nextOk) return;
		busy = true;
		const done = await attempt(() => api.changePassphrase(current, next));
		busy = false;
		if (!done) return;
		view = done;
		current = next = again = '';
		toast('The passphrase is changed. Other computers keep their key; a new one takes the new passphrase.');
	}

	let asked = $state('');
	let shown = $state('');
	async function showRecovery() {
		if (!asked) return;
		busy = true;
		const key = await attempt(() => api.recoveryKey(asked));
		busy = false;
		asked = '';
		if (key) shown = key;
	}

	async function forget() {
		const done = await attempt(() => api.forgetVaultKey());
		if (done) {
			view = done;
			toast('Forgotten on this computer: the next start asks for the passphrase');
		}
	}

	// ---- an encrypted copy of a vault of plain files
	const folderOf = (path: string) => path.replace(/[\\/][^\\/]*$/, '');
	const nameOf = (path: string) => path.split(/[\\/]/).pop() ?? 'Vault';
	// The form starts beside the vault, once; edits stay in it.
	// svelte-ignore state_referenced_locally
	let copyParent = $state(folderOf(settings.vaultPath));
	// svelte-ignore state_referenced_locally
	let copyName = $state(`${nameOf(settings.vaultPath)} (encrypted)`);
	let copyPass = $state('');
	let copyAgain = $state('');
	const copyOk = $derived(copyParent.trim() !== '' && copyName.trim() !== '' && copyPass.length >= MIN_PASSPHRASE && copyPass === copyAgain);
	let copied = $state('');

	async function browse() {
		const chosen = await attempt(() => pickFolder({ directory: true, multiple: false, defaultPath: copyParent || undefined, title: 'Where to put the encrypted copy' }));
		if (typeof chosen === 'string') copyParent = chosen;
	}

	async function makeCopy() {
		if (!copyOk) return;
		busy = true;
		const done = await attempt(() => api.encryptedCopy(copyParent.trim(), copyName.trim(), copyPass));
		busy = false;
		copyPass = copyAgain = '';
		if (done?.recovery) copied = done.recovery;
	}
</script>

<div class="panel">
	<h2>Encryption</h2>
	{#if view === null}
		<p class="hint">Looking...</p>
	{:else if view.encrypted}
		<p class="sub" style="margin:0">
			<Lock size={14} /> This vault is encrypted (key {view.keyId}): every file is kept encrypted on disk and in the cloud, its name too. Only Ledger, with the
			passphrase, reads it.
			{#if view.kept}This computer keeps the key in its keychain, so Ledger opens the vault without asking.{:else}This computer doesn't keep the key: Ledger asks
				for the passphrase at every start.{/if}
		</p>
		<p class="hint">
			A picture, a PDF or a layout you open with its own program is a plain copy outside the synced folder; your changes to it are taken in when you come
			back to Ledger.
		</p>

		<details class="more">
			<summary><ChevronRight size={14} class="disclosure" />Change the passphrase</summary>
			<label class="field">
				The passphrase now (or the recovery key)
				<input type="password" bind:value={current} autocomplete="current-password" />
			</label>
			<div class="form-row">
				<label class="field grow">
					New passphrase
					<input type="password" bind:value={next} autocomplete="new-password" placeholder="At least {MIN_PASSPHRASE} characters" />
				</label>
				<label class="field grow">
					Again
					<input type="password" bind:value={again} autocomplete="new-password" />
				</label>
			</div>
			<div class="row-end"><button class="btn primary" onclick={changePassphrase} disabled={busy || !current || !nextOk}>Change passphrase</button></div>
		</details>

		<details class="more">
			<summary><ChevronRight size={14} class="disclosure" />Show the recovery key</summary>
			{#if shown}
				<RecoveryKey value={shown} ondone={() => (shown = '')} done="Hide it" />
			{:else}
				<label class="field">
					Passphrase
					<input type="password" bind:value={asked} autocomplete="current-password" />
				</label>
				<div class="row-end"><button class="btn" onclick={showRecovery} disabled={busy || !asked}>Show</button></div>
			{/if}
		</details>

		{#if view.kept}
			<div class="form-row">
				<button class="btn small" onclick={forget}>Forget the key on this computer</button>
			</div>
		{/if}
	{:else}
		<p class="sub" style="margin:0">
			<LockOpen size={14} /> This vault is plain markdown files: any program (Obsidian, an editor) can read them, and so can your cloud storage. An encrypted vault
			keeps every file encrypted on disk and in the cloud, readable only by Ledger with your passphrase.
		</p>
		<details class="more">
			<summary><ChevronRight size={14} class="disclosure" />Make an encrypted copy</summary>
			{#if copied}
				<RecoveryKey value={copied} ondone={() => window.location.assign('/')} done="I have kept it: open the copy" />
			{:else}
				<p class="hint">
					The copy goes in a new folder and is opened at once; this vault stays as it is. Delete it yourself once the copy has synced: until your cloud
					storage empties its recycle bin, its plain files are still there.
				</p>
				<label class="field">
					Put it in
					<div class="form-row" style="flex-wrap:nowrap">
						<input class="mono" bind:value={copyParent} spellcheck="false" />
						<button class="btn" onclick={browse}>Browse</button>
					</div>
				</label>
				<label class="field">
					Name
					<input bind:value={copyName} spellcheck="false" />
				</label>
				<div class="form-row">
					<label class="field grow">
						Passphrase
						<input type="password" bind:value={copyPass} autocomplete="new-password" placeholder="At least {MIN_PASSPHRASE} characters" />
					</label>
					<label class="field grow">
						Again
						<input type="password" bind:value={copyAgain} autocomplete="new-password" />
					</label>
				</div>
				<div class="row-end"><button class="btn primary" onclick={makeCopy} disabled={busy || !copyOk}><Lock size={14} />Make the encrypted copy</button></div>
			{/if}
		</details>
	{/if}
</div>
