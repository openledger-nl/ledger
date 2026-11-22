import { goto } from '$app/navigation';
import { api } from './api';
import { attempt, confirmDialog, promptDialog, refresh } from './ui.svelte';
import type { Bump, DeliverableSummary, Prompt } from './types';
import { projectDialogs } from './dialogs.svelte';

export const projectUrl = (project: string) => `/projects/${project}`;
export const threadUrl = (project: string, thread: string) => `/projects/${project}/threads/${thread}`;
/** A deliverable's page; with `thread`, opened from that thread: the top bar leads back to it, and new feedback and a
 *  publish note go there. */
export const deliverableUrl = (project: string, path: string, thread?: string) =>
	`/projects/${project}/deliverables/view?path=${encodeURIComponent(path)}${thread ? `&thread=${encodeURIComponent(thread)}` : ''}`;

/** Opens New project, which asks for the kind, the goal and the end first. */
export function newProjectFlow() {
	projectDialogs.project = 'new';
}

/** Asks for a title, creates the thread in the project and opens it. */
export async function newThreadFlow(project: string) {
	const title = await promptDialog({
		title: 'New thread',
		label: 'What are you thinking about?',
		placeholder: 'e.g. Pricing model',
		confirmLabel: 'Create'
	});
	if (!title) return;
	const created = await attempt(() => api.createThread(project, title));
	if (!created) return;
	await refresh();
	await goto(threadUrl(project, created.slug));
}

/** A learning outcome's level, as Epsilon+ shows it: what a thread or deliverable started from it aims at. */
interface AimedLevel {
	name: string;
	level: number;
	description: string;
}

/** The opening of something started from a level in Epsilon+: which level it aims at, and what that level asks. */
function aimNote(aim: AimedLevel): string {
	const asks = aim.description.trim();
	return `Working towards **${aim.name}** (level ${aim.level}).${asks ? `\n\n${asks.split('\n').map((line) => `> ${line}`.trimEnd()).join('\n')}` : ''}`;
}

/** From Epsilon+: asks for a title, creates a thread in the project whose first post says which level it aims at and what
 *  that level asks, and opens it. */
export async function threadForLevelFlow(project: string, aim: AimedLevel) {
	const title = await promptDialog({
		title: `New thread for ${aim.name}`,
		label: 'What will you work on?',
		initial: `${aim.name}: `,
		confirmLabel: 'Create'
	});
	if (!title) return;
	const created = await attempt(() => api.createThread(project, title));
	if (!created) return;
	await attempt(() => api.addPost(project, created.slug, aimNote(aim)));
	await refresh();
	await goto(threadUrl(project, created.slug));
}

/** From Epsilon+: asks for a file name, creates the deliverable starting with which level it aims at (a markdown file; any
 *  other kind starts empty), and opens it. */
