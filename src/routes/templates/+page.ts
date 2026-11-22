import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The vault-wide templates: the ones deliverables start from (shared/templates/prompts), offered in every project of
// their kinds, and the ones semesters start from (shared/templates/semesters).
export const load: PageLoad = async () => {
	const [templates, semesterTemplates] = await Promise.all([api.listTemplates(''), api.listSemesterTemplates()]);
	return { templates, semesterTemplates };
};
