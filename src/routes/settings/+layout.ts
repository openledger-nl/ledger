import { api } from '$lib/api';
import type { LayoutLoad } from './$types';

// What the panels show beyond the settings: the semesters and the vault-wide templates have their own tabs.
export const load: LayoutLoad = async () => {
	const [settings, semesters, templates] = await Promise.all([api.getSettings(), api.listSemesters(), api.listTemplates('')]);
	return { settings, semesters, templates };
};
