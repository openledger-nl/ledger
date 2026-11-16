import { error } from '@sveltejs/kit';
import { api } from '$lib/api';
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
		// The deliverable's goal, number and own label, from its project's project.md.
		const deliverableMeta = await api.getDeliverableMeta(params.project, deliverable.basePath || path);
		return { deliverable, provenance, prompts, deliverableMeta };
	} catch (e) {
		error(404, e instanceof Error ? e.message : 'Deliverable not found');
	}
};
