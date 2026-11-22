<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import UploadCard from '$lib/components/UploadCard.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Uploads/UploadCard', component: UploadCard });
</script>

<Story name="A picture" asChild>
	<UploadCard project={fx.PROJECT} thread={fx.THREAD} upload={fx.upload} />
</Story>

<Story name="A file" asChild>
	<UploadCard project={fx.PROJECT} thread={fx.THREAD} upload={fx.uploadFile} />
</Story>

<Story name="File gone" asChild>
	<UploadCard project={fx.PROJECT} thread={fx.THREAD} upload={{ ...fx.uploadFile, exists: false }} />
</Story>
