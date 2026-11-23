import { api } from '$lib/api';
import type { PageLoad } from './$types';

/** What the page reloads on, when Ledger gets the focus back: a file may have been put in the folder meanwhile. */
export const _ATTACHMENTS = 'ledger:attachments';

export const load: PageLoad = async ({ params, url, depends }) => {
	depends(_ATTACHMENTS);
	return {
		uploads: await api.listProjectUploads(params.project),
		/** A file to point out (`?file=attachments/brief.pdf`): where a link to it leads. */
		file: url.searchParams.get('file'),
		/** A folder to open (`?dir=hans`), inside attachments/. */
		dir: url.searchParams.get('dir')
	};
};
