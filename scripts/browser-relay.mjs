// The Vite plugin that lets Ledger's front end run in an ordinary browser tab during development, at
// http://127.0.0.1:1420 next to the app's own window (`pnpm run dev`). A browser has no Rust backend, so every call the
// page makes to it goes to the app's window instead, which runs it and answers (src/lib/browserRelay.ts):
//
//   browser tab --ledger:invoke--> this plugin --ledger:invoke--> Ledger's window (the relay) --invoke--> Rust
//   browser tab <--ledger:result-- this plugin <--ledger:result-- Ledger's window
//
// Events the backend sends (vault-changed, sync-status, …) come back the same way. Everything goes over Vite's own HMR
// socket: no new port is opened, and Vite's checks on that socket (localhost, the page's origin, its token) still
// apply. Anyone with the dev page open can act on the vault, as in the window. `vite dev` only: never in a build.

const NO_RELAY = 'Open Ledger’s window (pnpm run dev) to use it from a browser: the browser reaches the backend through it.';
const RELAY_GONE = 'Ledger’s window went away before it answered. Start it again (pnpm run dev) and retry.';

/** @returns {import('vite').Plugin} */
export function browserRelay() {
	return {
		name: 'ledger-browser-relay',
		apply: 'serve',
		configureServer(server) {
			const ws = server.ws;
			/** The app's window, once it said it is the relay. @type {import('vite').WebSocketClient | null} */
			let relay = null;
			/** Calls sent to the relay and not answered yet: key → the tab that asked, and its own id for the call.
			 *  @type {Map<number, { client: import('vite').WebSocketClient; id: number }>} */
			const pending = new Map();
			/** The events some tab listens to: the relay listens to each of them once. @type {Set<string>} */
			const events = new Set();
			let next = 0;

			ws.on('ledger:relay', (_data, client) => {
				relay = client;
				if (events.size) client.send('ledger:listen', { events: [...events] });
			});

			ws.on('vite:client:disconnect', (_data, client) => {
				if (client !== relay) return;
				relay = null;
				for (const { client: tab, id } of pending.values()) tab.send('ledger:result', { id, ok: false, error: RELAY_GONE });
				pending.clear();
			});

			ws.on('ledger:invoke', (data, client) => {
				if (!relay) return client.send('ledger:result', { id: data.id, ok: false, error: NO_RELAY });
				const key = ++next;
				pending.set(key, { client, id: data.id });
				relay.send('ledger:invoke', { key, command: data.command, args: data.args, options: data.options });
			});

			ws.on('ledger:result', (data, client) => {
				if (client !== relay) return;
				const asked = pending.get(data.key);
				if (!asked) return;
				pending.delete(data.key);
				asked.client.send('ledger:result', { id: asked.id, ok: data.ok, value: data.value, error: data.error });
			});

			// The window's own events (a file dropped on it, its focus) stay with the window: a tab would act on them twice.
			ws.on('ledger:listen', (data) => {
				const fresh = (data?.events ?? []).filter((/** @type {unknown} */ e) => typeof e === 'string' && !e.startsWith('tauri://') && !events.has(e));
				for (const e of fresh) events.add(e);
				if (relay && fresh.length) relay.send('ledger:listen', { events: fresh });
			});

			ws.on('ledger:event', (data, client) => {
				if (client !== relay) return;
				for (const tab of ws.clients) if (tab !== relay) tab.send('ledger:event', data);
			});
		}
	};
}
