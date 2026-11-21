<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import RecoveryKey from '$lib/components/RecoveryKey.svelte';

	// An encrypted vault's recovery key, as it is shown once when the vault is made, or when asked for in Settings.
	const { Story } = defineMeta({ title: 'Vaults/RecoveryKey', component: RecoveryKey });
	const value = 'ledger-key-1:q3Xb9vT0cWm2LkP7sY4nR8aE1uJ6hD5gF0zQwXcVbNmA9kL2';
</script>

<Story name="Just made" args={{ value, ondone: () => {}, done: 'I have kept it: open the vault' }} />

<Story name="Asked for" args={{ value, ondone: () => {}, done: 'Hide it' }} />
