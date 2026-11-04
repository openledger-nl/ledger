// The explorer's display preferences: whether the Latest section (the newest threads and deliverables across all
// projects) is shown. Like the theme and the order, kept in the browser's storage, per machine, not in the vault.

const LATEST = 'ledger-show-latest';

function load(): boolean {
	try {
		const saved: unknown = JSON.parse(localStorage.getItem(LATEST) ?? 'null');
		if (typeof saved === 'boolean') return saved;
	} catch {
		// Storage can be unavailable or hold something else; the default is fine.
	}
	return true;
}

export const sidebar = $state<{ showLatest: boolean }>({ showLatest: load() });

/** Settings → Features: whether the explorer shows the Latest section. */
export function setShowLatest(show: boolean) {
	sidebar.showLatest = show;
	try {
		localStorage.setItem(LATEST, JSON.stringify(show));
	} catch {
		// Not persisted, still applied for this session.
	}
}
