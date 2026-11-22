<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import UploadModal from '$lib/components/UploadModal.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Uploads/UploadModal', component: UploadModal, parameters: { layout: 'fullscreen' } });
</script>

<Story name="Two files" asChild>
	<UploadModal project={fx.PROJECT} thread={fx.THREAD} paths={['/home/student/Downloads/datasheet.pdf', '/home/student/Pictures/greenhouse.jpg']} onclose={() => {}} />
</Story>
