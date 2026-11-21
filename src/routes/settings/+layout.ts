import { api } from '$lib/api';
import type { LayoutLoad } from './$types';

// What the panels show beyond the settings: the vault-wide templates have their own tab.
export const load: LayoutLoad = async () => {
	const [settings, templates] = await Promise.all([api.getSettings(), api.listTemplates('')]);
	return { settings, templates };
};
