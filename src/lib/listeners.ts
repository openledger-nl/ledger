// Listening to the backend's events for as long as a part of the window lives: `on` starts listening, `stop` stops
// every listener at once, also one whose listening only started after `stop` (listening starts asynchronously).

import { listen, type EventCallback } from '@tauri-apps/api/event';

export function listeners() {
	const cleanups: Array<() => void> = [];
	let alive = true;
	return {
		on<T>(event: string, handler: EventCallback<T>) {
			listen<T>(event, handler)
				.then((unlisten) => (alive ? cleanups.push(unlisten) : unlisten()))
				.catch(() => {});
		},
		stop() {
			alive = false;
			cleanups.forEach((fn) => fn());
		}
	};
}
