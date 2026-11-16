import { error } from '@sveltejs/kit';
import { api } from '$lib/api';
import { previews } from '$lib/mock/flag.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => {
	try {
		const thread = await api.getThread(params.project, params.thread);
		// Preview: each post's kind and labels, from its frontmatter; a post nobody has said anything about has none.
		const postMeta = previews.enabled
			? thread.posts.filter((p) => p.kind).map((p) => ({ postId: p.id, kind: p.kind as NonNullable<typeof p.kind>, labels: p.labels }))
			: null;
		return { thread, postMeta };
	} catch (e) {
		error(404, e instanceof Error ? e.message : 'Thread not found');
	}
};
