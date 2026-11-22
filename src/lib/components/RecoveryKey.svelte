<script lang="ts">
	// An encrypted vault's recovery key, shown once it is asked for (or once, when the vault is made): the key itself,
	// as text, to keep somewhere safe apart from the computer. It opens the vault when the passphrase is lost.
	import Copy from '@lucide/svelte/icons/copy';
	import KeyRound from '@lucide/svelte/icons/key-round';
	import { toast } from '$lib/ui.svelte';

	let { value, ondone, done = 'I have kept it somewhere safe' }: { value: string; ondone?: () => void; done?: string } = $props();

	async function copy() {
		try {
			await navigator.clipboard.writeText(value);
			toast('Recovery key copied');
		} catch {
			toast('Select the key and copy it by hand', true);
		}
	}
</script>

<div class="rk">
	<div class="rk-head"><KeyRound size={16} /><strong>The recovery key</strong></div>
	<p class="hint">
		It opens this vault when the passphrase is lost, on any computer. Keep it apart from this computer (a password manager, or on paper): without the
		passphrase or this key, nobody can open the vault, Ledger included.
	</p>
	<div class="rk-key">
		<code class="mono" aria-label="Recovery key">{value}</code>
		<button class="btn small" onclick={copy}><Copy size={14} />Copy</button>
	</div>
	{#if ondone}
		<div class="row-end"><button class="btn primary" onclick={ondone}>{done}</button></div>
	{/if}
</div>

<style>
	.rk {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 14px;
		border: 1px solid var(--accent);
		border-radius: var(--radius);
		background: var(--accent-soft);
	}
	.rk-head {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.rk-key {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.rk-key code {
		flex: 1;
		min-width: 0;
		word-break: break-all;
		padding: 8px 10px;
		background: var(--card);
		border: 1px solid var(--line);
		border-radius: 8px;
		user-select: all;
	}
</style>
