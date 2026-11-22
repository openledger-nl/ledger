import { api } from '$lib/api';
import type { LayoutLoad } from './$types';

// `links` is what a [[link]] in this project's markdown can point at; the editors and the rendered pages read it from
// the page's data, and it is loaded again with everything else whenever something changes.
export const load: LayoutLoad = async ({ params }) => {
	const [project, links, meta, people, semesters] = await Promise.all([
		api.getProject(params.project),
		api.listLinks(),
		// Its kind, goal, end and intended label, and its people, from its project.md; the semesters for its end.
		api.getProjectMeta(params.project),
		api.listProjectPeople(params.project),
		api.listSemesters()
	]);
	return { project, links, meta, people, semesters };
};
