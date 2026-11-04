import type { HarnessProvider } from './types';

export const harnesses: { id: HarnessProvider; label: string; program: string }[] = [
	{ id: 'claude', label: 'Claude Code', program: 'claude' },
	{ id: 'codex', label: 'Codex', program: 'codex' },
	{ id: 'agy', label: 'AGY', program: 'agy' },
	{ id: 'prime', label: 'Prime Agent', program: 'prime-agent' }
];

/** Records made before harness selection are Claude Code runs. */
export function harnessLabel(provider?: string): string {
	return harnesses.find((h) => h.id === (provider || 'claude'))?.label ?? provider ?? 'AI harness';
}
