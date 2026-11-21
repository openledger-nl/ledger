<script lang="ts">
	// Choosing the vault, as Obsidian does: the vaults opened on this machine on the left, and on the right making a new
	// one or opening a folder that is there. It is the whole window before the first vault (the first start goes
	// straight to making one), and after that an overlay in the middle of the window (vaults.svelte.ts). Opening a vault reloads the window on it, so nothing
	// of the vault before is left in any page.
	import { open as pickFolder } from '@tauri-apps/plugin-dialog';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import Cloud from '@lucide/svelte/icons/cloud';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import FolderPlus from '@lucide/svelte/icons/folder-plus';
	import Lock from '@lucide/svelte/icons/lock';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import { attempt } from '$lib/ui.svelte';
	import type { VaultEntry, Vaults } from '$lib/types';
	import RecoveryKey from './RecoveryKey.svelte';
	import Wordmark from './Wordmark.svelte';

	/** `modal`: switching vaults, over the app, with a way back. */
	let { vaults: initial, modal = false, onclose }: { vaults: Vaults; modal?: boolean; onclose?: () => void } = $props();

	/** What the picker opened with, read once: after that it keeps its own list. */
	const start = () => initial;
	const firstStart = () => !start().open && start().vaults.length === 0;
	let vaults = $state(start());
	let view = $state<'choose' | 'create' | 'unlock' | 'recovery'>(firstStart() ? 'create' : 'choose');
	let busy = $state(false);

	// ---- an encrypted vault: its passphrase when it is made, and to open one this computer has no key for

	let encrypt = $state(false);
	let passphrase = $state('');
	let repeat = $state('');
	const MIN_PASSPHRASE = 10;
	const passphraseOk = $derived(!encrypt || (passphrase.length >= MIN_PASSPHRASE && passphrase === repeat));
	/** The recovery key of the vault just made, shown once before the window opens on it. */
	let recovery = $state('');

	/** The encrypted vault to open, and what is typed to open it. */
	let locked = $state<{ path: string; name: string } | null>(null);
	let secret = $state('');
	let keep = $state(true);

	function askKey(path: string, name = path.split(/[\\/]/).pop() ?? path) {
		locked = { path, name };
		secret = '';
		view = 'unlock';
	}

	async function unlock() {
		if (!locked || !secret) return;
		busy = true;
		const done = await attempt(() => api.unlockVault(locked?.path ?? '', secret, keep));
		secret = '';
		if (done) return opened();
		busy = false;
	}

	// ---- a new vault

	let name = $state('Ledger');
	let parent = $state(start().suggestedParent ?? '');
	const separator = $derived(parent.includes('\\') && !parent.includes('/') ? '\\' : '/');
	const target = $derived(parent.trim() && name.trim() ? `${parent.trim().replace(/[\\/]+$/, '')}${separator}${name.trim()}` : '');

	async function browseParent() {
		const chosen = await attempt(() => pickFolder({ directory: true, multiple: false, defaultPath: parent || undefined, title: 'Where to put the vault' }));
		if (typeof chosen === 'string') parent = chosen;
	}

	async function create() {
		if (!passphraseOk) return;
		busy = true;
		const done = await attempt(() => api.createVault(parent.trim(), name.trim(), encrypt ? passphrase : null));
		passphrase = repeat = '';
		if (done?.recovery) {
			recovery = done.recovery;
			view = 'recovery';
			busy = false;
			return;
		}
		if (done) return opened();
		busy = false;
	}

	// ---- a vault that is there

	async function openFolder() {
		const chosen = await attempt(() => pickFolder({ directory: true, multiple: false, title: 'Open folder as vault' }));
		if (typeof chosen === 'string') await openPath(chosen);
	}

	async function openPath(path: string) {
		busy = true;
		try {
			await api.openVault(path);
			return opened();
		} catch (e) {
			busy = false;
			const message = e instanceof Error ? e.message : String(e);
			// An encrypted vault this computer has no key for: it takes the passphrase.
			if (message.includes('enter its passphrase')) return askKey(path);
			await attempt(() => Promise.reject(e));
		}
	}

	async function forget(vault: VaultEntry) {
		const next = await attempt(() => api.forgetVault(vault.path));
		if (next) vaults = next;
	}

	/** The window starts again on the vault now open: every page, list and setting comes from it. */
	function opened() {
		window.location.assign('/');
	}
