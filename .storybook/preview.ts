// Every story gets Ledger's look (the tokens and base styles of src/app.css, in the theme picked in the toolbar) and
// a stand-in for the Rust backend: Tauri's own IPC mock answers each `invoke`, the plugins' calls and `listen` from the
// sample vault in src/stories/backend.ts. The page is a project's ("smart-greenhouse") unless a story says otherwise.

import type { Preview } from '@storybook/sveltekit';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import '../src/app.css';
import { loadPortflow } from '../src/lib/portflow.svelte';
import { answer } from '../src/stories/backend';
import { PROJECT } from '../src/stories/fixtures';

mockWindows('main');
mockIPC((command, args) => answer(command, (args ?? {}) as Record<string, unknown>), { shouldMockEvents: true });

// What the app's layout loads once at the start, and components read from Ledger's shared state.
void loadPortflow();

const THEMES = ['sepia', 'light', 'dark', 'night', 'forest', 'ocean'];

const preview: Preview = {
	globalTypes: {
		theme: {
			description: 'Ledger theme',
			toolbar: { title: 'Theme', icon: 'paintbrush', items: THEMES, dynamicTitle: true }
		}
	},
	initialGlobals: { theme: 'sepia' },
	decorators: [
		(story, context) => {
			const theme = String(context.globals.theme ?? 'sepia');
			document.documentElement.dataset.theme = theme;
			// A page story runs the app's layout, which takes the theme from where Settings keeps it.
			try {
				localStorage.setItem('ledger-theme', theme);
			} catch {
				// No storage: the layout falls back to sepia.
			}
			return story();
		}
	],
	parameters: {
		layout: 'padded',
		controls: { expanded: true },
		sveltekit_experimental: {
			state: { page: { params: { project: PROJECT }, url: new URL(`http://localhost/projects/${PROJECT}`) } }
		}
	}
};

export default preview;
