// The [[links]] of the project the window is in, or of the targets and the project a page gives (a meeting's): the index
// they are resolved and completed against (loaded with the page, so every change that refreshes it refreshes them), and
// what opening one does. The rules for matching are in links.ts.

import { goto } from '$app/navigation';
import { page } from '$app/state';
import { deliverableUrl, threadUrl } from './actions';
import { LinkIndex } from './links';
import type { LinkTarget } from './types';

/** Where a link leads, in the target's own project. A piece of feedback opens on the deliverable it is about, with its
 *  window open at it; a meeting day on that day's notes; a person on their page. */
export function linkUrl(target: LinkTarget): string {
	const project = target.project;
	if (target.kind === 'meeting') return `/meetings?day=${encodeURIComponent(target.id)}`;
	if (target.kind === 'person') return `/people?person=${encodeURIComponent(target.id)}`;
	if (target.kind === 'thread') return threadUrl(project, target.id);
	const url = deliverableUrl(project, target.open || target.id);
	return target.kind === 'feedback' ? `${url}&feedback=${encodeURIComponent(target.id)}` : url;
}

/** The targets a page gives its links, and the project they are written in (null for none of its own). */
export interface LinkScope {
	targets: LinkTarget[];
	home: string | null;
}

/**
 * The links of the project the page is in, and a way to follow one. Call it while a component is being set up. `own`
 * gives the targets and their project instead (a meeting's notes, outside any project). Outside a project and without
 * `own` (the vault-wide templates) there is no index: nothing resolves and nothing is offered.
 */
export function projectLinks(own?: () => LinkScope | undefined) {
	const project = $derived(page.params.project ?? null);
	const index = $derived.by(() => {
		const given = own?.();
		if (given) return new LinkIndex(given.targets, given.home);
		const targets = (page.data as { links?: LinkTarget[] }).links;
		return project && targets ? new LinkIndex(targets, project) : null;
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
			void goto(linkUrl(target));
		}
	};
}
