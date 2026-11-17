// Searching inside the open thread. On a thread's page the top bar's search box looks in that thread first: what is typed
// lands here, and the thread page shows only its notes, prompts, feedback and uploads that hold all of its words. The
// box's "Everywhere" button (or Enter) searches the whole vault instead, as on every other page.

export const threadSearch = $state({ query: '' });

/** Whether `text` holds every word of `query`, ignoring case; an empty query matches everything. */
export function matches(text: string, query: string): boolean {
	const words = query.toLowerCase().split(/\s+/).filter(Boolean);
	if (!words.length) return true;
	const hay = text.toLowerCase();
	return words.every((w) => hay.includes(w));
}
