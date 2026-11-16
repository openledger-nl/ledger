import { error } from '@sveltejs/kit';
import { api } from '$lib/api';
import { mockApi } from '$lib/mock/api';
import { previews } from '$lib/mock/flag.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params, url }) => {
	const path = url.searchParams.get('path') ?? '';
	if (!path) error(404, 'No deliverable was asked for');
	try {
		const [deliverable, provenance, prompts] = await Promise.all([
			api.getDeliverable(params.project, path),
			api.getProvenance(params.project, path),
			api.listDeliverablePrompts(params.project, path)
		]);
		// Preview: the deliverable's goal, number and own label come from the mock data for now.
		const deliverableMeta = previews.enabled ? await mockApi.getDeliverableMeta(params.project, deliverable.basePath || path) : null;
		return { deliverable, provenance, prompts, deliverableMeta };
	} catch (e) {
		error(404, e instanceof Error ? e.message : 'Deliverable not found');
	}
};
