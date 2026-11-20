import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The days' notes in meetings/, the projects' facts and people, and the feedback (for the people it names).
export const load: PageLoad = async () => {
	const [notes, meta, people, feedback] = await Promise.all([api.listDailyNotes(), api.listProjectMeta(), api.listProjectPeople(), api.listAllFeedback()]);
	return { notes, meta, people, feedback };
};
