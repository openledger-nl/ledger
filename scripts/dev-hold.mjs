// The Vite plugin `pnpm run dev:app` (scripts/dev.mjs) runs the dev server with: it keeps what file changes would do to
// the window (patch modules, reload, drop modules, show an error) from it, and hears when the window loads a page, which
// then has the front end as the files are now.
//
// A reload Vite sends for its own reasons is let through: at the start it reloads once it has prepared the packages a page
// uses, and holding that back would leave the first page half loaded. So only what follows a change to a watched file
// (within CHANGE_WINDOW_MS) is held.

import path from 'node:path';

const HELD_MESSAGES = new Set(['update', 'full-reload', 'prune', 'error']);
export const CHANGE_WINDOW_MS = 3000;

/**
 * @param {{ onHeld: () => void, onPage: () => void }} hooks `onHeld` when a message to the window is kept back, `onPage`
 *   when the window asks for a page.
 */
export function hold({ onHeld, onPage }) {
	let touched = 0;
	return {
		name: 'ledger-dev-hold',
		enforce: 'pre',
		hotUpdate: {
			order: 'pre',
			handler() {
				touched = Date.now();
			}
		},
		configureServer(server) {
			server.watcher.on('all', (_event, file) => {
				if (!String(file).includes(`${path.sep}node_modules${path.sep}.vite`)) touched = Date.now();
			});
			// The client environment's channel is this same socket server, so every update to the window goes through here.
			const send = server.ws.send.bind(server.ws);
			server.ws.send = (...args) => {
				const type = typeof args[0] === 'string' ? 'custom' : args[0]?.type;
				if (HELD_MESSAGES.has(type) && Date.now() - touched < CHANGE_WINDOW_MS) {
					onHeld();
					return;
				}
				return send(...args);
			};
			server.middlewares.use((req, _res, next) => {
				const page = req.method === 'GET' && (req.headers['sec-fetch-dest'] === 'document' || req.headers.accept?.startsWith('text/html'));
				if (page) onPage();
				next();
			});
		}
	};
}
