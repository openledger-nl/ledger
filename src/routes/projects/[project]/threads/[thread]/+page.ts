import { error } from '@sveltejs/kit';
import { api } from '$lib/api';
import { mockApi } from '$lib/mock/api';
import { previews } from '$lib/mock/flag.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => {
	try {
		const thread = await api.getThread(params.project, params.thread);
		// Preview: the posts' kinds and labels come from the mock data for now.
		const postMeta = previews.enabled ? await mockApi.listPostMeta(thread.posts.map((p) => p.id)) : null;
		return { thread, postMeta };
	} catch (e) {
		error(404, e instanceof Error ? e.message : 'Thread not found');
	}
};
