import { api } from '$lib/api';
import { mockApi } from '$lib/mock/api';
import { previews } from '$lib/mock/flag.svelte';
import type { LayoutLoad } from './$types';

// `links` is what a [[link]] in this project's markdown can point at; the editors and the rendered pages read it from
// the page's data, and it is loaded again with everything else whenever something changes.
export const load: LayoutLoad = async ({ params }) => {
	const [project, links, meta, semesters] = await Promise.all([
		api.getProject(params.project),
		api.listLinks(params.project),
		// Preview: the v1 facts (kind, goal, end, intended label) come from the mock data for now.
		previews.enabled ? mockApi.getProjectMeta(params.project) : Promise.resolve(null),
		previews.enabled ? api.listSemesters() : Promise.resolve([])
	]);
	return { project, links, meta, semesters };
};
