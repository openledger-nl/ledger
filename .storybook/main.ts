// Storybook for Ledger's UI components: `pnpm storybook` (port 6006), `pnpm build-storybook` (to storybook-static/).
// The stories live in src/stories/, one file per component in src/lib/components. Vite's config (vite.config.js) is
// read as for the app; nothing here talks to the Rust backend (src/stories/backend.ts stands in for it).

import type { StorybookConfig } from '@storybook/sveltekit';

const config: StorybookConfig = {
	stories: ['../src/stories/**/*.stories.svelte'],
	addons: ['@storybook/addon-svelte-csf', '@storybook/addon-docs'],
	framework: { name: '@storybook/sveltekit', options: {} },
	core: { disableTelemetry: true }
};

export default config;
