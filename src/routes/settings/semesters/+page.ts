import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

// Semesters are shared by the whole vault and have a page of their own now, under Shared in the explorer.
export const load: PageLoad = () => {
	redirect(307, '/semesters');
};
