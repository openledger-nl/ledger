// What the sidebar shows, as chosen in Settings: the Latest section, the projects, or both. A display preference like
// the theme and the order, so it is kept the same way: in the browser's storage, not in the vault. Read once when this
// module loads, so the first paint already shows the chosen sections.

export type SidebarView = 'both' | 'projects' | 'latest';

const KEY = 'ledger-sidebar';
const views: SidebarView[] = ['both', 'projects', 'latest'];

function load(): SidebarView {
	try {
		const saved = localStorage.getItem(KEY) as SidebarView | null;
		if (saved && views.includes(saved)) return saved;
	} catch {
		// Storage can be unavailable; the default is fine.
	}
	return 'both';
}

/** Both sections by default: Latest above the projects. */
export const sidebar = $state<{ view: SidebarView }>({ view: load() });

export function setSidebarView(view: SidebarView) {
	sidebar.view = view;
	try {
		localStorage.setItem(KEY, view);
	} catch {
		// Not persisted, still applied for this session.
	}
}
