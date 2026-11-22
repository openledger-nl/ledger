import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The vault-wide templates (shared/templates/prompts), offered in every project of their kinds.
export const load: PageLoad = async () => {
	const templates = await api.listTemplates('');
	return { templates };
};
