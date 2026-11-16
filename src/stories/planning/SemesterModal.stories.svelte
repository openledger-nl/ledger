<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import SemesterModal from '$lib/components/SemesterModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Planning/SemesterModal', component: SemesterModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="New semester" asChild>
	<SemesterModal semester={null} existing={[fx.semester]} onclose={() => {}} />
</Story>

<Story name="Editing" asChild>
	<SemesterModal semester={fx.semester} existing={[fx.semester]} onclose={() => {}} />
</Story>
