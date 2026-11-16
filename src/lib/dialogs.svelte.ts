// The dialogs that more than one screen opens (New project, a project's goal and end, New deliverable): one place
// holds which is open, and ProjectDialogs.svelte in the root layout shows it.

import type { ProjectMeta } from '$lib/types';

export const projectDialogs = $state<{
	/** New project (null: closed), or an existing project's goal, kind and end. */
	project: { slug: string; name: string; meta: ProjectMeta | null } | 'new' | null;
	/** New deliverable in this project (its slug), with the question it answers when started from one. */
	deliverable: { project: string; answers?: string | null } | null;
}>({ project: null, deliverable: null });
