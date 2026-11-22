// The models offered for Claude Code runs: Settings > Claude Code picks the usual one, and a run's confirmation can pick
// another for that run. They are Claude Code's aliases, which always mean the newest model of that kind; any other name
// Claude Code accepts (a full model id, `sonnet[1m]`) can be typed under "Other".

import type { DropdownOption } from './components/Dropdown.svelte';

/** "" passes no --model: Claude Code uses its own default. */
export const DEFAULT_MODEL = '';
/** The Dropdown value that shows the text field for a name of your own. */
export const OTHER_MODEL = '\u0000other';

const known: DropdownOption[] = [
	{ value: 'opus', label: 'Opus', hint: 'The most capable, and the slowest' },
	{ value: 'sonnet', label: 'Sonnet', hint: 'Fast and capable: the usual choice' },
	{ value: 'haiku', label: 'Haiku', hint: 'The fastest and cheapest, for small tasks' }
];

/** Whether a model name is one of the offered aliases (or the default). */
export const isKnownModel = (model: string) => model === DEFAULT_MODEL || known.some((o) => o.value === model);

/**
 * The options for a model Dropdown. `defaultLabel` names what "" means where it is shown (Claude Code's default in
 * Settings; the model from Settings in a run). A name of your own that is chosen now is shown as an option of its own.
 */
export function modelOptions(defaultLabel: string, current = ''): DropdownOption[] {
	const own = current && !isKnownModel(current) ? [{ value: current, label: current, hint: 'Typed under Other' }] : [];
	return [{ value: DEFAULT_MODEL, label: defaultLabel }, ...known, ...own, { value: OTHER_MODEL, label: 'Other...', hint: 'Type a model name or id' }];
}

/** How a model is named on screen. */
export const modelLabel = (model: string) => known.find((o) => o.value === model)?.label ?? (model || "Claude Code's default");
