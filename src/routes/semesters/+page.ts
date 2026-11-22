import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The vault's semesters (shared/semesters), which the dashboard's contribution graph and study projects follow, and
// the templates (shared/templates/semesters) a new one can start from.
export const load: PageLoad = async () => {
	const [semesters, templates] = await Promise.all([api.listSemesters(), api.listSemesterTemplates()]);
	return { semesters, templates };
};
