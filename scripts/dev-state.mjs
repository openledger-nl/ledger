// What `pnpm run dev:app` (scripts/dev.mjs) holds back, and the state file the app reads that from
// (src-tauri/src/devupdates.rs). Kept apart from the processes it runs, so how it moves along can be tested by itself.
//
// The file is written once things have settled (SETTLE_MS after the last change), never halfway: a merge or a checkout
// changes front-end and Rust files a moment apart, and an announcement made in between would say only the front end
// changed and offer a reload, when a rebuild is on its way.

export const SETTLE_MS = 800;
/** How long the Rust side waits for more changes before it builds. */
export const DEBOUNCE_MS = 400;

/**
 * @param {object} io
 * @param {() => Promise<{ ok: boolean, error?: string | null }>} io.cargo builds the Rust side
 * @param {() => string} io.head the commit the checkout is at
 * @param {() => string} io.stamp what tells one built program from another (its size and time)
 * @param {(state: object) => void} io.write writes the state file
 * @param {(result: { ok: boolean, error?: string | null }, ready: boolean) => void} [io.built] told after each build
 */
export function createHold({ cargo, head, stamp, write, built = () => {}, settleMs = SETTLE_MS, debounceMs = DEBOUNCE_MS }) {
	const state = {
		/** The commit the checkout was at when what is running was built. */
		running: '',
		/** Goes up when something starts being held back, or comes with new commits: the app announces each once. */
		generation: 0,
		frontend: false,
		build: { state: 'none', error: null },
		dependencies: false
	};
	let announcedHead = '';
	let building = null; // the build that is running, as a promise
	let queued = false; // a change came in that no build has started on yet
	let timer = null;
	let lastError = null; // the newest build failed with this
	let good = null; // the newest build that worked: { commit, stamp }
	let runningStamp = null; // the program the app runs, by the same stamp
	let writeTimer = null;

	const live = () => runningStamp !== null;

	function buildState() {
		if (building || queued) return { state: 'building', error: null };
		if (lastError) return { state: 'failed', error: lastError };
		if (good && live() && good.stamp !== runningStamp) return { state: 'ready', error: null };
		return { state: 'none', error: null };
	}

	const waiting = () => state.frontend || state.dependencies || state.build.state !== 'none';

	function flush() {
		clearTimeout(writeTimer);
		writeTimer = null;
		state.build = buildState();
		write(state);
	}

	function publish(now = false) {
		state.build = buildState();
		if (now) return flush();
		clearTimeout(writeTimer);
		writeTimer = setTimeout(flush, settleMs);
	}

	/** Something more is held back: a new announcement when nothing was before, or when it came with new commits. */
	function held(change) {
		const before = waiting();
		change();
		state.build = buildState();
		const now = head();
		if (waiting() && (!before || now !== announcedHead)) {
			state.generation++;
			announcedHead = now;
		}
		publish();
	}

	/** Builds what the checkout has now, and again afterwards as long as changes keep coming in meanwhile. */
	function build() {
		if (building) return building;
		queued = false;
		const commit = head();
		building = cargo().then((result) => {
			building = null;
			if (result.ok) {
				lastError = null;
				good = { commit, stamp: stamp() };
			} else {
				lastError = result.error ?? 'the build failed';
			}
			if (queued) return build();
			if (live()) {
				held(() => {});
				built(result, buildState().state === 'ready');
			}
			return result;
		});
		// Only now that it is under way: said before, it would read as nothing being built.
		if (live()) held(() => {});
		return building;
	}

	return {
		state,
		buildState,
		get good() {
			return good;
		},
		get lastError() {
			return lastError;
		},
		get busy() {
			return building !== null || timer !== null;
		},
		build,

		/** A file under src-tauri changed: build once the changes stop coming for a moment. */
		rustChanged() {
			queued = true;
			clearTimeout(timer);
			timer = setTimeout(() => {
				timer = null;
				void build();
			}, debounceMs);
			if (live()) held(() => {});
		},

		/** A reload or a patch was kept from the window. */
		frontendHeld() {
			if (live() && !state.frontend) held(() => (state.frontend = true));
		},

		/** The window loaded a page: it has the front end as the files are now. */
		pageLoaded() {
			if (!live() || !state.frontend) return;
			state.frontend = false;
			state.build = buildState();
			if (state.build.state === 'none' && !state.dependencies) state.running = head();
			publish();
		},

		dependenciesChanged() {
			if (live() && !state.dependencies) held(() => (state.dependencies = true));
		},

		/** Until no build is running or about to start. */
		async settled() {
			while (building || timer) await (building ?? new Promise((resolve) => setTimeout(resolve, debounceMs + 50)));
		},

		/**
		 * The app is being started with the newest build that worked. It has what that build was made from, and loads a
		 * fresh page, so the front end is the files' too. Written at once: the app reads it as it starts.
		 */
		launching() {
			runningStamp = good.stamp;
			state.running = good.commit;
			state.frontend = false;
			state.dependencies = false;
			publish(true);
			return good;
		},

		stop() {
			clearTimeout(timer);
			clearTimeout(writeTimer);
		}
	};
}
