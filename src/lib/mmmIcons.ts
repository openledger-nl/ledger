// The official MMM labels, in colour (src/lib/assets/mmm, from the label pack at https://mmmlabel.tech: free to use in
// any context). Kept apart from mmm.ts, which stays free of bundler imports so it can be exercised on its own.

import type { MmmLevel } from './types';
import cyborg from './assets/mmm/cyborg.svg';
import handyman from './assets/mmm/handyman.svg';
import lazyPrompter from './assets/mmm/lazy-prompter.svg';
import manMade from './assets/mmm/man-made.svg';
import piggybacker from './assets/mmm/piggybacker.svg';

export const MMM_ICONS: Record<MmmLevel, string> = {
	'lazy-prompter': lazyPrompter,
	piggybacker,
	cyborg,
	handyman,
	'man-made': manMade
};

export const mmmIcon = (level: string | null | undefined): string | undefined => (level ? MMM_ICONS[level as MmmLevel] : undefined);
