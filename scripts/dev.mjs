#!/usr/bin/env node
// pnpm run dev:app: Ledger from this checkout, the way `pnpm run tauri dev` runs it, except that nothing that changes is
// put in front of you until you say so.
//
// It runs the front end's dev server (Vite, with this checkout's vite.config.js) and builds and starts the app, like
// `tauri dev`. It keeps watching the files too, but holds back what they change:
//
// * the front end: the window is not reloaded or patched (Vite's reload messages are kept from it, scripts/dev-hold.mjs),
//   so it keeps the page it has. A page it hasn't opened yet can come with the newer code, as files are read when they
//   are first asked for.
// * the Rust side (anything under src-tauri/): the new version is built in the background while the app keeps running
//   the build it started with. The app runs from a copy of the program, so the build can replace the file.
// * package.json or pnpm-lock.yaml: the packages are installed when you update.
//
// What it holds back it writes to target/dev-hold/state.json (scripts/dev-state.mjs), and the app is given that path in
// LEDGER_DEV_HOLD. The app reads it, lists the commits the local branch gained since the running build, and asks
// (src-tauri/src/devupdates.rs). Update reloads the window when only the front end changed. Otherwise the app exits with
// code 75 and this script starts the new build in its place (once a build still going is done). Closing Ledger ends this
// script, as it ends `tauri dev`; starting it again runs whatever the checkout has.

import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { hold } from './dev-hold.mjs';
import { createHold } from './dev-state.mjs';

const ROOT = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
// SvelteKit takes the project from the working directory, whatever Vite's root says.
process.chdir(ROOT);
const TAURI = path.join(ROOT, 'src-tauri');
const TARGET = path.resolve(process.env.CARGO_TARGET_DIR || path.join(TAURI, 'target'));
const EXE = process.platform === 'win32' ? '.exe' : '';
const BIN = 'ledger'; // [package] name in src-tauri/Cargo.toml
const DEBUG = path.join(TARGET, 'debug');
const BUILT = path.join(DEBUG, BIN + EXE);
const HOLD = path.join(TARGET, 'dev-hold');
const STATE = path.join(HOLD, 'state.json');
const RESTART_CODE = 75; // devupdates::RESTART_CODE

const say = (text) => console.log(`\x1b[36m[dev:app]\x1b[0m ${text}`);
const stripColor = (text) => text.replace(/\x1b\[[0-9;]*[A-Za-z]/g, '');
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function head() {
	try {
		return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
	} catch {
		return '';
	}
}

function writeState(state) {
	fs.mkdirSync(HOLD, { recursive: true });
	const temp = `${STATE}.tmp`;
	fs.writeFileSync(temp, JSON.stringify(state, null, '\t'));
	// Replaced in one step, so the app never reads half a file. A reader can hold it for a moment on Windows: try again.
	for (let attempt = 0; ; attempt++) {
		try {
			fs.renameSync(temp, STATE);
			return;
		} catch (e) {
			if (attempt >= 20) throw e;
			Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 25);
		}
	}
}

// ---- the Rust side: built in the background, run from a copy ---------------------------------------------

