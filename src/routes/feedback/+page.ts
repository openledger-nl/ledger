import { api } from '$lib/api';
import { mockApi } from '$lib/mock/api';
import type { PageLoad } from './$types';

// Preview: the inbox reads the mock feedback files until feedback is its own kind of file in the vault.
export const load: PageLoad = async () => {
	const [feedback, meta, notes] = await Promise.all([mockApi.listFeedback(), api.listProjectMeta(), mockApi.listDailyNotes()]);
	return { feedback, meta, notes };
};
