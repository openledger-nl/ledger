import { error } from '@sveltejs/kit';
import { api } from '$lib/api';
import type { PageLoad } from './$types';

/** The frontmatter at the top of a markdown file, as Obsidian writes it, and the text after it. */
function splitFrontmatter(text: string): { meta: string; body: string } {
	const found = /^---\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)/.exec(text);
	return found ? { meta: found[1] ?? '', body: text.slice(found[0].length) } : { meta: '', body: text };
}

export const load: PageLoad = async ({ params, url }) => {
	const path = url.searchParams.get('path') ?? '';
	if (!path) error(404, 'No file was asked for');
	try {
		const bytes = await api.readProjectFile(params.project, path);
		const { meta, body } = splitFrontmatter(new TextDecoder().decode(bytes).replace(/^﻿/, ''));
		return { path, meta, html: await api.renderMarkdown(body) };
	} catch (e) {
		error(404, e instanceof Error ? e.message : 'File not found');
	}
};
