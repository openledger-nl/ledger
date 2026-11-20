// The changes in a document since a version, shown in the editor while it is written (see LiveChanges.svelte). Always: a
// thin bar in the margin beside each changed passage, coloured by who made the change (you, or AI), and a small notch
// where a passage was taken out; a click on one asks the page to say more. On request: the changes marked in the text
// itself, as Word's track changes: added words underlined, removed ones struck through where they were.
//
// The backend works the changes out (history.rs, `live_changes`) from the text as it was a moment ago. Until its next
// answer, the changes move along with what is typed, so the bars never point at the wrong line.

import { RangeSet, StateEffect, StateField, type ChangeDesc, type EditorState, type Extension, type Range } from '@codemirror/state';
import { Decoration, EditorView, GutterMarker, ViewPlugin, WidgetType, gutter, type DecorationSet } from '@codemirror/view';
import type { HistorySource, LiveChange, LiveChanges } from '$lib/types';

export interface ChangeView {
	data: LiveChanges | null;
	/** Mark the changes in the text, not only in the margin. */
	inline: boolean;
	/** The change being looked at (stepped to, or reviewed). */
	focus: string | null;
}

const setView = StateEffect.define<ChangeView>();

interface Tracked {
	view: ChangeView;
	/** The changes, their positions moved along with what was typed since the backend answered. */
	changes: LiveChange[];
	decorations: DecorationSet;
	markers: RangeSet<GutterMarker>;
}

/** "ai", "human", "unknown", or "gone" for a change that only took something out. */
export function authorOf(change: LiveChange, sources: HistorySource[]): string {
	return change.by === null ? 'gone' : (sources[change.by]?.by ?? 'unknown');
}

/** A change's positions kept inside a text of `length`: an answer about a moment ago may point past its end. */
function clampChange(c: LiveChange, length: number): LiveChange {
	const at = (pos: number) => Math.max(0, Math.min(length, pos));
	return {
		...c,
		from: at(c.from),
		to: at(c.to),
		inserts: c.inserts.map((s) => ({ ...s, from: at(s.from), to: at(s.to) })),
		deletes: c.deletes.map((d) => ({ ...d, at: at(d.at) })),
		revert: c.revert.map((e) => ({ ...e, from: at(e.from), to: at(e.to) }))
	};
}

function mapChange(c: LiveChange, m: ChangeDesc): LiveChange {
	const at = (pos: number, assoc = 1) => m.mapPos(Math.max(0, Math.min(m.length, pos)), assoc);
	return {
		...c,
		from: at(c.from, -1),
		to: Math.max(at(c.from, -1), at(c.to, 1)),
		inserts: c.inserts.map((s) => ({ ...s, from: at(s.from, 1), to: at(s.to, -1) })).filter((s) => s.to > s.from),
		deletes: c.deletes.map((d) => ({ ...d, at: at(d.at, -1) })),
		revert: c.revert.map((e) => ({ ...e, from: at(e.from, -1), to: Math.max(at(e.from, -1), at(e.to, 1)) }))
	};
}

class Bar extends GutterMarker {
	constructor(
		readonly cls: string,
		readonly id: string
	) {
		super();
	}
	eq(other: Bar) {
		return other.cls === this.cls && other.id === this.id;
	}
	toDOM() {
		const el = document.createElement('div');
		el.className = `cm-ch-bar ${this.cls}`;
		el.dataset.change = this.id;
		return el;
	}
}

