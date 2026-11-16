import { api } from '$lib/api';
import { mockApi } from '$lib/mock/api';
import type { PageLoad } from './$types';

// Preview: meeting notes (the days' notes in Daily/) come from the mock data until the vault has them; the feedback
// is read too, for the people it names.
export const load: PageLoad = async () => {
	const [notes, meta, feedback] = await Promise.all([mockApi.listDailyNotes(), api.listProjectMeta(), api.listAllFeedback()]);
	return { notes, meta, feedback };
};
