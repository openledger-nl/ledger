import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The days' notes in meetings/, the projects' facts and people, the feedback (for the people it names), and what the
// notes' [[links]] can point at in every project.
export const load: PageLoad = async () => {
	const [notes, meta, people, feedback, links] = await Promise.all([
		api.listDailyNotes(),
		api.listProjectMeta(),
		api.listProjectPeople(),
		api.listAllFeedback(),
		api.listVaultLinks()
	]);
	return { notes, meta, people, feedback, links };
};
