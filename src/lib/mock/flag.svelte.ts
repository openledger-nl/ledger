// Whether the v1 preview screens are shown: screens built from the Penpot wireframes before the backend has what they
// need, so they read and write mock data (src/lib/mock) instead of the vault. On in development, off in a release until
// switched on under Settings › Features; kept on this machine.

const KEY = 'ledger-previews';

function load(): boolean {
	try {
		const saved = localStorage.getItem(KEY);
		if (saved === '1' || saved === '0') return saved === '1';
	} catch {
		// Storage unavailable: the default.
	}
	return import.meta.env.DEV;
}

export const previews = $state({ enabled: load() });

export function setPreviews(on: boolean) {
	previews.enabled = on;
	try {
		localStorage.setItem(KEY, on ? '1' : '0');
	} catch {
		// Applied for this session only.
	}
}
