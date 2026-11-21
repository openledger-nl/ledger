// Commands: everything the command palette (Ctrl+P) offers, and what a keyboard shortcut can start. Each command has an
// id, a name and, optionally, a default shortcut; Settings > Tools > Keyboard shortcuts gives any of them a key of your
// own. The keys are kept per machine (localStorage), like the theme.
//
// Pages are commands too: "Go to" every main page, every settings tab, every project and its tabs, and every thread.
// A component that owns what a command does (the search box, the debug capture) hands it in with `provide`.

import { goto } from '$app/navigation';
import { page } from '$app/state';
import { api } from './api';
import { newDeliverableFlow, newProjectFlow, newThreadFlow, projectUrl, threadUrl } from './actions';
import { canonicalHotkey, hotkeyRecorder, prettyHotkey } from './hotkey';
import { toggleSidebar } from './layout.svelte';
import { portflow } from './portflow.svelte';
import { today } from './semesters';
import { attempt, setTheme, themes, toast } from './ui.svelte';
import { openVaultPicker } from './vaults.svelte';
import { groups } from './components/settings/panels';
import type { ProjectSummary } from './types';

export type CommandGroup = 'Create' | 'Go to' | 'Project' | 'Thread' | 'App' | 'Theme';

export interface Command {
	/** Stable: a shortcut is stored by it. */
	id: string;
	title: string;
	group: CommandGroup;
	/** A second line in the palette: the project a thread is in, say. */
	hint?: string;
	/** Extra words the palette also finds it by. */
	keywords?: string;
	defaultKey?: string;
	/** False where it can't run (a project's command with no project open): left out of the palette, still given a key in Settings. */
	available?: boolean;
	run: () => unknown;
}

// ---- what components hand in -----------------------------------------------------------------------

const handlers = new Map<string, () => unknown>();

/** Lets a component do what a command does while it is on screen. Returns the cleanup. */
export function provide(id: string, run: () => unknown): () => void {
	handlers.set(id, run);
	return () => {
		if (handlers.get(id) === run) handlers.delete(id);
	};
}

const handled = (id: string) => () => handlers.get(id)?.();

// ---- the palette ------------------------------------------------------------------------------------

export const palette = $state({ open: false });

export function openPalette() {
	palette.open = true;
}

// ---- starting a meeting -----------------------------------------------------------------------------

/** Set when a meeting is to be added to today's note: the meeting notes page picks it up once it shows today. */
export const meetingStart = $state({ pending: false });

/** Opens today's meeting note with a new meeting in it, its title ready to type in. */
export async function startMeeting() {
	meetingStart.pending = true;
	await goto(`/meetings?day=${today()}`);
}

// ---- every command ----------------------------------------------------------------------------------

const PROJECT_TABS = [
	{ path: '', label: 'Overview' },
	{ path: '/threads', label: 'Threads' },
	{ path: '/deliverables', label: 'Deliverables' },
	{ path: '/attachments', label: 'Attachments' },
	{ path: '/stories', label: 'Stories' },
	{ path: '/board', label: 'Board' },
	{ path: '/people', label: 'People' },
	{ path: '/templates', label: 'Templates' }
];

const go = (href: string) => () => goto(href);