/** Text that was taken out, where it was: struck through, and not part of the text (it can't be edited or selected). */
class Gone extends WidgetType {
	constructor(
		readonly text: string,
		readonly block: boolean,
		readonly id: string
	) {
		super();
	}
	eq(other: Gone) {
		return other.text === this.text && other.block === this.block && other.id === this.id;
	}
	toDOM() {
		const el = document.createElement(this.block ? 'div' : 'span');
		el.className = this.block ? 'cm-ch-gone-block' : 'cm-ch-gone';
		el.dataset.change = this.id;
		// A whole block reads without its markdown marks, as the live preview would show it.
		el.textContent = this.block ? this.text.replace(/^[ \t]{0,3}(#{1,6}\s+|[-*+]\s+(\[[ xX]\]\s+)?|\d+[.)]\s+|>\s?)/gm, '') : this.text;
		return el;
	}
	ignoreEvent() {
		return false;
	}
}

function build(state: EditorState, view: ChangeView, changes: LiveChange[]) {
	const sources = view.data?.sources ?? [];
	const bars = new Map<number, Bar>();
	const found: Range<Decoration>[] = [];
	const length = state.doc.length;
	const clamp = (pos: number) => Math.max(0, Math.min(length, pos));
	for (const c of changes) {
		const who = authorOf(c, sources);
		const focused = view.focus === c.id ? ' cm-ch-focus' : '';
		if (c.kind === 'removed' || c.to <= c.from) {
			const line = state.doc.lineAt(clamp(c.from));
			if (!bars.has(line.from)) bars.set(line.from, new Bar(`cm-ch-removed${focused}`, c.id));
		} else {
			const first = state.doc.lineAt(clamp(c.from)).number;
			const last = state.doc.lineAt(clamp(Math.max(c.from, c.to - 1))).number;
			for (let n = first; n <= last; n++) {
				const from = state.doc.line(n).from;
				const ends = `${n === first ? ' cm-ch-first' : ''}${n === last ? ' cm-ch-last' : ''}`;
				bars.set(from, new Bar(`cm-ch-${who}${c.kind === 'moved' ? ' cm-ch-moved' : ''}${ends}${focused}`, c.id));
				if (focused) found.push(Decoration.line({ class: 'cm-ch-focusline' }).range(from));
			}
		}
		if (!view.inline) continue;
		for (const s of c.inserts) {
			if (s.to <= s.from || s.to > length) continue;
			const by = sources[s.by]?.by ?? 'unknown';
			found.push(Decoration.mark({ class: `cm-ch-ins cm-ch-${by}`, attributes: { 'data-change': c.id } }).range(s.from, s.to));
		}
		for (const d of c.deletes) {
			const at = clamp(d.at);
			found.push(Decoration.widget({ widget: new Gone(d.text, d.block, c.id), side: -1, block: d.block }).range(at));
		}
	}
	const markers = RangeSet.of(
		[...bars.entries()].sort((a, b) => a[0] - b[0]).map(([from, bar]) => bar.range(from)),
		true
	);
	return { decorations: Decoration.set(found, true), markers };
}

const tracked = StateField.define<Tracked>({
	create: () => ({ view: { data: null, inline: false, focus: null }, changes: [], decorations: Decoration.none, markers: RangeSet.empty }),
	update(value, tr) {
		let { view, changes } = value;
		let stale = false;
		if (tr.docChanged) {
			changes = changes.map((c) => mapChange(c, tr.changes));
			stale = true;
		}
		for (const effect of tr.effects) {
			if (effect.is(setView)) {
				view = effect.value;
				changes = (view.data?.changes ?? []).map((c) => clampChange(c, tr.state.doc.length));
				stale = true;
			}
		}
		return stale ? { view, changes, ...build(tr.state, view, changes) } : value;
	},
	provide: (field) => EditorView.decorations.from(field, (value) => value.decorations)
});

const looks = EditorView.baseTheme({
	'.cm-gutters': { backgroundColor: 'transparent', border: 'none' },
	'.cm-ch-gutter .cm-gutterElement': { padding: '0 8px 0 0', cursor: 'default' },
	'.cm-ch-bar': { width: '3px', height: '100%', borderRadius: '0', backgroundColor: 'var(--green)', cursor: 'pointer', transition: 'width 120ms' },
	'.cm-ch-bar.cm-ch-first': { borderTopLeftRadius: '2px', borderTopRightRadius: '2px' },
	'.cm-ch-bar.cm-ch-last': { borderBottomLeftRadius: '2px', borderBottomRightRadius: '2px' },
	'.cm-ch-bar.cm-ch-ai': { backgroundColor: 'var(--accent)' },
	'.cm-ch-bar.cm-ch-unknown': { backgroundColor: 'var(--faint)' },
	'.cm-ch-bar.cm-ch-gone': { backgroundColor: 'var(--red)' },
	'.cm-ch-bar.cm-ch-moved': { backgroundImage: 'repeating-linear-gradient(to bottom, transparent 0 3px, var(--card) 3px 5px)' },
	'.cm-ch-bar:hover, .cm-ch-bar.cm-ch-focus': { width: '5px' },
	'.cm-ch-bar.cm-ch-removed': {
		width: '0',
		height: '0',
		borderTop: '5px solid transparent',
		borderBottom: '5px solid transparent',
		borderLeft: '6px solid var(--red)',
		backgroundColor: 'transparent',
		marginTop: '-5px'
	},
	'.cm-ch-bar.cm-ch-removed.cm-ch-focus': { borderLeftWidth: '8px' },
	'.cm-ch-focusline': { backgroundColor: 'var(--amber-soft)' },
	'.cm-ch-ins': { backgroundColor: 'var(--green-soft)', borderBottom: '1px solid var(--green)' },
	'.cm-ch-ins.cm-ch-ai': { backgroundColor: 'var(--accent-soft)', borderBottomColor: 'var(--accent)' },
	'.cm-ch-ins.cm-ch-unknown': { backgroundColor: 'transparent', borderBottom: '1px dotted var(--faint)' },
	'.cm-ch-gone': { color: 'var(--muted)', textDecoration: 'line-through', textDecorationColor: 'var(--red)', margin: '0 2px', cursor: 'pointer' },
	'.cm-ch-gone-block': {
		color: 'var(--muted)',
		textDecoration: 'line-through',
		textDecorationColor: 'var(--red)',
		whiteSpace: 'pre-wrap',
		borderLeft: '2px solid var(--red)',
		paddingLeft: '10px',
		margin: '2px 0 8px',
		cursor: 'pointer'
	}
});

// The bars are placed by the heights the editor measured. Measured before the fonts were in, or before the page around it
// settled on its width, those heights are those of unwrapped lines and the bars drift; so it measures again then.
const remeasure = ViewPlugin.fromClass(
	class {
		private observer: ResizeObserver | null;
		constructor(view: EditorView) {
			const again = () => view.requestMeasure();
			void document.fonts?.ready.then(again);
			this.observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(again);
			this.observer?.observe(view.contentDOM);
		}
		destroy() {
			this.observer?.disconnect();
		}
	}
);

/** The margin, marks and clicks; `clicked` hears a click on a change's bar or mark, with where it is on the screen. */
export function changeTracking(clicked?: (id: string, rect: DOMRect) => void): Extension {
	return [
		tracked,
		remeasure,
		gutter({
			class: 'cm-ch-gutter',
			markers: (view) => view.state.field(tracked).markers,
			// A table (drawn as a whole in the live preview) and a passage that was taken out are blocks of their own: their
			// bar comes from here.
			widgetMarker: (view, widget, block) => {
				if (widget instanceof Gone) return new Bar('cm-ch-gone cm-ch-first cm-ch-last', widget.id);
				const { view: shown, changes } = view.state.field(tracked);
				const c = changes.find((c) => c.kind !== 'removed' && c.from < block.to && c.to > block.from);
				if (!c) return null;
				const who = authorOf(c, shown.data?.sources ?? []);
				return new Bar(`cm-ch-${who} cm-ch-first cm-ch-last${shown.focus === c.id ? ' cm-ch-focus' : ''}`, c.id);
			},
			domEventHandlers: {
				mousedown(_view, _line, event) {
					const bar = (event.target as HTMLElement | null)?.closest?.<HTMLElement>('[data-change]');
					if (!bar?.dataset.change) return false;
					clicked?.(bar.dataset.change, bar.getBoundingClientRect());
					return true;
				}
			}
		}),
		EditorView.domEventHandlers({
			click(event, view) {
				const el = (event.target as HTMLElement | null)?.closest?.<HTMLElement>('.cm-content [data-change]');
				if (!el?.dataset.change || !view.state.selection.main.empty) return false;
				clicked?.(el.dataset.change, el.getBoundingClientRect());
				return false;
			}
		}),
		looks
	];
}

export function showChanges(view: EditorView, next: ChangeView) {
	view.dispatch({ effects: setView.of(next) });
}

function find(view: EditorView, id: string): LiveChange | undefined {
	return view.state.field(tracked, false)?.changes.find((c) => c.id === id);
}

/** Puts back what the change replaced, as an edit of its own (so Ctrl+Z brings the change back). */
export function revertChange(view: EditorView, id: string): boolean {
	const change = find(view, id);
	if (!change) return false;
	const length = view.state.doc.length;
	const changes = [...change.revert]
		.map((e) => ({ from: Math.min(e.from, length), to: Math.min(e.to, length), insert: e.insert }))
		.sort((a, b) => a.from - b.from);
	view.dispatch({ changes, userEvent: 'input.revert', scrollIntoView: false });
	return true;
}

/** Scrolls the change into the middle of the editor. */
export function revealChange(view: EditorView, id: string): boolean {
	const change = find(view, id);
	if (!change) return false;
	view.dispatch({ effects: EditorView.scrollIntoView(Math.min(change.from, view.state.doc.length), { y: 'center' }) });
	return true;
}

/** Where the change is on the screen: from the top of its first line to the bottom of its last. Measured by the editor's
 * line blocks, so a change in a table (a widget in the live preview) has a place too. */
export function changeRect(view: EditorView, id: string): DOMRect | null {
	const change = find(view, id);
	if (!change) return null;
	const length = view.state.doc.length;
	const first = view.lineBlockAt(Math.min(change.from, length));
	const last = view.lineBlockAt(Math.min(Math.max(change.from, change.to), length));
	const content = view.contentDOM.getBoundingClientRect();
	const top = view.documentTop + first.top;
	return new DOMRect(content.left, top, view.contentDOM.clientWidth, Math.max(view.documentTop + last.bottom - top, 16));
}