export async function deliverableForLevelFlow(project: string, aim: AimedLevel) {
	const slug = aim.name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'deliverable';
	const name = await promptDialog({
		title: `New deliverable for ${aim.name}`,
		label: 'File name',
		initial: `${slug}.md`,
		confirmLabel: 'Create'
	});
	if (!name) return;
	const title = name.replace(/^.*\//, '').replace(/\.[^.]+$/, '').replace(/[-_]+/g, ' ');
	const body = /\.md$/i.test(name) ? `# ${title.charAt(0).toUpperCase()}${title.slice(1)}\n\n${aimNote(aim)}\n` : '';
	const made = await attempt(() => api.createDeliverable(project, name, body));
	if (!made) return;
	await refresh();
	await goto(deliverableUrl(project, made.path));
}

/** Opens New deliverable, which starts from its goal and numbers it. (The Deliverables tab can also start one from a template.) */
export function newDeliverableFlow(project: string) {
	projectDialogs.deliverable = { project };
}

export async function renameProjectFlow(slug: string, current: string) {
	const name = await promptDialog({ title: 'Rename project', label: 'Name', initial: current, confirmLabel: 'Rename' });
	if (!name || name === current) return;
	if (await attempt(() => api.renameProject(slug, name))) await refresh();
}

export async function deleteProjectFlow(slug: string, name: string) {
	const ok = await confirmDialog({
		title: `Delete "${name}"?`,
		message: 'Its threads, deliverables, stories, tasks and uploaded files are removed from the vault. Your cloud storage keeps them in its recycle bin for a while.',
		confirmLabel: 'Delete project',
		danger: true
	});
	if (!ok) return;
	if ((await attempt(() => api.deleteProject(slug))) === undefined) return;
	await refresh();
	await goto('/');
}

/** The next version a publish would create, mirroring the backend: "M.m" from the highest published one. */
export function nextVersion(versions: string[], bump: Bump): string {
	const [major, minor] = (versions.at(-1) ?? '0.0').split('.').map(Number);
	return bump === 'major' ? `${major + 1}.0` : `${major}.${minor + 1}`;
}

/** "reports/plan.md" + "1.2" → "reports/plan@v1.2.md", as the backend names a published version. */
export function snapshotPath(base: string, version: string): string {
	const slash = base.lastIndexOf('/');
	const dir = slash < 0 ? '' : base.slice(0, slash + 1);
	const file = base.slice(slash + 1);
	const dot = file.lastIndexOf('.');
	return dir + (dot > 0 ? `${file.slice(0, dot)}@v${version}${file.slice(dot)}` : `${file}@v${version}`);
}

/** Where a deliverable opens: its live file, or its newest published version when the live file is gone. */
export const deliverableTarget = (d: DeliverableSummary) =>
	d.exists || d.versions.length === 0 ? d.path : snapshotPath(d.path, d.versions.at(-1) as string);

/** The file name of a deliverable path, without its folders. */
export const baseName = (path: string) => path.slice(path.lastIndexOf('/') + 1);

/** The live file behind a deliverable path: feedback given on version 0.1 names "plan@v0.1.md", whose live file is "plan.md". */
export const liveDeliverable = (path: string, version: string | null) => (version ? path.replace(`@v${version}`, '') : path);

/** What a prompt is called on screen: the name it was given, or the start of what it asks while it has none. */
export function promptName(prompt: Pick<Prompt, 'title' | 'base' | 'body'>): string {
	const title = prompt.title.trim();
	if (title) return title;
	const line = (prompt.body.trim() || prompt.base.trim()).split('\n').find((l) => l.trim()) ?? 'Untitled prompt';
	return line.length > 60 ? `${line.slice(0, 60).trimEnd()}...` : line;
}

/** Whether a deliverable path names a published version ("reports/plan@v1.2.md", "LICENSE@v2.0"), as the backend reads it. */
export const isVersionPath = (path: string) => /^.+@v\d+\.\d+(\.[^.]*)?$/.test(baseName(path));

/**
 * Where a project's deliverable lives relative to the project's folder (what file commands take), as the backend puts it
 * (`deliverables::in_project`): the live file under `deliverables/`, a published version under `versions/`, in a folder
 * of its deliverable's own named after it ("reports/plan@v1.2.md" is `versions/reports/plan/plan@v1.2.md`).
 */
export function deliverableFile(path: string): string {
	if (!isVersionPath(path)) return `deliverables/${path}`;
	const name = baseName(path);
	const live = name.replace(/@v\d+\.\d+(?=(\.[^.]*)?$)/, '');
	const dot = live.lastIndexOf('.');
	return `versions/${path.slice(0, path.length - name.length)}${dot > 0 ? live.slice(0, dot) : live}/${name}`;
}

export function mimeOf(name: string): string {
	const ext = name.split('.').pop()?.toLowerCase() ?? '';
	const types: Record<string, string> = {
		png: 'image/png',
		jpg: 'image/jpeg',
		jpeg: 'image/jpeg',
		gif: 'image/gif',
		webp: 'image/webp',
		svg: 'image/svg+xml',
		bmp: 'image/bmp',
		avif: 'image/avif',
		jxl: 'image/jxl',
		pdf: 'application/pdf'
	};
	return types[ext] ?? 'application/octet-stream';
}

/** Whether a file is a picture Ledger shows (the backend's `uploads::is_image`, which also decides what has a preview). */
export const isImageName = (name: string) => /\.(png|jpe?g|gif|webp|svg|bmp|jxl)$/i.test(name);

/** The type of a picture as the backend hands it to the window: a JPEG XL one comes decoded, as PNG (`images.rs`). */
export const shownMimeOf = (name: string) => (/\.jxl$/i.test(name) ? 'image/png' : mimeOf(name));

export function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10_240 ? 1 : 0)} KB`;
	return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
