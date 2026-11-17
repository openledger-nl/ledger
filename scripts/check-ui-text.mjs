#!/usr/bin/env node
// pnpm run check: UI text uses plain punctuation. No middle dot, em or en dash, ellipsis character, curly quotes or
// arrows in what a person reads in Ledger: use - or |, three dots, straight quotes, "to". Only text that can reach the
// screen is looked at, never comments:
//
// * .svelte under src/: the template's text and attribute values, and string literals in its scripts and expressions
//   (parsed with svelte/compiler, so comments are left out);
// * .ts under src/: string and template literals (parsed with TypeScript);
// * .rs under src-tauri/src/, except tests*.rs and whatever follows a top-level #[cfg(test)]: string and char literals
//   (error messages, toasts, sync reports, log lines and the text Ledger writes into the vault).
//
// A literal that needs one of these characters to work (a parser that recognises it in someone else's text) is let
// through by `ui-text: allow` in a comment on its line or the line before. Keep that rare and say why.

import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { parse } from 'svelte/compiler';

/** The characters, and what to write instead. */
export const BANNED = new Map([
	['·', '" | " between items, or "-"'],
	['—', '" - ", or a new sentence'],
	['–', '"-"'],
	['…', 'nothing on a button or menu item, else "..."'],
	['“', 'a straight "'],
	['”', 'a straight "'],
	['‘', "a straight '"],
	['’', "a straight '"],
	['→', '"to", or "-"'],
	['←', '"from", or "-"']
]);

const ALLOW = 'ui-text: allow';

/** Not checked: the GitHub sync's screens, which are being left as they are for now (asked not to be changed). */
const EXCLUDED = new Set([
	'src/lib/components/SyncWidget.svelte',
	'src/lib/components/settings/GitHubSettings.svelte',
	'src/lib/components/settings/EncryptionSettings.svelte'
]);

/** Where each banned character is in `text`: its offset in it. */
export function bannedIn(text) {
	const out = [];
	for (let i = 0; i < text.length; i++) if (BANNED.has(text[i])) out.push(i);
	return out;
}

// ---- where the text is ---------------------------------------------------------------------------
// Each scanner gives the pieces of a file a person can read: [start offset, text].

