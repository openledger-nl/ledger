<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import TaskModal from '$lib/components/TaskModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Planning/TaskModal', component: TaskModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="New task" asChild>
	<TaskModal project={fx.PROJECT} task={null} board={fx.board} threads={fx.threads} onclose={() => {}} />
</Story>

<Story name="Editing" asChild>
	<TaskModal project={fx.PROJECT} task={fx.task} board={fx.board} threads={fx.threads} onclose={() => {}} />
</Story>
