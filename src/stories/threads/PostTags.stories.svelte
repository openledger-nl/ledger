<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PostTags from '$lib/components/PostTags.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Threads/PostTags', component: PostTags });
</script>

<Story name="Research, to check" asChild>
	<PostTags project={fx.PROJECT} thread={fx.THREAD} postId="p-2" meta={fx.postMeta} />
</Story>

<Story name="No tags yet" asChild>
	<PostTags project={fx.PROJECT} thread={fx.THREAD} postId="p-3" meta={null} />
</Story>
