<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Switch from '$lib/components/Switch.svelte';

	const { Story } = defineMeta({
		title: 'UI/Switch',
		component: Switch,
		args: { checked: true, label: 'Encrypt what goes to GitHub', disabled: false, onchange: () => {} }
	});
</script>

<Story name="On" />
<Story name="Off" args={{ checked: false }} />
<Story name="Disabled" args={{ disabled: true }} />