/** String and template literals in TypeScript or JavaScript. */
export function piecesOfTs(source, fileName = 'file.ts') {
	const file = ts.createSourceFile(fileName, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
	const out = [];
	const visit = (node) => {
		switch (node.kind) {
			case ts.SyntaxKind.StringLiteral:
			case ts.SyntaxKind.NoSubstitutionTemplateLiteral:
			case ts.SyntaxKind.TemplateHead:
			case ts.SyntaxKind.TemplateMiddle:
			case ts.SyntaxKind.TemplateTail: {
				const start = node.getStart(file);
				out.push([start, source.slice(start, node.end)]);
				break;
			}
		}
		ts.forEachChild(node, visit);
	};
	visit(file);
	return out;
}

/** A Svelte component's template text, attribute values, and the string literals of its scripts and expressions. */
export function piecesOfSvelte(source) {
	const root = parse(source, { modern: true });
	const out = [];
	const seen = new Set();
	const walk = (node) => {
		if (!node || typeof node !== 'object' || seen.has(node)) return;
		seen.add(node);
		if (Array.isArray(node)) return node.forEach(walk);
		if (node.type === 'Comment' || node.type === 'StyleSheet') return;
		const isText = node.type === 'Text' || (node.type === 'Literal' && typeof node.value === 'string') || node.type === 'TemplateElement';
		if (isText && typeof node.start === 'number' && typeof node.end === 'number') out.push([node.start, source.slice(node.start, node.end)]);
		for (const [key, value] of Object.entries(node)) {
			if (key === 'css' || key === 'parent' || key === 'loc' || key === 'leadingComments' || key === 'trailingComments') continue;
			if (value && typeof value === 'object') walk(value);
		}
	};
	walk(root);
	return out;
}

/** String and char literals in Rust, up to a top-level #[cfg(test)]; comments are skipped. */
export function piecesOfRust(source) {
	const cut = source.search(/^#\[cfg\(test\)\]/m);
	const text = cut === -1 ? source : source.slice(0, cut);
	const out = [];
	let i = 0;
	while (i < text.length) {
		const c = text[i];
		const next = text[i + 1];
		if (c === '/' && next === '/') {
			const end = text.indexOf('\n', i);
			i = end === -1 ? text.length : end;
		} else if (c === '/' && next === '*') {
			let depth = 1;
			i += 2;
			while (i < text.length && depth > 0) {
				if (text[i] === '/' && text[i + 1] === '*') (depth++, (i += 2));
				else if (text[i] === '*' && text[i + 1] === '/') (depth--, (i += 2));
				else i++;
			}
		} else if ((c === 'r' || (c === 'b' && next === 'r')) && /^b?r#*"/.test(text.slice(i, i + 40)) && !/[A-Za-z0-9_]/.test(text[i - 1] ?? '')) {
			const [head, hashes] = /^b?r(#*)"/.exec(text.slice(i));
			const close = '"' + hashes;
			const end = text.indexOf(close, i + head.length);
			const stop = end === -1 ? text.length : end + close.length;
			out.push([i, text.slice(i, stop)]);
			i = stop;
		} else if (c === '"') {
			let j = i + 1;
			while (j < text.length && text[j] !== '"') j += text[j] === '\\' ? 2 : 1;
			out.push([i, text.slice(i, j + 1)]);
			i = j + 1;
		} else if (c === "'") {
			// A char literal ('x', '\n', '\u{2026}'); otherwise a lifetime ('a), which is left alone.
			const m = /^'(\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'/.exec(text.slice(i, i + 14));
			if (m) {
				out.push([i, m[0]]);
				i += m[0].length;
			} else i++;
		} else i++;
	}
	return out;
}

// ---- checking ------------------------------------------------------------------------------------

/** Every banned character in a file's readable text: { line, col, char, snippet, fix }. */
export function check(source, kind) {
	const pieces = kind === 'svelte' ? piecesOfSvelte(source) : kind === 'rs' ? piecesOfRust(source) : piecesOfTs(source);
	const lines = source.split('\n');
	const lineStarts = [0];
	for (let i = 0; i < source.length; i++) if (source[i] === '\n') lineStarts.push(i + 1);
	const lineOf = (offset) => {
		let lo = 0;
		let hi = lineStarts.length - 1;
		while (lo < hi) {
			const mid = (lo + hi + 1) >> 1;
			if (lineStarts[mid] <= offset) lo = mid;
			else hi = mid - 1;
		}
		return lo;
	};
	const found = [];
	const reported = new Set();
	for (const [start, text] of pieces) {
		for (const at of bannedIn(text)) {
			const offset = start + at;
			if (reported.has(offset)) continue;
			reported.add(offset);
			const line = lineOf(offset);
			if (lines[line].includes(ALLOW) || (line > 0 && lines[line - 1].includes(ALLOW))) continue;
			const char = source[offset];
			const col = offset - lineStarts[line];
			const snippet = lines[line].slice(Math.max(0, col - 30), col + 30).trim();
			found.push({ line: line + 1, col: col + 1, char, snippet, fix: BANNED.get(char) });
		}
	}
	return found.sort((a, b) => a.line - b.line || a.col - b.col);
}

function* filesUnder(dir, extensions) {
	let entries;
	try {
		entries = readdirSync(dir, { withFileTypes: true });
	} catch {
		return;
	}
	for (const entry of entries) {
		const full = path.join(dir, entry.name);
		if (entry.isDirectory()) {
			if (entry.name === 'node_modules' || entry.name.startsWith('.')) continue;
			yield* filesUnder(full, extensions);
		} else if (extensions.some((ext) => entry.name.endsWith(ext)) && !entry.name.endsWith('.d.ts')) {
			yield full;
		}
	}
}

/** Every finding in the repository at `root`. */
export function checkRepo(root) {
	const findings = [];
	const look = (file, kind) => {
		if (EXCLUDED.has(path.relative(root, file).split(path.sep).join('/'))) return;
		for (const f of check(readFileSync(file, 'utf8'), kind)) findings.push({ file: path.relative(root, file), ...f });
	};
	for (const file of filesUnder(path.join(root, 'src'), ['.svelte', '.ts'])) look(file, file.endsWith('.svelte') ? 'svelte' : 'ts');
	for (const file of filesUnder(path.join(root, 'src-tauri', 'src'), ['.rs'])) {
		if (!/^tests[^/\\]*\.rs$/.test(path.basename(file))) look(file, 'rs');
	}
	return findings;
}

const name = (char) => `U+${char.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')} ${char}`;

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
	const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
	const findings = checkRepo(root);
	for (const f of findings) console.log(`${f.file}:${f.line}:${f.col}  ${name(f.char)}  ${f.snippet}\n    use ${f.fix}`);
	if (findings.length) {
		console.error(`\n${findings.length} typographic character(s) in UI text. Use - or |, three dots and straight quotes (scripts/check-ui-text.mjs).`);
		process.exit(1);
	}
	console.log('UI text: plain punctuation only.');
}
