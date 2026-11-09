// The app shell's state: how wide the window is, whether the sidebar is shown, and the drawers that stand in for
// the sidebar and the search box when there is no room for them (see the LoFi wireframes A4, A6 and A7).
//
//   full    1200 px and wider   sidebar with names, side panels beside the page; Ctrl \ folds the sidebar
//   half    720 to 1199 px      sidebar folded (it slides in from the top bar), side panels as drawers, search as an icon
//   narrow  below 720 px        as half, but never a rail
//
// What a folded sidebar becomes is a setting (Settings › Visual › Sidebar): an icon rail, the default, or gone. It is
// kept in the vault's .ledger/settings.json, which never syncs, so each machine has its own; a copy in the browser's
// storage only makes the first paint right before the vault has answered. Whether the sidebar is folded right now is
// a passing state, kept in the browser's storage alone.
//
// The CSS in app.css draws the same three widths with media queries at the same numbers; this module is for what
// scripts decide (which buttons to show, whether a click closes a drawer, what Ctrl K does).

import { api } from './api';
import { toast } from './ui.svelte';
import type { SidebarFold } from './types';

export type Width = 'full' | 'half' | 'narrow';

export const HALF = 1200;
export const NARROW = 720;

const HIDDEN_KEY = 'ledger-sidebar-hidden';
const FOLD_KEY = 'ledger-sidebar-fold';

/** What a folded sidebar becomes: an icon rail, or nothing (it comes back from the top bar). */
export type Fold = SidebarFold;

// Read before the first paint (the app renders in the browser only), so a folded sidebar doesn't flash open.
function savedHidden(): boolean {
	try {
		return localStorage.getItem(HIDDEN_KEY) === '1';
	} catch {
		return false;
	}
}

function savedFold(): Fold {
	try {
		return localStorage.getItem(FOLD_KEY) === 'hide' ? 'hide' : 'rail';
	} catch {
		return 'rail';
	}
}

function widthOf(px: number): Width {
	return px >= HALF ? 'full' : px >= NARROW ? 'half' : 'narrow';
}

export const layout = $state({
	width: typeof window === 'undefined' ? ('full' as Width) : widthOf(window.innerWidth),
	/** The person folded the sidebar at full width; remembered on this machine, like the theme. */
	sidebarHidden: savedHidden(),
	/** What folded means; remembered on this machine. */
	fold: savedFold(),
	/** The whole sidebar, slid in over the page from the top bar's button, below full width. */
	drawerOpen: false,
	/** The search box, opened over the top bar where it is only an icon. */
	searchOpen: false
});

export function setSidebarHidden(hidden: boolean) {
	layout.sidebarHidden = hidden;
	try {
		localStorage.setItem(HIDDEN_KEY, hidden ? '1' : '0');
	} catch {
		// Not remembered; it still folds for this session.
	}
}

function applyFold(fold: Fold) {
	layout.fold = fold;
	try {
		localStorage.setItem(FOLD_KEY, fold);
	} catch {
		// No copy for the first paint; the vault still has it.
	}
}

/** Reads the choice from the vault's settings: at start, and after switching to another vault. */
export async function loadLayoutSettings() {
	try {
		applyFold((await api.layoutSettings()).sidebarFold);
	} catch {
		// The copy from the last run stays; the error is in the log.
	}
}

/** Saves the choice in the vault's settings; it shows at once, and goes back if it can't be saved. */
export async function setFold(fold: Fold) {
	const before = layout.fold;
	applyFold(fold);
	try {
		applyFold((await api.saveSidebarFold(fold)).sidebarFold);
	} catch (e) {
		applyFold(before);
		toast(e instanceof Error ? e.message : String(e), true);
	}
}

/** The one button for the sidebar: folds or unfolds it at full width, slides it in over the page below that. */
export function toggleSidebar() {
	if (layout.width !== 'full') layout.drawerOpen = !layout.drawerOpen;
	else setSidebarHidden(!layout.sidebarHidden);
}

/** Follows the window's width and listens for the shell's shortcuts; returns the clean-up. */
export function startLayout(): () => void {
	loadLayoutSettings();

	const half = window.matchMedia(`(min-width: ${HALF}px)`);
	const narrow = window.matchMedia(`(min-width: ${NARROW}px)`);
	const measure = () => {
		const next = widthOf(window.innerWidth);
		if (next === layout.width) return;
		layout.width = next;
		// A drawer opened for one width makes no sense at another: the sidebar is back in place, or the search box is.
		layout.drawerOpen = false;
		layout.searchOpen = false;
	};
	measure();
	half.addEventListener('change', measure);
	narrow.addEventListener('change', measure);

	const onKey = (e: KeyboardEvent) => {
		const mod = e.ctrlKey || e.metaKey;
		if (mod && !e.altKey && !e.shiftKey && (e.code === 'Backslash' || e.key === '\\')) {
			e.preventDefault();
			toggleSidebar();
		} else if (e.key === 'Escape' && (layout.drawerOpen || layout.searchOpen)) {
			layout.drawerOpen = false;
			layout.searchOpen = false;
		}
	};
	window.addEventListener('keydown', onKey);

	return () => {
		half.removeEventListener('change', measure);
		narrow.removeEventListener('change', measure);
		window.removeEventListener('keydown', onKey);
	};
}
