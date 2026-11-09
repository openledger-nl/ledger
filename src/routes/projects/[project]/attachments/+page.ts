import { api } from '$lib/api';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => ({
	uploads: await api.listProjectUploads(params.project)
});
