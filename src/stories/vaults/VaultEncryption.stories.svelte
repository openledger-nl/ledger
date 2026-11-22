<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import VaultEncryption from '$lib/components/settings/VaultEncryption.svelte';
	import * as fx from '../fixtures';

	// Settings > Encryption, for the sample vault: plain files, with the encrypted copy to make.
	const { Story } = defineMeta({ title: 'Vaults/VaultEncryption', component: VaultEncryption });
</script>

<Story name="A vault of plain files" args={{ settings: fx.settings }} />
