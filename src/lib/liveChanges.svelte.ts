// The changes in a deliverable's text since a version, while it is being written: one tracker per page, shared by the
// editor (the bars in its margin and the marks in the text, editorChanges.ts), the card a click on a change opens and the
// review of what AI changed (LiveChanges.svelte), and the Changes section of the side window (ChangesPanel.svelte).
// LiveChanges.svelte also asks the backend (history.rs, `live_changes`) for them whenever the text has rested a moment.

import { SvelteSet } from 'svelte/reactivity';
import type { ChangeView } from '$lib/editorChanges';
import { authorLabel } from '$lib/mmm';
import { toast } from '$lib/ui.svelte';
import type { LiveChange, LiveChanges, ProvenanceVersion } from '$lib/types';

export type TrackedVersion = Pick<ProvenanceVersion, 'version' | 'by' | 'run'>;

/** What the tracker needs of the editor. */
export interface ChangeEditor {
	revert(id: string): boolean;
	showChange(id: string): DOMRect | null;
	rectOf(id: string): DOMRect | null;
}

const SHOWN = 'ledger.changes.shown';
const INLINE = 'ledger.changes.inline';

function stored(key: string, fallback: boolean): boolean {
	try {
		const v = localStorage.getItem(key);
		return v === null ? fallback : v === '1';
	} catch {
		return fallback;
	}
}

function store(key: string, value: string) {
	try {
		localStorage.setItem(key, value);
	} catch {
		// Not kept on this machine: the default comes back next time.
	}
}

export const CHANGE_KIND: Record<LiveChange['kind'], string> = { added: 'Added', changed: 'Changed', removed: 'Taken out', moved: 'Moved here' };

