// The vault picker as an overlay over the app: one place that opens it (the switch button at the bottom left of the
// sidebar and rail, Settings > Vault), drawn once by the root layout.

import { api } from './api';
import { attempt } from './ui.svelte';
import type { Vaults } from './types';

export const vaultPicker = $state<{ vaults: Vaults | null }>({ vaults: null });

/** Opens the picker on the vaults as they are now. */
export async function openVaultPicker() {
	const vaults = await attempt(() => api.listVaults());
	if (vaults) vaultPicker.vaults = vaults;
}

export function closeVaultPicker() {
	vaultPicker.vaults = null;
}
