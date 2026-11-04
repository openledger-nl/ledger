// The update notice: what the backend found, whether "Later" hid it, and how far installing has got. The backend
// looks by itself, for releases in a release build and, in a development build, for what `pnpm run dev:app` holds back
// of the changes in its checkout, and says so with `update-available`; installing downloads a release once, waits until
// a restart cuts nothing off, and ends in the restart (see src-tauri/src/updates.rs and devupdates.rs).

import { listen } from '@tauri-apps/api/event';
import { updatesApi } from './api';
import type { Announced, RunsChoice, UpdateProgress } from './types';

/**
 * - `idle`: the notice asks: *Install and restart* and *Later*, or in a development build *Update* and *Not now*.
 * - `working`: downloading (with progress), checking the signature, installing, or bringing the new build in.
 * - `ask`: Claude Code is still running prompts; wait for them or stop them?
 * - `waiting`: ready; installs as soon as what is still going is over.
 * - `applied`: a development build is bringing in what was held back; it restarts, or the window reloads.
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
	/** What is being brought in, once a development build is updating. */
	applied: null as { commits: number; head: string; restart: boolean } | null,
	error: null as string | null
});

/** What tells one announcement from another: a release's version; for a development build, which change it is. */
export const keyOf = (update: Announced) => (update.kind === 'release' ? update.version : `${update.target}#${update.generation}`);

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

/**
 * Shows the notice for this update, unless installing is already under way. A development build hears of the same change
 * again as its build moves on (building, ready, failed): that refreshes the notice, and one that *Not now* hid stays hidden
 * until another change comes, unless `reveal` asks for it (Settings).
 */
export function showUpdate(info: Announced, reveal = false) {
	const same = updates.available !== null && keyOf(updates.available) === keyOf(info);
	if (updates.phase === 'working' || updates.phase === 'waiting' || updates.phase === 'ask' || updates.phase === 'applied') {
		if (same) {
			updates.available = info;
			updates.hidden = false;
		}
		return;
	}
	if (same && info.kind === 'branch' && updates.hidden && !reveal) {
		updates.available = info;
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
 * *Install and restart* (*Update*). Asks the backend again every second while the answer is to wait; a release is
 * downloaded once. A release that installs never comes back: Ledger restarts. A development build comes back as `applied`.
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
			updates.applied = { commits: step.commits, head: step.head, restart: step.restart };
			updates.phase = 'applied';
			// With a restart, Ledger is already exiting and `pnpm run dev:app` starts the new build. Without one, only the
			// front end changed: a reload is the restart, and the page it loads is the new one.
			if (!step.restart) setTimeout(() => window.location.reload(), 800);
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
