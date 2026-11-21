import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The conflict copies in the vault, with both versions when they are text.
export const load: PageLoad = async () => {
	const conflicts = await api.listConflicts();
	return { conflicts };
};
