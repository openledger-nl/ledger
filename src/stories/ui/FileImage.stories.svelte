<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import FileImage from '$lib/components/FileImage.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'UI/FileImage', component: FileImage });
</script>

<!-- A picture from a project's folder, read through the backend (here a sample sketch). -->
<Story name="Thumbnail" asChild>
	<FileImage project={fx.PROJECT} path={fx.upload.file} alt="Greenhouse sketch" />
</Story>

<Story name="Large preview" asChild>
	<FileImage project={fx.PROJECT} path={fx.upload.file} alt="Greenhouse sketch" cls="shot-preview" />
</Story>
