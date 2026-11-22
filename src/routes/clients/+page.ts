import { api } from '$lib/api';
import type { PageLoad } from './$types';

// Every client in the vault (shared/clients), with its projects; `?client=<id>` opens one of them.
export const load: PageLoad = async ({ url }) => {
	const clients = await api.listClients();
	return { clients, open: url.searchParams.get('client') };
};
