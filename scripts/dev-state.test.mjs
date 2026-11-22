// How what `pnpm run dev:app` holds back moves along (scripts/dev-state.mjs), with a build that finishes when the test
// says so. Runs with `pnpm run check` (node --test).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHold } from './dev-state.mjs';

const SETTLE = 40;
const DEBOUNCE = 10;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** A hold whose builds wait for `finish()`, on a checkout at `checkout.head`. Every state written is kept. */
function rig() {
	const checkout = { head: 'aaa', stamp: 's1' };
	const writes = [];
	let finish = null;
	const hold = createHold({
		cargo: () => new Promise((resolve) => (finish = resolve)),
		head: () => checkout.head,
		stamp: () => checkout.stamp,
		write: (state) => writes.push(structuredClone(state)),
		settleMs: SETTLE,
		debounceMs: DEBOUNCE
	});
	const build = async (ok = true, stamp = checkout.stamp) => {
		checkout.stamp = stamp;
		finish({ ok, error: ok ? null : 'error[E0425]: cannot find value `x`' });
		await sleep(5);
	};
	return { checkout, writes, hold, build };
}

async function started() {
	const r = rig();
	const first = r.hold.build();
	await r.build();
	await first;
	r.hold.launching();
	return r;
}

test('the first build starts the app, which is told at once that nothing is held back', async () => {
	const { writes } = await started();
	assert.equal(writes.length, 1);
	assert.deepEqual(writes[0], { running: 'aaa', generation: 0, frontend: false, build: { state: 'none', error: null }, dependencies: false });
});

test('a merge that changes both sides is announced as a rebuild, never as only the front end', async () => {
	const { checkout, writes, hold, build } = await started();
	writes.length = 0;
	checkout.head = 'bbb';
	// The front end is held first, the Rust side a moment later: the order a merge or a checkout gives.
	hold.frontendHeld();
	await sleep(SETTLE / 4);
	hold.rustChanged();
	await sleep(DEBOUNCE + SETTLE + 20); // the build is under way by now
	assert.ok(writes.length >= 1, 'written once it settled');
	for (const state of writes) assert.notEqual(state.build.state, 'none', `while it is building: ${JSON.stringify(state)}`);
	assert.equal(writes.at(-1).build.state, 'building');
	assert.equal(writes.at(-1).frontend, true);

	await build(true, 's2');
	await sleep(SETTLE + 20);
	const last = writes.at(-1);
	assert.equal(last.build.state, 'ready');
	assert.equal(last.running, 'aaa', 'the app still runs what it started with');
	assert.equal(writes.filter((s) => s.generation !== last.generation).length, 0, 'one change, one announcement');

	hold.launching();
	assert.deepEqual(writes.at(-1), { running: 'bbb', generation: last.generation, frontend: false, build: { state: 'none', error: null }, dependencies: false });
});

test('a change to the front end alone is a reload, and loading a page takes it in', async () => {
	const { checkout, writes, hold } = await started();
	checkout.head = 'ccc';
	hold.frontendHeld();
	await sleep(SETTLE + 20);
	assert.deepEqual(writes.at(-1).build, { state: 'none', error: null });
	assert.equal(writes.at(-1).frontend, true);
	assert.equal(writes.at(-1).generation, 1);

	hold.pageLoaded();
	await sleep(SETTLE + 20);
	assert.equal(writes.at(-1).frontend, false);
	assert.equal(writes.at(-1).running, 'ccc', 'the window has the files as they are now');
});

test('a build that changes nothing is nothing to update to', async () => {
	const { writes, hold, build } = await started();
	hold.rustChanged();
	await sleep(DEBOUNCE + 5);
	await build(true, 's1'); // cargo found nothing to do: the same program
	await sleep(SETTLE + 20);
	assert.deepEqual(writes.at(-1).build, { state: 'none', error: null });
});

test('a failing build says why, and the next good one is ready', async () => {
	const { writes, hold, build } = await started();
	hold.rustChanged();
	await sleep(DEBOUNCE + 5);
	await build(false);
	await sleep(SETTLE + 20);
	assert.deepEqual(writes.at(-1).build, { state: 'failed', error: 'error[E0425]: cannot find value `x`' });

	hold.rustChanged();
	await sleep(DEBOUNCE + 5);
	await build(true, 's3');
	await sleep(SETTLE + 20);
	assert.equal(writes.at(-1).build.state, 'ready');
	assert.equal(hold.lastError, null);
});

test('changes that keep coming while it builds are built too before it is ready', async () => {
	const { writes, hold, build } = await started();
	hold.rustChanged();
	await sleep(DEBOUNCE + 5);
	hold.rustChanged(); // saved again while the first build runs
	await sleep(DEBOUNCE + 5);
	await build(true, 's2');
	await sleep(SETTLE + 20);
	assert.equal(writes.at(-1).build.state, 'building', 'the first build is out of date already');
	await build(true, 's3');
	await sleep(SETTLE + 20);
	assert.equal(writes.at(-1).build.state, 'ready');
	await hold.settled();
	assert.equal(hold.good.stamp, 's3');
});