function cargo() {
	return new Promise((resolve) => {
		const child = spawn('cargo', ['build', '--manifest-path', path.join(TAURI, 'Cargo.toml'), '--color', 'always'], {
			cwd: TAURI,
			env: { ...process.env, CARGO_TARGET_DIR: TARGET },
			stdio: ['ignore', 'inherit', 'pipe']
		});
		let error = null;
		let rest = '';
		child.stderr.on('data', (chunk) => {
			process.stderr.write(chunk);
			rest += chunk.toString();
			let end;
			while ((end = rest.indexOf('\n')) >= 0) {
				const line = stripColor(rest.slice(0, end)).trim();
				rest = rest.slice(end + 1);
				if (!error && /^error(\[|:)/.test(line)) error = line;
			}
		});
		child.on('error', (e) => resolve({ ok: false, error: `cargo couldn't be started: ${e.message}` }));
		child.on('close', (code) => resolve({ ok: code === 0, error: error ?? `cargo build ended with ${code}` }));
	});
}

function stampOf(file) {
	const stat = fs.statSync(file);
	return `${stat.size}:${stat.mtimeMs}`;
}

const held = createHold({
	cargo,
	head,
	stamp: () => stampOf(BUILT),
	write: writeState,
	built(result, ready) {
		if (!result.ok) say(`The build failed: ${result.error}`);
		else say(ready ? 'The new build is ready: Update in Ledger to switch to it.' : 'Built; nothing new for the app.');
	}
});

/** Folders under src-tauri that a build writes to itself, or that `.taurignore` leaves out: no reason to build again. */
function rustIgnored(rel) {
	const parts = rel.split(/[\\/]/);
	const first = parts[0];
	const name = parts.at(-1) ?? '';
	if (first.startsWith('target') || first === 'gen' || first.startsWith('.')) return true;
	if (name.endsWith('~') || name.endsWith('.tmp') || name.endsWith('.swp')) return true;
	return taurignore.some((pattern) => first === pattern);
}

const taurignore = (() => {
	try {
		return fs
			.readFileSync(path.join(TAURI, '.taurignore'), 'utf8')
			.split(/\r?\n/)
			.map((l) => l.trim().replace(/\/$/, ''))
			.filter((l) => l && !l.startsWith('#'));
	} catch {
		return [];
	}
})();

// ---- the app --------------------------------------------------------------------------------------------

let app = null;
let launches = 0;

function launch() {
	const copy = path.join(DEBUG, `${BIN}-dev-${process.pid}-${++launches}${EXE}`);
	fs.copyFileSync(BUILT, copy);
	const good = held.launching();
	say(`Starting Ledger (${good.commit.slice(0, 7) || 'no commit'}).`);
	app = spawn(copy, [], { cwd: TAURI, stdio: 'inherit', env: { ...process.env, LEDGER_DEV_HOLD: STATE } });
	app.on('error', (e) => {
		say(`Ledger couldn't be started: ${e.message}`);
		void shutdown(1);
	});
	app.on('exit', (code) => void exited(copy, code));
}

async function exited(copy, code) {
	app = null;
	// The program's file is let go a moment after it exits.
	setTimeout(() => fs.rm(copy, { force: true }, () => {}), 2000);
	if (code !== RESTART_CODE) return shutdown(code ?? 0);

	say('Update: bringing in the new version…');
	if (held.state.dependencies) {
		say('package.json changed: installing the packages.');
		await run('pnpm', ['install', '--frozen-lockfile']);
		await vite.restart();
	}
	await held.settled();
	if (held.lastError) say(`The newest build failed (${held.lastError}); starting the last one that worked.`);
	launch();
}

function run(program, args) {
	return new Promise((resolve) => {
		// pnpm is a script on Windows, which only a shell starts; the words are fixed here, never from outside.
		const child = spawn(program, args, { cwd: ROOT, stdio: 'inherit', shell: process.platform === 'win32' });
		child.on('close', resolve);
		child.on('error', resolve);
	});
}

let closing = false;
async function shutdown(code) {
	if (closing) return;
	closing = true;
	held.stop();
	if (app) app.kill();
	try {
		await vite?.close();
	} catch {
		// It is going away either way.
	}
	process.exit(code);
}

// ---- start ---------------------------------------------------------------------------------------------------

process.on('SIGINT', () => void shutdown(130));
process.on('SIGTERM', () => void shutdown(143));

for (const name of fs.existsSync(DEBUG) ? fs.readdirSync(DEBUG) : []) {
	if (name.startsWith(`${BIN}-dev-`) && name.endsWith(EXE)) fs.rm(path.join(DEBUG, name), { force: true }, () => {});
}

if (!fs.existsSync(path.join(ROOT, 'node_modules'))) {
	say('Installing the packages first.');
	await run('pnpm', ['install', '--frozen-lockfile']);
}

const vite = await createServer({
	root: ROOT,
	configFile: path.join(ROOT, 'vite.config.js'),
	plugins: [hold({ onHeld: () => held.frontendHeld(), onPage: () => held.pageLoaded() })]
});
await vite.listen();
vite.printUrls();

fs.watch(TAURI, { recursive: true }, (_event, file) => {
	if (file && !rustIgnored(file.toString())) held.rustChanged();
});
fs.watch(ROOT, (_event, file) => {
	const name = file?.toString();
	if (name === 'package.json' || name === 'pnpm-lock.yaml') held.dependenciesChanged();
});

say('Building Ledger…');
await held.build();
while (!held.good) {
	say('The first build failed. Fix it, save, and it is built again.');
	while (!held.busy) await sleep(300);
	await held.settled();
}
launch();
