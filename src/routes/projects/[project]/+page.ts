import { api } from '$lib/api';
import type { PageLoad } from './$types';

// The latest deliverables on the Overview; the threads come with the projects.
export const load: PageLoad = async ({ params }) => ({ deliverables: await api.listDeliverables(params.project) });