/** The commands that make sense here and now: the ones for the open project only while one is open. */
export function commands(): Command[] {
	const projects = (page.data?.projects ?? []) as ProjectSummary[];
	const current = projects.find((p) => p.slug === page.params.project);
	const list: Command[] = [
		{ id: 'meeting.start', title: 'Start a meeting note', group: 'Create', keywords: 'new meeting daily note today', defaultKey: 'CommandOrControl+Shift+D', run: startMeeting },
		{ id: 'project.new', title: 'New project', group: 'Create', run: newProjectFlow }
	];
	const inProject = (flow: (slug: string) => unknown) => () => (current ? flow(current.slug) : toast('Open a project first'));
	list.push(
		{ id: 'thread.new', title: 'New thread in this project', group: 'Create', hint: current?.name, available: !!current, run: inProject(newThreadFlow) },
		{ id: 'deliverable.new', title: 'New deliverable in this project', group: 'Create', hint: current?.name, available: !!current, run: inProject(newDeliverableFlow) }
	);

	list.push(
		{ id: 'go.home', title: 'Home', group: 'Go to', keywords: 'dashboard start', run: go('/') },
		{ id: 'go.meetings', title: 'Meeting notes', group: 'Go to', keywords: 'daily', run: go('/meetings') },
		{ id: 'go.feedback', title: 'Feedback', group: 'Go to', keywords: 'inbox', run: go('/feedback') },
		{ id: 'go.semesters', title: 'Semesters', group: 'Go to', keywords: 'shared sprints vacation half year calendar nj26', run: go('/semesters') },
		{ id: 'go.people', title: 'People', group: 'Go to', keywords: 'shared coach teammates client contacts', run: go('/people') }
	);
	if (portflow.enabled) list.push({ id: 'go.epsilon', title: 'Epsilon+', group: 'Go to', keywords: 'portflow learning outcomes epsilon', run: go('/epsilon') });
	list.push({ id: 'go.search', title: 'Search page', group: 'Go to', run: go('/search') });
	for (const g of groups) {
		list.push({ id: `go.settings.${g.id}`, title: `Settings: ${g.label}`, group: 'Go to', keywords: 'settings preferences', run: go(`/settings/${g.id}`) });
	}

	for (const p of projects) {
		for (const tab of PROJECT_TABS) {
			list.push({
				id: `go.project.${p.slug}${tab.path}`,
				title: tab.path ? `${p.name}: ${tab.label}` : p.name,
				group: 'Project',
				keywords: 'project',
				run: go(`${projectUrl(p.slug)}${tab.path}`)
			});
		}
	}
	for (const p of projects) {
		for (const t of p.threads) {
			list.push({ id: `go.thread.${p.slug}/${t.slug}`, title: t.title, group: 'Thread', hint: p.name, keywords: 'thread', run: go(threadUrl(p.slug, t.slug)) });
		}
	}

	list.push(
		{ id: 'palette', title: 'Command palette', group: 'App', keywords: 'commands', defaultKey: 'CommandOrControl+P', run: openPalette },
		{ id: 'search', title: 'Search', group: 'App', keywords: 'find', defaultKey: 'CommandOrControl+K', run: handled('search') },
		{ id: 'sidebar.toggle', title: 'Show or hide the sidebar', group: 'App', keywords: 'fold collapse', defaultKey: 'CommandOrControl+\\', run: toggleSidebar },
		{ id: 'screenshot', title: 'Take a screenshot', group: 'App', keywords: 'capture picture', run: () => attempt(api.takeScreenshot) },
		{ id: 'vault.switch', title: 'Switch vault', group: 'App', keywords: 'open another vault folder', run: openVaultPicker },
		{ id: 'debug.capture', title: 'Start or stop a debug capture', group: 'App', keywords: 'record log report bug', defaultKey: 'CommandOrControl+Alt+Shift+D', run: handled('debug.capture') },
		{ id: 'go.settings.shortcuts', title: 'Keyboard shortcuts', group: 'App', keywords: 'hotkeys keys bindings settings', run: go('/settings/tools#shortcuts') }
	);
	for (const t of themes) list.push({ id: `theme.${t.id}`, title: `Theme: ${t.name}`, group: 'Theme', keywords: 'colours colors look', run: () => setTheme(t.id) });

	return list;
}

// ---- the shortcuts ----------------------------------------------------------------------------------

const KEYS_KEY = 'ledger-shortcuts';

/** The shortcuts set here, by command id; "" takes a default one away. */
export const shortcuts = $state<{ keys: Record<string, string> }>({ keys: loadKeys() });

