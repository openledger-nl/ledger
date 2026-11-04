// The update notice: what the backend found, whether "Later" hid it, and how far installing has got. The backend
// looks by itself, for releases in a release build and for new commits on its branch in a development build, and says
// so with `update-available`; installing downloads a release once, waits until a restart cuts nothing off, and ends in
// the restart (see src-tauri/src/updates.rs and devupdates.rs).

import { listen } from '@tauri-apps/api/event';
import { updatesApi } from './api';
import type { Announced, RunsChoice, UpdateProgress } from './types';

/**
 * - `idle`: the notice asks: *Install and restart* and *Later*, or for new commits *Yes* and *Not now*.
 * - `working`: downloading (with progress), checking the signature, installing, or pulling the commits.
 * - `ask`: Claude Code is still running prompts; wait for them or stop them?
 * - `waiting`: ready; installs as soon as what is still going is over.
 * - `applied`: a development build brought its new commits in; it rebuilds, or the window reloads.
 * - `failed`: said why, and nothing was installed.
 */
export type UpdatePhase = 'idle' | 'working' | 'ask' | 'waiting' | 'applied' | 'failed';

export const updates = $state({
	available: null as Announced | null,
	/** *Later* was pressed: hidden until the next check finds the update again. */
	hidden: false,
	phase: 'idle' as UpdatePhase,
	waitingFor: [] as string[],
	runs: [] as string[],
	progress: null as UpdateProgress | null,
	/** What was brought in, once a development build has. */
	applied: null as { commits: number; head: string; rebuild: boolean } | null,
	error: null as string | null
});

/** What tells one announcement from another: a release's version, the commit a branch update goes to. */
export const keyOf = (update: Announced) => (update.kind === 'release' ? update.version : update.target);

// ---- before the restart, whatever is typed and not yet saved is saved ---------------------------------

const flushers = new Set<() => unknown>();

/** Something to save before Ledger restarts for an update, such as an editor's pending autosave. Returns the undo. */
export function beforeRestart(flush: () => unknown): () => void {
	flushers.add(flush);
	return () => {
		flushers.delete(flush);
	};
}

async function flushAll() {
	for (const flush of [...flushers]) {
		try {
			await flush();
		} catch {
			// What failed to save says so itself; the backend still checks everything else.
		}
	}
}

// ---- found, hidden, shown -----------------------------------------------------------------------------

/** Shows the notice for this update, unless installing is already under way. */
export function showUpdate(info: Announced) {
	if (updates.phase === 'working' || updates.phase === 'waiting' || updates.phase === 'ask' || updates.phase === 'applied') {
		if (updates.available && keyOf(updates.available) === keyOf(info)) updates.hidden = false;
		return;
	}
	updates.available = info;
	updates.hidden = false;
	updates.phase = 'idle';
	updates.error = null;
}

/** *Later* (*Not now*): hides the notice until the next check, and stops waiting to install. */
export function later() {
	// What was applied has been dealt with: closing the notice ends it, rather than offering it again.
	if (updates.phase === 'applied') {
		updates.available = null;
		updates.applied = null;
	}
	stopWaiting();
	updates.hidden = true;
}

/** Listens for what the backend finds, and asks what it already found before the window was listening. */
export function startUpdates(): () => void {
	const cleanups: Array<() => void> = [];
	let alive = true;
	const keep = (unlisten: () => void) => (alive ? cleanups.push(unlisten) : unlisten());

	listen<Announced>('update-available', (event) => showUpdate(event.payload))
		.then(keep)
		.catch(() => {});
	listen<UpdateProgress>('update-progress', (event) => {
		updates.progress = event.payload;
	})
		.then(keep)
		.catch(() => {});
	updatesApi
		.status()
		.then((view) => {
			if (view.available && !updates.available) showUpdate(view.available);
		})
		.catch(() => {});

	return () => {
		alive = false;
		cleanups.forEach((fn) => fn());
	};
}

// ---- installing ---------------------------------------------------------------------------------------

/** Bumped to stop a wait in progress: a loop whose number is no longer current stops. */
let attempt = 0;

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * *Install and restart* (*Yes*). Asks the backend again every second while the answer is to wait; a release is downloaded
 * once. A release that installs never comes back: Ledger restarts. New commits come back as `applied`.
 */
export async function installUpdate(choice: RunsChoice = 'ask') {
	const mine = ++attempt;
	updates.error = null;
	updates.phase = 'working';
	updates.progress = null;
	while (mine === attempt) {
		await flushAll();
		let step;
		try {
			step = await updatesApi.install(choice);
		} catch (e) {
			if (mine !== attempt) return;
			updates.phase = 'failed';
			updates.error = e instanceof Error ? e.message : String(e);
			return;
		}
		if (mine !== attempt) return;
		if (step.state === 'ask') {
			updates.phase = 'ask';
			updates.runs = step.runs;
			return;
		}
		if (step.state === 'applied') {
			updates.applied = { commits: step.commits, head: step.head, rebuild: step.rebuild };
			updates.phase = 'applied';
			// With a rebuild coming, the toolchain restarts the app by itself. Without one, only the front end changed: a
			// reload is the restart, and the page it loads is the new one.
			if (!step.rebuild) setTimeout(() => window.location.reload(), 800);
			return;
		}
		updates.phase = 'waiting';
		updates.waitingFor = step.waitingFor;
		await pause(1000);
	}
}

/** Stops waiting to install; the downloaded update is kept for the next try. */
export function stopWaiting() {
	attempt++;
	if (updates.phase !== 'idle') {
		updates.phase = 'idle';
		updates.progress = null;
		updates.error = null;
	}
}
