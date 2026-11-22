import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The days' notes in meetings/, the projects' facts and people, the vault's people (who can be at a meeting), the
// feedback (for the projects it names), and what the notes' [[links]] can point at.
export const load: PageLoad = async () => {
	const [notes, meta, people, everyone, feedback, links] = await Promise.all([
		api.listDailyNotes(),
		api.listProjectMeta(),
		api.listProjectPeople(),
		api.listPeople(),
		api.listAllFeedback(),
		api.listLinks()
	]);
	return { notes, meta, people, everyone, feedback, links };
};
