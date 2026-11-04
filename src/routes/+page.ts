import { api } from '$lib/api';
import type { DeliverableSummary } from '$lib/types';
import type { PageLoad } from './$types';

/** What the dashboard shows of a project beyond its summary: its deliverables and how many tasks are still open. */
export interface ProjectDetail {
	deliverables: DeliverableSummary[];
	openTasks: number;
}

export const load: PageLoad = async ({ parent }) => {
	const { projects, onboarding } = await parent();
	const [activity, semesters, settings, details] = await Promise.all([
		api.listActivity(),
		api.listSemesters(),
		// The guide's sync step is the settings' own form.
		onboarding.active ? api.getSettings() : Promise.resolve(null),
		Promise.all(
			projects.map(async (p) => {
				const [deliverables, view] = await Promise.all([api.listDeliverables(p.slug), api.getProject(p.slug)]);
				return [p.slug, { deliverables, openTasks: view.openTaskCount }] as const;
			})
		)
	]);
	return { activity, semesters, settings, details: Object.fromEntries(details) as Record<string, ProjectDetail> };
};
