import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The vault's semesters (shared/semesters), which the dashboard's contribution graph and study projects follow.
export const load: PageLoad = async () => {
	const semesters = await api.listSemesters();
	return { semesters };
};
