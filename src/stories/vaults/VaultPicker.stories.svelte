<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import VaultPicker from '$lib/components/VaultPicker.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Vaults/VaultPicker', component: VaultPicker });
</script>

<Story name="First start" args={{ vaults: fx.firstStart }} />
<Story name="Choosing among vaults" args={{ vaults: { ...fx.vaults, open: false, vaults: fx.vaults.vaults.map((v) => ({ ...v, open: false })) } }} />
<Story name="A vault to upgrade, at the start" args={{ vaults: fx.upgradeWaiting }} />
<Story name="Switching from Settings" args={{ vaults: fx.vaults, modal: true, onclose: () => {} }} />
<Story name="Fixed by LEDGER_VAULT" args={{ vaults: { ...fx.vaults, locked: true }, modal: true, onclose: () => {} }} />