/** A block's text without its markdown marks, as the live preview shows it. */
export const readable = (md: string) => md.replace(/^[ \t]{0,3}(#{1,6}\s+|[-*+]\s+(\[[ xX]\]\s+)?|\d+[.)]\s+|>\s?)/gm, '').trim();

export class ChangeTracker {
	// ---- what it follows (set by LiveChanges.svelte) ----
	project = $state('');
	/** The live file's path. */
	path = $state('');
	text = $state('');
	/** A live markdown file to follow. */
	enabled = $state(false);
	/** The published versions, oldest first. */
	versions = $state<TrackedVersion[]>([]);
	editor: ChangeEditor | undefined;

	// ---- what is shown, kept per machine ----
	/** The bars in the margin. */
	shown = $state(stored(SHOWN, true));
	/** The changes marked in the text too. */
	inline = $state(stored(INLINE, true));

	/** The version compared with; blank for the newest. */
	against = $state('');
	data = $state<LiveChanges | null>(null);
	/** The change whose card is open, and where it is on the screen. */
	popover = $state<{ id: string; rect: DOMRect } | null>(null);

	changes = $derived(this.data?.changes ?? []);
	sources = $derived(this.data?.sources ?? []);
	current = $derived(this.changes.find((c) => c.id === this.popover?.id) ?? null);


	// ---- reviewing what AI changed ----
	reviewing = $state(false);
	/** Kept or undone during this review. */
	handled = new SvelteSet<string>();
	/** How many passages the newest version changed, for the offer to review them. */
	offered = $state(0);
	/** The version gone through last on this machine. */
	reviewedVersion = $state('');

	/** For the editor. */
	view: ChangeView = $derived({ data: this.shown && this.enabled ? this.data : null, inline: this.inline || this.reviewing, focus: this.popover?.id ?? null });

	latest = $derived(this.versions.at(-1) ?? null);
	before = $derived(this.versions.at(-2) ?? null);
	/** The newest version, when AI made it and it hasn't been gone through. */
	toReview = $derived(this.latest && this.before && this.latest.by === 'ai' && this.reviewedVersion !== this.latest.version ? this.latest : null);
	reviewedBy = $derived(this.toReview ? authorLabel(this.toReview) : '');
	queue = $derived(
		this.reviewing && this.toReview && this.data?.against === this.before?.version
			? this.changes.filter((c) => c.by !== null && this.sources[c.by]?.version === this.toReview?.version && !this.handled.has(c.id))
			: []
	);

	get reviewedKey() {
		return `ledger.reviewed:${this.project}/${this.path}`;
	}

	// ---- naming ----

	whoMade(c: LiveChange): string {
		const s = c.by === null ? null : this.sources[c.by];
		if (!s) return `taken out since v${this.data?.against ?? ''}`;
		return s.version ? `${s.who}, v${s.version}` : `${s.who}, not published yet`;
	}

	// ---- looking at a change ----

	/** Opens the card of a change, beside it. */
	open(id: string, rect?: DOMRect) {
		// The editor may not hold the change yet (it was handed over a moment ago): the card opens where the last one was,
		// and moves to it right after.
		const at = this.editor?.rectOf(id) ?? rect ?? this.popover?.rect ?? new DOMRect(0, 0, 0, 0);
		this.popover = { id, rect: at };
	}

	close() {
		this.popover = null;
	}

	/** Scrolls to a change and opens its card; the card moves along once the scroll has settled. */
	show(id: string) {
		this.editor?.showChange(id);
		this.open(id);
		setTimeout(() => {
			this.editor?.showChange(id);
			this.follow();
		});
		requestAnimationFrame(() => requestAnimationFrame(() => this.follow()));
	}

	step(by: number) {
		const list = this.reviewing ? this.queue : this.changes;
		if (!list.length) return;
		const i = list.findIndex((c) => c.id === this.popover?.id);
		const next = i < 0 ? (by > 0 ? 0 : list.length - 1) : (i + by + list.length) % list.length;
		this.show(list[next].id);
	}

	/** Keeps the card beside its change as the page scrolls. */
	follow() {
		if (!this.popover) return;
		const rect = this.editor?.rectOf(this.popover.id);
		if (rect) this.popover = { id: this.popover.id, rect };
	}

	undo(c: LiveChange) {
		if (!this.editor?.revert(c.id)) return;
		this.handled.add(c.id);
		if (this.reviewing) this.next(c);
		else this.popover = null;
	}

	keep(c: LiveChange) {
		this.handled.add(c.id);
		this.next(c);
	}

	private next(after: LiveChange) {
		const rest = this.queue.filter((c) => c.id !== after.id && !this.handled.has(c.id));
		if (!rest.length) return;
		this.show((rest.find((c) => c.from >= after.from) ?? rest[0]).id);
	}

	// ---- settings ----

	setInline(on: boolean) {
		this.inline = on;
		store(INLINE, on ? '1' : '0');
	}

	setShown(on: boolean) {
		this.shown = on;
		store(SHOWN, on ? '1' : '0');
		this.popover = null;
	}

	/** Compares with this version (the newest when blank). */
	since(version: string) {
		this.against = version === this.latest?.version ? '' : version;
		this.popover = null;
	}

	// ---- the review ----

	startReview() {
		if (!this.before) return;
		this.handled.clear();
		this.against = this.before.version;
		this.reviewing = true;
	}

	markReviewed(version: string) {
		this.reviewedVersion = version;
		store(this.reviewedKey, version);
	}

	finish(done: boolean) {
		if (done && this.toReview) {
			toast(`All of ${this.reviewedBy}'s changes in v${this.toReview.version} are reviewed`);
			this.markReviewed(this.toReview.version);
		}
		this.reviewing = false;
		this.against = '';
		this.popover = null;
	}

	/** Another file: its own changes, since its own newest version. */
	reset() {
		this.against = '';
		this.data = null;
		this.popover = null;
		this.reviewing = false;
		try {
			this.reviewedVersion = localStorage.getItem(this.reviewedKey) ?? '';
		} catch {
			this.reviewedVersion = '';
		}
	}
}