function loadKeys(): Record<string, string> {
	try {
		const saved = JSON.parse(localStorage.getItem(KEYS_KEY) ?? '{}');
		return saved && typeof saved === 'object' ? saved : {};
	} catch {
		return {};
	}
}

function saveKeys() {
	try {
		localStorage.setItem(KEYS_KEY, JSON.stringify(shortcuts.keys));
	} catch {
		// Kept for this session.
	}
}

/** The shortcut a command has now: its own, its default, or "" for none. */
export const keyOf = (c: Pick<Command, 'id' | 'defaultKey'>) => (c.id in shortcuts.keys ? shortcuts.keys[c.id] : (c.defaultKey ?? ''));

/** A command's shortcut as this platform writes it, or "" when it has none. */
export function prettyKeyOf(id: string): string {
	const c = commands().find((x) => x.id === id);
	const key = c ? keyOf(c) : '';
	return key ? prettyHotkey(key) : '';
}

/**
 * Gives a command a shortcut ("" for none). A key another command had moves over: one key starts one thing. Returns the
 * command it was taken from.
 */
export function setKey(target: Command, key: string, all: Command[] = commands()): Command | undefined {
	const wanted = key ? canonicalHotkey(key) : '';
	const taken = wanted ? all.find((c) => c.id !== target.id && canonicalHotkey(keyOf(c)) === wanted) : undefined;
	const next = { ...shortcuts.keys };
	if (taken) next[taken.id] = '';
	if (key === (target.defaultKey ?? '')) delete next[target.id];
	else next[target.id] = key;
	shortcuts.keys = next;
	saveKeys();
	return taken;
}

export function resetKey(target: Command, all?: Command[]) {
	return setKey(target, target.defaultKey ?? '', all);
}

/** The text fields where a plain key is typing, not a shortcut. */
const typing = (el: EventTarget | null) =>
	el instanceof HTMLElement && (el.isContentEditable || el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT');

/**
 * Listens for the shortcuts on every screen. Heard before the page's own keys (an editor would take Ctrl+P), and only
 * once per press. A key without Ctrl, Alt or Super is left to a text field that has the focus. Returns the cleanup.
 */
export function startCommands(): () => void {
	const onKey = (e: KeyboardEvent) => {
		if (e.repeat || hotkeyRecorder.active || e.defaultPrevented) return;
		const plain = !e.ctrlKey && !e.altKey && !e.metaKey;
		if (plain && (typing(e.target) || !/^F\d+$/.test(e.code))) return;
		const pressed = canonicalHotkey(e);
		if (!pressed) return;
		const command = commands().find((c) => {
			const key = keyOf(c);
			return key && canonicalHotkey(key) === pressed;
		});
		if (!command) return;
		e.preventDefault();
		e.stopPropagation();
		if (command.id !== 'palette') palette.open = false;
		Promise.resolve()
			.then(command.run)
			.catch((err) => toast(err instanceof Error ? err.message : String(err), true));
	};
	window.addEventListener('keydown', onKey, { capture: true });
	return () => window.removeEventListener('keydown', onKey, { capture: true });
}

// ---- recently used ----------------------------------------------------------------------------------

const RECENT_KEY = 'ledger-recent-commands';

/** The commands run from the palette, newest first: what it shows on top before anything is typed. */
export const recent = $state<{ ids: string[] }>({ ids: loadRecent() });

function loadRecent(): string[] {
	try {
		const saved = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
		return Array.isArray(saved) ? saved.filter((x) => typeof x === 'string') : [];
	} catch {
		return [];
	}
}

export function noteRecent(id: string) {
	recent.ids = [id, ...recent.ids.filter((x) => x !== id)].slice(0, 8);
	try {
		localStorage.setItem(RECENT_KEY, JSON.stringify(recent.ids));
	} catch {
		// Kept for this session.
	}
}
