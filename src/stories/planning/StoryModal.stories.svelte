<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import StoryModal from '$lib/components/StoryModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Planning/StoryModal', component: StoryModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="New user story" asChild>
	<StoryModal project={fx.PROJECT} story={null} threads={fx.threads} initialThreadSlugs={[fx.THREAD]} onclose={() => {}} />
</Story>

<Story name="Editing" asChild>
	<StoryModal project={fx.PROJECT} story={fx.story} threads={fx.threads} onclose={() => {}} />
</Story>
