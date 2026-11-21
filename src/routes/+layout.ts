// Tauri has no Node.js server to render on, so the app is a static single-page app:
// https://v2.tauri.app/start/frontend/sveltekit/
import { api } from '$lib/api';
import type { OnboardingStatus, Vaults } from '$lib/types';
import type { LayoutLoad } from './$types';

export const ssr = false;

const noGuide: OnboardingStatus = { active: false, skipped: [], purpose: null };

export const load: LayoutLoad = async ({ url }) => {
	// The screenshot overlay is its own window and needs none of this.
	if (url.pathname.startsWith('/screenshot-overlay')) return { projects: [], onboarding: noGuide, vaults: null as Vaults | null, conflictCount: 0 };
	// Before the first vault there is nothing to load: the window is the vault picker.
	const vaults = await api.listVaults();
	if (!vaults.open) return { projects: [], onboarding: noGuide, vaults: vaults as Vaults | null, conflictCount: 0 };
	const [projects, onboarding, conflicts] = await Promise.all([api.listProjects(), api.onboardingStatus(), api.listConflicts().catch(() => [])]);
	return { projects, onboarding, vaults: vaults as Vaults | null, conflictCount: conflicts.length };
};
