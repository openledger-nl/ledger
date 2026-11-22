import { api } from '$lib/api';
import type { LayoutLoad } from './$types';

// What every panel is given: the settings.
export const load: LayoutLoad = async () => {
	const settings = await api.getSettings();
	return { settings };
};
