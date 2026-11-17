<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import EpsilonMatrix from '$lib/components/EpsilonMatrix.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Portflow/EpsilonMatrix', component: EpsilonMatrix });
</script>

<Story name="S3" asChild>
	<EpsilonMatrix view={fx.epsilon} />
</Story>
