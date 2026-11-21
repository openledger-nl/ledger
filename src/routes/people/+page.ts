import { api } from '$lib/api';
import type { PageLoad } from './$types';

// Everyone the vault knows (shared/people), with the projects each is part of; `?person=<id>` opens one of them.
export const load: PageLoad = async ({ url }) => {
	const people = await api.listPeople();
	return { people, open: url.searchParams.get('person') };
};
