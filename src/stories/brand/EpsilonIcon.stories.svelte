<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import EpsilonIcon from '$lib/components/EpsilonIcon.svelte';

	const { Story } = defineMeta({ title: 'Brand/EpsilonIcon', component: EpsilonIcon, args: { size: 48, strokeWidth: 2 } });
</script>

<Story name="Default" />
<Story name="Accent" args={{ color: 'var(--accent)', size: 64 }} />
