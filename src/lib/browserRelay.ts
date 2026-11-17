// Ledger's front end in an ordinary browser tab, during development (scripts/browser-relay.mjs has the whole picture).
//
// - In Ledger's own window this is the relay: it runs the calls a browser tab sends it with the real `invoke`, and
//   passes the backend's events on.
// - In a browser tab it stands in for Tauri's internals (`window.__TAURI_INTERNALS__`), so that everything written
//   for the window (api.ts, `listen`, the opener and dialog plugins) works unchanged and goes to the relay.
//
// Bytes (a file read, an upload's body) travel as base64. What only the window itself can do stays there: a dialog
// opens over Ledger's window, a file is dropped on that window only, and the window's own events aren't passed on.

type ViteHotContext = NonNullable<ImportMeta['hot']>;

type Packed = { __bytes: string };
type Result = { id?: number; key?: number; ok: boolean; value?: unknown; error?: unknown };

function toBase64(bytes: Uint8Array): string {
	let text = '';
	for (let i = 0; i < bytes.length; i += 0x8000) text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
	return btoa(text);
}

function fromBase64(text: string): ArrayBuffer {
	const raw = atob(text);
	const bytes = new Uint8Array(raw.length);
	for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
	return bytes.buffer;
}

/** A command's arguments or answer, ready for JSON: bytes become `{ __bytes }`. */
function pack(value: unknown): unknown {
	if (value instanceof ArrayBuffer) return { __bytes: toBase64(new Uint8Array(value)) } satisfies Packed;
	if (ArrayBuffer.isView(value)) return { __bytes: toBase64(new Uint8Array(value.buffer, value.byteOffset, value.byteLength)) } satisfies Packed;
	return value;
}

function isPacked(value: unknown): value is Packed {
	return !!value && typeof value === 'object' && Object.keys(value).length === 1 && typeof (value as Packed).__bytes === 'string';
}

const unpack = (value: unknown) => (isPacked(value) ? fromBase64(value.__bytes) : value);

/** What a failed call says, as Tauri hands it over (Ledger's commands reject with a message). */
const failure = (e: unknown) => (e instanceof Error ? e.message : e);

/** Starts the relay in Ledger's window, or the stand-in in a browser tab. Development only (hooks.client.ts). */
export function startBrowserRelay(hot: ViteHotContext) {
	const w = window as unknown as Record<string, unknown>;
	if ('__TAURI_INTERNALS__' in w) relay(hot, w);
	else standIn(hot, w);
}

// ---- in Ledger's window ------------------------------------------------------------------------------

function relay(hot: ViteHotContext, w: Record<string, unknown>) {
	// Only the main window: the screenshot overlay is a window of its own, and isn't the app.
	const label = (w.__TAURI_INTERNALS__ as { metadata?: { currentWindow?: { label?: string } } }).metadata?.currentWindow?.label;
	if (label && label !== 'main') return;
	const hello = () => hot.send('ledger:relay', {});
	hello();
	hot.on('vite:ws:connect', hello);

	hot.on('ledger:invoke', async (call: { key: number; command: string; args?: unknown; options?: unknown }) => {
		const { invoke } = await import('@tauri-apps/api/core');
		try {
			const value = await invoke(call.command, unpack(call.args) as never, call.options as never);
			hot.send('ledger:result', { key: call.key, ok: true, value: pack(value) });
		} catch (e) {
			hot.send('ledger:result', { key: call.key, ok: false, error: failure(e) });
		}
	});

	const listening = new Set<string>();
	hot.on('ledger:listen', async ({ events }: { events: string[] }) => {
		const { listen } = await import('@tauri-apps/api/event');
		for (const event of events) {
			if (listening.has(event)) continue;
			listening.add(event);
			listen(event, (e) => hot.send('ledger:event', { event, payload: e.payload })).catch(() => listening.delete(event));
		}
	});
}

// ---- in a browser tab --------------------------------------------------------------------------------

function standIn(hot: ViteHotContext, w: Record<string, unknown>) {
	const waiting = new Map<number, { resolve: (v: unknown) => void; reject: (e: unknown) => void }>();
	const callbacks = new Map<number, (payload: unknown) => void>();
	/** Per event: the listener ids `listen` was given, and the callback each one calls. */
	const listeners = new Map<string, Map<number, number>>();
	let next = 1;

	hot.on('ledger:result', (result: Result) => {
		const call = result.id === undefined ? undefined : waiting.get(result.id);
		if (!call || result.id === undefined) return;
		waiting.delete(result.id);
		if (result.ok) call.resolve(unpack(result.value));
		else call.reject(result.error);
	});

	hot.on('ledger:event', ({ event, payload }: { event: string; payload: unknown }) => {
		for (const [id, callback] of listeners.get(event) ?? []) callbacks.get(callback)?.({ event, id, payload });
	});

	// A reconnected socket (Vite restarted) tells the relay again which events this tab listens to.
	hot.on('vite:ws:connect', () => {
		if (listeners.size) hot.send('ledger:listen', { events: [...listeners.keys()] });
	});

	function send(command: string, args?: unknown, options?: unknown): Promise<unknown> {
		const id = next++;
		return new Promise((resolve, reject) => {
			waiting.set(id, { resolve, reject });
			hot.send('ledger:invoke', { id, command, args: pack(args), options });
		});
	}

	w.__TAURI_INTERNALS__ = {
		// The page believes it is in the main window, so the code that asks which window it is in carries on.
		metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
		transformCallback(callback?: (payload: unknown) => void, once = false) {
			const id = next++;
			callbacks.set(id, (payload) => {
				if (once) callbacks.delete(id);
				callback?.(payload);
			});
			return id;
		},
		unregisterCallback(id: number) {
			callbacks.delete(id);
		},
		convertFileSrc(path: string, protocol = 'asset') {
			return `${protocol}://localhost/${encodeURIComponent(path)}`;
		},
		async invoke(command: string, args?: Record<string, unknown>, options?: unknown) {
			if (command === 'plugin:event|listen') {
				const event = String(args?.event);
				const id = next++;
				if (!listeners.has(event)) listeners.set(event, new Map());
				listeners.get(event)!.set(id, Number(args?.handler));
				hot.send('ledger:listen', { events: [event] });
				return id;
			}
			if (command === 'plugin:event|unlisten') {
				listeners.get(String(args?.event))?.delete(Number(args?.eventId));
				return null;
			}
			return send(command, args, options);
		}
	};
	w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
		unregisterListener(event: string, id: number) {
			listeners.get(event)?.delete(id);
		}
	};
}
