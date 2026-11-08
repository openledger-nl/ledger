// The [[links]] of the project the window is in: the index they are resolved and completed against (loaded with the
// project, so every change that refreshes the page refreshes it), and what opening one does. The rules for matching
// are in links.ts.

import { goto } from '$app/navigation';
import { page } from '$app/state';
import { deliverableUrl, threadUrl } from './actions';
import { LinkIndex } from './links';
import type { LinkTarget } from './types';

/** Where a link leads. A piece of feedback opens on the deliverable it is about, with its window open at it. */
export function linkUrl(project: string, target: LinkTarget): string {
	if (target.kind === 'thread') return threadUrl(project, target.id);
	const url = deliverableUrl(project, target.open || target.id);
	return target.kind === 'feedback' ? `${url}&feedback=${encodeURIComponent(target.id)}` : url;
}

/**
 * The links of the project the page is in, and a way to follow one. Call it while a component is being set up. Outside a
 * project (the vault-wide templates) there is no index: nothing resolves and nothing is offered.
 */
export function projectLinks() {
	const project = $derived(page.params.project ?? null);
	const index = $derived.by(() => {
		const targets = (page.data as { links?: LinkTarget[] }).links;
		return project && targets ? new LinkIndex(targets) : null;
	});
	return {
		get project() {
			return project;
		},
		get index() {
			return index;
		},
		/** Opens what a link points at. */
		open(target: LinkTarget) {
			if (project) void goto(linkUrl(project, target));
		}
	};
}
