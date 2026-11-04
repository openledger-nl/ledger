// Whether Portflow is switched on, and how it stands. Every Portflow screen and control asks `portflow.enabled`
// first and shows nothing while it is off: the screens then look exactly as they do without the feature.

import { listen } from '@tauri-apps/api/event';
import { portflowApi } from './api';
import { attempt, toast } from './ui.svelte';
import type { PortflowStatus } from './types';

export const portflow = $state({
	/** Off until the backend says otherwise. */
	enabled: false,
	status: null as PortflowStatus | null
});

/** Asks the backend again (after the switch changed, or the vault was reloaded). */
export async function loadPortflow() {
	try {
		const view = await portflowApi.get();
		portflow.enabled = view.enabled;
		portflow.status = view.status;
	} catch {
		portflow.enabled = false;
	}
}

export function startPortflow(): () => void {
	let alive = true;
	let unlisten: (() => void) | undefined;
	void loadPortflow();
	listen<PortflowStatus>('portflow-status', (event) => {
		portflow.status = event.payload;
		portflow.enabled = event.payload.state !== 'off';
	})
		.then((fn) => (alive ? (unlisten = fn) : fn()))
		.catch(() => {});
	return () => {
		alive = false;
		unlisten?.();
	};
}

/** Opens Portflow in the default browser and says where to go: Portflow only runs inside Canvas, so it opens on its home
 *  page, not at the item. `item` names it ("Project plan, version 3"). */
export async function openInPortflow(item: string) {
	if ((await attempt(() => portflowApi.open())) === undefined) return;
	toast(`Portflow opens in your browser, through Canvas. Go to ${item} there.`);
}
