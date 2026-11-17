<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import MmmBadge from '$lib/components/MmmBadge.svelte';

	const { Story } = defineMeta({ title: 'Brand/MmmBadge', component: MmmBadge, args: { level: 'cyborg', tagline: false }, argTypes: { level: { control: 'select', options: ['lazy-prompter', 'piggybacker', 'cyborg', 'handyman', 'man-made'] } } });
</script>

<Story name="Cyborg" />
<Story name="With tagline" args={{ tagline: true }} />
<Story name="All levels" asChild>
	<div style="display:flex;gap:8px;flex-wrap:wrap">
		{#each ['lazy-prompter', 'piggybacker', 'cyborg', 'handyman', 'man-made'] as level (level)}
			<MmmBadge {level} />
		{/each}
	</div>
</Story>
