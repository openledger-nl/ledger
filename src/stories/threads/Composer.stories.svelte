<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Composer from '$lib/components/Composer.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Threads/Composer', component: Composer });
</script>

<Story name="New post" asChild>
	<Composer project={fx.PROJECT} thread={fx.THREAD} replyTo={null} oncancelreply={() => {}} onsent={() => {}} onprompt={() => {}} onfeedback={() => {}} onupload={() => {}} />
</Story>

<Story name="Replying" asChild>
	<Composer project={fx.PROJECT} thread={fx.THREAD} replyTo={fx.posts[1]} oncancelreply={() => {}} onsent={() => {}} onprompt={() => {}} onfeedback={() => {}} onupload={() => {}} />
</Story>