</script>

<svelte:window onkeydown={(e) => modal && onclose && e.key === 'Escape' && !busy && onclose()} />

<div class="vp" class:vp-modal={modal} class:vp-screen={!modal}>
	<div class="vp-card" role={modal ? 'dialog' : undefined} aria-modal={modal ? 'true' : undefined} aria-label="Choose a vault">
		{#if vaults.vaults.length}
			<aside class="vp-list" aria-label="Your vaults">
				<h3>Your vaults</h3>
				<ul>
					{#each vaults.vaults as vault (vault.path)}
						<li class:current={vault.open} class:gone={!vault.exists || !!vault.refused}>
							<button
								class="vp-vault"
								disabled={busy || vault.open || !vault.exists || !!vault.refused || vaults.locked}
								onclick={() => (vault.needsKey ? askKey(vault.path, vault.name) : openPath(vault.path))}
								title={vault.refused ?? (vault.exists ? vault.path : `${vault.path} isn't there any more`)}
							>
								<span class="vp-name"
									>{vault.name}{#if vault.encrypted}<span class="vp-lock" title="Encrypted"><Lock size={11} /></span>{/if}{#if vault.open}<span class="vp-tag">open</span>{/if}</span
								>
								<span class="vp-path mono">{vault.path}</span>
								{#if !vault.exists}<span class="vp-where">Folder not found</span>{:else if vault.refused}<span class="vp-where">Can't be opened by this Ledger</span>{/if}
							</button>
							{#if !vault.open}
								<button class="vp-forget" aria-label="Remove {vault.name} from the list" title="Remove from the list (the folder is left alone)" onclick={() => forget(vault)}>
									<X size={14} />
								</button>
							{/if}
						</li>
					{/each}
				</ul>
			</aside>
		{/if}

		<section class="vp-main">
			{#if modal && onclose}
				<button class="vp-close icon-btn" aria-label="Close" onclick={onclose}><X size={18} /></button>
			{/if}
			<div class="vp-brand"><Wordmark on="page" height={34} /></div>

			{#if view === 'recovery'}
				<h2>Your vault is encrypted</h2>
				<p class="vp-lead">Everything in it is kept encrypted on disk and in the cloud. Open it on another computer with your passphrase.</p>
				<RecoveryKey value={recovery} ondone={opened} done="I have kept it: open the vault" />
			{:else if view === 'unlock' && locked}
				{#if !firstStart()}
					<button class="link vp-back" onclick={() => ((view = 'choose'), (locked = null))}><ArrowLeft size={13} />Back</button>
				{/if}
				<h2><Lock size={18} /> Open {locked.name}</h2>
				<p class="vp-lead">This vault is encrypted. Enter its passphrase, or its recovery key when the passphrase is lost.</p>
				<form
					class="vp-form"
					onsubmit={(e) => {
						e.preventDefault();
						unlock();
					}}
				>
					<label class="field">
						Passphrase or recovery key
						<!-- svelte-ignore a11y_autofocus -->
						<input type="password" bind:value={secret} autocomplete="current-password" autofocus />
					</label>
					<label class="check"><input type="checkbox" bind:checked={keep} /> Remember on this computer (in its keychain), so Ledger doesn't ask again</label>
					<div class="row-end">
						<button type="submit" class="btn primary" disabled={busy || !secret}><Lock size={14} />Open vault</button>
					</div>
				</form>
			{:else if vaults.locked}
				<div class="notice info">The vault is fixed by the <code>LEDGER_VAULT</code> environment variable, so no other can be opened.</div>
			{:else if view === 'choose'}
				<p class="vp-lead">A vault is a folder of plain markdown files, like an Obsidian vault. Everything you write in Ledger lives in it.</p>
				<div class="vp-actions">
					<div class="vp-action">
						<div>
							<h4>Create new vault</h4>
							<p>A new, empty folder for your projects.</p>
						</div>
						<button class="btn primary" disabled={busy} onclick={() => (view = 'create')}><FolderPlus size={14} />Create</button>
					</div>
					<div class="vp-action">
						<div>
							<h4>Open folder as vault</h4>
							<p>A folder that is a Ledger vault already, synced here from another computer, say.</p>
						</div>
						<button class="btn" disabled={busy} onclick={openFolder}><FolderOpen size={14} />Open</button>
					</div>
				</div>
			{:else}
				{#if !firstStart()}
					<button class="link vp-back" onclick={() => (view = 'choose')}><ArrowLeft size={13} />Back</button>
				{/if}
				<h2>{firstStart() ? 'Welcome to Ledger' : 'Create new vault'}</h2>
				<p class="vp-lead">
					{#if firstStart()}Start by making your vault: the folder where everything you write is kept, as plain markdown files.{:else}A new, empty
						folder for your projects.{/if}
				</p>
				<form
					class="vp-form"
					onsubmit={(e) => {
						e.preventDefault();
						create();
					}}
				>
					<label class="field">
						Vault name
						<input bind:value={name} spellcheck="false" placeholder="Ledger" />
					</label>
					<label class="field">
						Location
						<div class="form-row" style="flex-wrap:nowrap">
							<input class="mono" bind:value={parent} spellcheck="false" placeholder="Pick a folder" />
							<button type="button" class="btn" onclick={browseParent}>Browse</button>
						</div>
					</label>
					{#if target}
						<div class="vp-target">Your vault will be <span class="mono">{target}</span></div>
					{/if}
					<div class="vp-cloud">
						<Cloud size={14} />
						<span>Put your vault in a folder that cloud storage keeps in sync, such as OneDrive, SharePoint or Nextcloud: it carries the vault to your other computers, keeps earlier versions of every file and can put the folder back as it was on a day.</span>
					</div>
					<label class="check"><input type="checkbox" bind:checked={encrypt} /> Encrypt this vault</label>
					{#if encrypt}
						<div class="hint">
							Every file is kept encrypted on disk and in the cloud, its name too; only Ledger, with your passphrase, can read it. Other programs (Obsidian, an
							editor) can't. Choose this now: a vault can't be encrypted later, only copied into an encrypted one.
						</div>
						<div class="form-row">
							<label class="field grow">
								Passphrase
								<input type="password" bind:value={passphrase} autocomplete="new-password" placeholder="At least {MIN_PASSPHRASE} characters" />
							</label>
							<label class="field grow">
								Again
								<input type="password" bind:value={repeat} autocomplete="new-password" />
							</label>
						</div>
						{#if passphrase && repeat && passphrase !== repeat}<div class="hint error-text">The two passphrases differ.</div>{/if}
					{/if}
					<div class="row-end">
						{#if firstStart()}
							<button type="button" class="btn ghost" disabled={busy} onclick={openFolder}><FolderOpen size={14} />Open an existing vault</button>
						{/if}
						<button type="submit" class="btn primary" disabled={busy || !target || !passphraseOk}><FolderPlus size={14} />Create vault</button>
					</div>
				</form>
			{/if}
		</section>
	</div>
</div>

<style>
	.vp {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px 16px;
	}
	.vp-screen {
		min-height: 100vh;
		background: var(--bg);
	}
	.vp-modal {
		position: fixed;
		inset: 0;
		z-index: 60;
		background: color-mix(in srgb, var(--bg) 70%, transparent);
	}
	.vp-card {
		display: flex;
		width: min(860px, 100%);
		max-height: calc(100vh - 48px);
		background: var(--card);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		box-shadow: 0 24px 60px color-mix(in srgb, var(--ink) 14%, transparent);
		overflow: hidden;
	}
	.vp-list {
		width: 280px;
		flex-shrink: 0;
		border-right: 1px solid var(--line);
		background: var(--card-2);
		padding: 16px 10px;
		overflow-y: auto;
	}
	.vp-list h3 {
		margin: 0 8px 10px;
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--muted);
	}
	.vp-list ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.vp-list li {
		position: relative;
		display: flex;
	}
	.vp-vault {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
		text-align: left;
		background: none;
		border: 0;
		border-radius: 8px;
		padding: 8px 30px 8px 10px;
		color: var(--ink);
	}
	.vp-vault:not(:disabled):hover {
		background: var(--accent-soft);
	}
	li.current .vp-vault {
		background: var(--accent-soft);
		cursor: default;
	}
	li.gone .vp-vault {
		opacity: 0.55;
	}
	.vp-name {
		font-weight: 600;
		font-size: 13px;
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.vp-lock {
		display: inline-flex;
		color: var(--muted);
	}
	.vp-tag {
		font-size: 10px;
		font-weight: 600;
		color: var(--accent);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}
	.vp-path {
		font-size: 11px;
		color: var(--muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.vp-where {
		font-size: 11px;
		color: var(--faint);
	}
	.vp-forget {
		position: absolute;
		right: 4px;
		top: 6px;
		background: none;
		border: 0;
		border-radius: 6px;
		padding: 3px;
		color: var(--faint);
		display: inline-flex;
		opacity: 0;
	}
	li:hover .vp-forget,
	.vp-forget:focus-visible {
		opacity: 1;
	}
	.vp-forget:hover {
		color: var(--red);
		background: var(--red-soft);
	}
	.vp-main {
		position: relative;
		flex: 1;
		min-width: 0;
		padding: 28px 32px;
		display: flex;
		flex-direction: column;
		gap: 14px;
		overflow-y: auto;
	}
	.vp-close {
		position: absolute;
		top: 12px;
		right: 12px;
		color: var(--muted);
	}
	.vp-brand {
		margin-bottom: 4px;
	}
	.vp-main h2 {
		margin: 0;
		font-size: 20px;
	}
	.vp-lead {
		margin: 0;
		color: var(--muted);
		font-size: 13px;
		max-width: 480px;
	}
	.vp-actions {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--line);
	}
	.vp-action {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 14px 0;
		border-bottom: 1px solid var(--line);
	}
	.vp-action h4 {
		margin: 0 0 2px;
		font-size: 14px;
	}
	.vp-action p {
		margin: 0;
		font-size: 12px;
		color: var(--muted);
	}
	.vp-action .btn,
	.vp-form .btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		flex-shrink: 0;
	}
	.vp-back {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		align-self: flex-start;
	}
	.vp-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.vp-form input {
		flex: 1;
		min-width: 0;
	}
	.vp-target {
		font-size: 12px;
		color: var(--muted);
		overflow-wrap: anywhere;
	}
	.vp-cloud {
		display: flex;
		align-items: flex-start;
		gap: 8px;
		font-size: 12px;
		border-radius: 10px;
		padding: 8px 12px;
		background: var(--accent-soft);
		color: var(--muted);
	}
	.vp-cloud :global(svg) {
		flex-shrink: 0;
		margin-top: 1px;
	}
	@media (max-width: 720px) {
		.vp-card {
			flex-direction: column-reverse;
			overflow-y: auto;
		}
		.vp-list {
			width: auto;
			border-right: 0;
			border-top: 1px solid var(--line);
			overflow: visible;
		}
		.vp-main {
			padding: 22px 18px;
			overflow: visible;
		}
		.vp-forget {
			opacity: 1;
		}
	}
</style>
