import { api } from '$lib/api';
import type { PageLoad } from './$types';

export const load: PageLoad = async () => ({ semesters: await api.listSemesters() });
