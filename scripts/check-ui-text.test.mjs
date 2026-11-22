// What scripts/check-ui-text.mjs finds, and what it leaves alone. Runs with `pnpm run check` (node --test).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { check } from './check-ui-text.mjs';

const chars = (found) => found.map((f) => f.char).join('');

test('a Svelte template: its text and attributes, not its comments or styles', () => {
	const source = [
		'<script lang="ts">',
		"\t// a comment may say 1 — 2",
		"\tconst label = 'Rename…';",
		'</script>',
		'<!-- nor a comment · here -->',
		'<p title="Open “x”">a · b</p>',
		'<style>.x::after { content: "→"; }</style>'
	].join('\n');
	assert.equal(chars(check(source, 'svelte')), '…“”·');
});

test('TypeScript: string and template literals, not comments', () => {
	const source = ["/** Saving… */", "const a = 'x — y';", 'const b = `${a} → z`; // …'].join('\n');
	const found = check(source, 'ts');
	assert.equal(chars(found), '—→');
	assert.deepEqual(found.map((f) => f.line), [2, 3]);
});

test('Rust: strings and chars, not comments, not tests, and raw strings too', () => {
	const source = [
		'//! A doc comment — fine.',
		'/* and /* nested … */ still a comment */',
		'fn a<\'a>(s: &\'a str) -> String { format!("{s} · {}", \'…\') }',
		'const RAW: &str = r#"say "“hi”""#;',
		'#[cfg(test)]',
		'mod tests { const T: &str = "—"; }'
	].join('\n');
	assert.equal(chars(check(source, 'rs')), '·…“”');
});

test('ui-text: allow on the line, or the line before, lets a literal through', () => {
	const ts = ["const quotes = ['“', '”']; // ui-text: allow (a parser)", '// ui-text: allow (the next line too)', "const arrow = '→';", "const bad = '→';"].join('\n');
	assert.deepEqual(check(ts, 'ts').map((f) => f.line), [4]);
	const svelte = ['<!-- ui-text: allow (shown as typed) -->', '<p>…</p>', '<p>…</p>'].join('\n');
	assert.deepEqual(check(svelte, 'svelte').map((f) => f.line), [3]);
});

test('each finding says what to use instead', () => {
	const [found] = check("const a = 'a · b';", 'ts');
	assert.equal(found.col, 14);
	assert.match(found.fix, /\|/);
});
