#!/usr/bin/env node
// pnpm run check:warn (and the end of pnpm run check): the checks that are coming. Each one runs and says what it
// finds, but nothing here fails yet: the existing cases are fixed first, then the check moves to `pnpm run check` as
// an error. Much of this code is written by agents, and these are mistakes they make often.
//
// * Unchecked indexes: TypeScript with `noUncheckedIndexedAccess` (tsconfig.strict.json), where `list[i]` may be
//   undefined. Counted per file; `--list` names every place.
// * Duplicate code: jscpd (.jscpd.json) finds blocks of code that appear twice, token for token, in the app's code
//   (not tests or stories). The copies there now are in .jscpd-baseline.json; only new ones are named. After removing
//   copies, `--update-baseline` writes the smaller list. Never update it to hide a new copy.
//
// Inside GitHub Actions each new copy is also a warning on its lines in the pull request.

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const args = new Set(process.argv.slice(2));
const list = args.has('--list');
const updateBaseline = args.has('--update-baseline');
const github = process.env.GITHUB_ACTIONS === 'true';

const bold = (text) => (process.stdout.isTTY ? `\x1b[1m${text}\x1b[0m` : text);
const rel = (file) => path.relative(ROOT, path.resolve(ROOT, file)).split(path.sep).join('/');

/** Runs a tool from node_modules/.bin. */
function tool(name, toolArgs) {
	const win = process.platform === 'win32';
	const bin = path.join(ROOT, 'node_modules', '.bin', name + (win ? '.cmd' : ''));
	// A .cmd is a script only a shell starts on Windows; the arguments are fixed here, never from outside.
	return spawnSync(bin, toolArgs, { cwd: ROOT, encoding: 'utf8', shell: win, maxBuffer: 64 * 1024 * 1024 });
}

console.log(bold('Coming checks') + ' (warnings for now, errors later: see CONTRIBUTING.md)');

// ---- unchecked indexes ----------------------------------------------------------------------------------------

{
	const run = tool('svelte-check', ['--tsconfig', './tsconfig.strict.json', '--output', 'machine']);
	if (run.error || run.stdout === null) {
		console.log(`\nUnchecked indexes: svelte-check couldn't run (${run.error?.message ?? 'no output'}).`);
	} else {
		// `<time> ERROR "<file>" <line>:<column> "<message>"`. `pnpm run check` has none without the setting, so each
		// one here comes from it.
		const places = [...run.stdout.matchAll(/^\d+ ERROR "([^"]+)" (\d+):(\d+) "(.*)"$/gm)].map(([, file, line, column, message]) => ({
			file: rel(file),
			line,
			column,
			message
		}));
		const files = new Map();
		for (const p of places) files.set(p.file, (files.get(p.file) ?? 0) + 1);
		console.log(`\nUnchecked indexes (noUncheckedIndexedAccess): ${places.length} places in ${files.size} files.`);
		if (places.length) {
			console.log('  `list[i]` may be undefined: check it, or use a value that is there for sure.');
			if (list) for (const p of places) console.log(`  ${p.file}:${p.line}:${p.column}  ${p.message.replace(/\\n/g, ' ')}`);
			else {
				const top = [...files].sort((a, b) => b[1] - a[1]);
				for (const [file, count] of top.slice(0, 5)) console.log(`  ${String(count).padStart(3)}  ${file}`);
				if (top.length > 5) console.log(`  ...and ${top.length - 5} more files. Every place: pnpm run check:warn --list`);
			}
		}
	}
}

// ---- duplicate code -------------------------------------------------------------------------------------------

{
	const baseline = ['--config', '.jscpd.json', '--baseline', '.jscpd-baseline.json'];
	if (updateBaseline) {
		const run = tool('jscpd', [...baseline, '--update-baseline', '--silent']);
		console.log(`\nDuplicate code: ${run.status === 0 ? 'wrote .jscpd-baseline.json from the code as it is now.' : `jscpd failed: ${run.stderr || run.error?.message}`}`);
	} else {
		const out = fs.mkdtempSync(path.join(os.tmpdir(), 'ledger-jscpd-'));
		try {
			const run = tool('jscpd', [...baseline, '--absolute', '--reporters', 'json', '--output', out, '--silent']);
			const report = path.join(out, 'jscpd-report.json');
			if (run.error || !fs.existsSync(report)) {
				console.log(`\nDuplicate code: jscpd couldn't run (${run.error?.message ?? (run.stderr.trim() || 'no report')}).`);
			} else {
				const clones = JSON.parse(fs.readFileSync(report, 'utf8')).duplicates ?? [];
				const fresh = clones.filter((c) => c.isNew);
				console.log(`\nDuplicate code (jscpd): ${clones.length - fresh.length} copies known from before, ${fresh.length} new.`);
				if (fresh.length) console.log('  Use the code that is already there (move it somewhere both can reach) instead of a copy:');
				for (const c of fresh) {
					const a = `${rel(c.firstFile.name)}:${c.firstFile.start}-${c.firstFile.end}`;
					const b = `${rel(c.secondFile.name)}:${c.secondFile.start}-${c.secondFile.end}`;
					console.log(`  ${c.lines} lines: ${a} and ${b}`);
					if (github) {
						const where = `file=${rel(c.secondFile.name)},line=${c.secondFile.start},endLine=${c.secondFile.end}`;
						console.log(`::warning ${where},title=Duplicate code::The same ${c.lines} lines as ${a}`);
					}
				}
			}
		} finally {
			fs.rmSync(out, { recursive: true, force: true });
		}
	}
}
