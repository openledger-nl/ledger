<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PostCard from '$lib/components/PostCard.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Threads/PostCard', component: PostCard });
</script>

<Story name="Guiding question" asChild>
	<PostCard project={fx.PROJECT} thread={fx.THREAD} post={fx.posts[0]} byId={fx.postById} onreply={() => {}} onjump={() => {}} />
</Story>

<Story name="Research, to check" asChild>
	<PostCard project={fx.PROJECT} thread={fx.THREAD} post={fx.posts[1]} byId={fx.postById} tags={fx.postMeta} onreply={() => {}} onjump={() => {}} />
</Story>

<Story name="A reply" asChild>
	<PostCard project={fx.PROJECT} thread={fx.THREAD} post={fx.posts[2]} byId={fx.postById} variant="answer" onreply={() => {}} onjump={() => {}} />
</Story>

<Story name="Version published" asChild>
	<PostCard project={fx.PROJECT} thread={fx.THREAD} post={fx.posts[3]} byId={fx.postById} onreply={() => {}} onjump={() => {}} />
</Story>
