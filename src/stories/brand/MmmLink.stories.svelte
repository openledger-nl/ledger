<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import MmmLink from '$lib/components/MmmLink.svelte';

	const { Story } = defineMeta({ title: 'Brand/MmmLink', component: MmmLink });
</script>

<Story name="Default" />
<Story name="Other text" args={{ text: 'What do these labels mean?' }} />
