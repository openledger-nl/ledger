import { api } from '$lib/api';
import { mockApi } from '$lib/mock/api';
import type { PageLoad } from './$types';

// Every piece of feedback in the vault, the projects' facts and people, and the meeting notes feedback can come from.
export const load: PageLoad = async () => {
	const [feedback, meta, people, notes] = await Promise.all([api.listAllFeedback(), api.listProjectMeta(), api.listProjectPeople(), mockApi.listDailyNotes()]);
	return { feedback, meta, people, notes };
};
