<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { EditorState, type Extension } from '@codemirror/state';
	import { EditorView } from '@codemirror/view';
	import type { Mark } from '$lib/anchors';
	import type { EvidenceMention, LinkTarget } from '$lib/types';
	import { projectLinks } from '$lib/links.svelte';
	import { changeRect, changeTracking, revealChange, revertChange, showChanges, type ChangeView } from '$lib/editorChanges';
	import { markdownEditor, refreshLinks, revealMark, setSourceMode, showMarks, type MentionOption } from '$lib/markdownEditor';
	import { imageLoader, type ImageBase } from '$lib/vaultImages';

	/**
	 * Where markdown is written: CodeMirror on the markdown itself, styled like the rendered page (see markdownEditor.ts).
	 * `onsubmit` runs on Ctrl/⌘+Enter, or on Enter with `submitOnEnter`; without it Ctrl/⌘+Enter only bubbles up,
	 * to a modal's own listener. `oninput` is called for typing, not when `value` is changed from outside.
	 */
	let {
		value = $bindable(''),
		placeholder = '',
		label,
		variant = 'field',
		rows = 3,
		minHeight,
		autofocus = false,
		spellcheck = true,
		source = false,
		images,
		submitOnEnter = false,
		onsubmit,
		oncancel,
		oninput,
		marks,
		onmark,
		mentions,
		people,
		linkTargets,
		linkHome = null,
		changes,
		onchangeclick
	}: {
		value?: string;
		placeholder?: string;
		label?: string;
		/** `field` in forms and cards, `doc` for whole documents, `composer` for the thread's note box. */
		variant?: 'field' | 'doc' | 'composer';
		/** The height it starts at, in lines; `minHeight` (any CSS length) overrides it. */
		rows?: number;
		minHeight?: string;
		autofocus?: boolean;
		spellcheck?: boolean;
		/** Source mode: the raw markdown, every mark shown, instead of the live preview. */
		source?: boolean;
		/** Where pictures in the vault are read from; without it, only web pictures are shown. */
		images?: ImageBase;
		submitOnEnter?: boolean;
		onsubmit?: () => void;
		/** Escape. */
		oncancel?: () => void;
		oninput?: () => void;
		/** Passages to light up (feedback on a selection), found again by their words as the text changes. */
		marks?: Mark[];
		/** A click on one of them. */
		onmark?: (id: string) => void;
		/** What typing `@` offers: a Portflow reading guide's evidence items. */
		mentions?: EvidenceMention[];
		/** Or the people `@` offers (a meeting's), each written as a link to them (`[[people/frank]]`). */
		people?: MentionOption[];
		/** What [[links]] point at here, instead of the project the window is in: a meeting's. */
		linkTargets?: LinkTarget[];
		/** The project those links are written in (a meeting's only project); null for none. */
		linkHome?: string | null;
		/** The changes since a version, in the margin (and in the text with `inline`); see editorChanges.ts. */
		changes?: ChangeView | null;
		/** A click on a change's bar or mark. Given, the editor has a margin for the changes. */
		onchangeclick?: (id: string, rect: DOMRect) => void;
	} = $props();

	// [[links]] are read from the project the window is in, or from `linkHome` among `linkTargets` when given (none
	// outside a project otherwise).
	const links = projectLinks(() => (linkTargets ? { targets: linkTargets, home: linkHome } : undefined));

	let host: HTMLDivElement;
	let view: EditorView | undefined;
	let extensions: Extension;
	/** The text the editor holds, to tell its own changes from changes made to `value` outside. */
	let held = '';
	let focusWhenReady = false;

	onMount(() => {
		held = value;
		const pictures = imageLoader(() => images);
		const editor = markdownEditor({
			placeholder,
			label,
			spellcheck,
			source,
			loadImage: images ? pictures.load : undefined,
			submitOnEnter,
			tabIndents: variant === 'doc',
			submit: () => onsubmit?.(),
			cancel: () => {
				if (!oncancel) return false;
				oncancel();
				return true;
			},
			markClicked: (id) => onmark?.(id),
			links: { index: () => links.index, open: (target) => links.open(target) },
			mentions: () =>
				people ?? (mentions ?? []).map((m) => ({ id: `evidence-${m.portflowId}`, label: m.name, detail: m.deliverable ? 'Deliverable' : (m.threadTitle ?? 'Not in Ledger yet') })),
			changed: (update) => {
				if (!update.docChanged) return;
				held = update.state.doc.toString();
				value = held;
				oninput?.();
			}
		});
		extensions = onchangeclick ? [editor, changeTracking((id, rect) => onchangeclick?.(id, rect))] : editor;
		view = new EditorView({ parent: host, state: EditorState.create({ doc: value, extensions }) });
		pushMarks(marks ?? []);
		pushChanges(untrack(() => changes));
		if (autofocus || focusWhenReady) focus(autofocus);
		return () => {
			view?.destroy();
			pictures.dispose();
		};
	});

	// Changed outside (a note sent, a file reloaded or another one opened): start afresh, so undo can't bring back
	// the text that was there before.
	$effect(() => {
		const next = value;
		if (!view || next === held) return;
		held = next;
		const head = Math.min(view.state.selection.main.head, next.length);
		view.setState(EditorState.create({ doc: next, selection: { anchor: head }, extensions }));
		setSourceMode(view, untrack(() => source));
		pushMarks(untrack(() => marks) ?? []);
		pushChanges(untrack(() => changes));
	});

	$effect(() => pushChanges(changes));

	function pushChanges(next: ChangeView | null | undefined) {
		if (view && onchangeclick) showChanges(view, next ?? { data: null, inline: false, focus: null });
	}

	/** Undoes one change: puts back what the version said there. */
	export function revert(id: string): boolean {
		return view ? revertChange(view, id) : false;
	}

	/** Scrolls a change into view, and says where it then is on the screen. */
	export function showChange(id: string): DOMRect | null {
		if (!view) return null;
		revealChange(view, id);
		return changeRect(view, id);
	}

	/** Where a change is on the screen. */
	export function rectOf(id: string): DOMRect | null {
		return view ? changeRect(view, id) : null;
	}

	$effect(() => pushMarks(marks ?? []));

	// The things links point at changed (something was made, renamed or deleted): they are looked at again.
	$effect(() => {
		void links.index;
		if (view) refreshLinks(view);
	});

	$effect(() => {
		const next = source;
		if (view) setSourceMode(view, next);
	});

	let lit = false;
	/** Hands the passages to the editor; an editor that has never had any isn't bothered. */
	function pushMarks(next: Mark[]) {
		if (!view || (!next.length && !lit)) return;
		lit = next.length > 0;
		showMarks(view, next);
	}

	/** Scrolls the passage of mark `id` into view; false when it is not in the text. */
	export function reveal(id: string): boolean {
		return view ? revealMark(view, id) : false;
	}

	/** Focuses the editor, now or as soon as it exists; `atEnd` puts the cursor after the text. */
	export function focus(atEnd = false) {
		if (!view) {
			focusWhenReady = true;
			return;
		}
		view.focus();
		if (atEnd) view.dispatch({ selection: { anchor: view.state.doc.length }, scrollIntoView: true });
	}
</script>

<div class="md-editor md-{variant}" style:--md-min-height={minHeight ?? `calc(${rows} * 1.55em + 2 * var(--md-pad-y))`} bind:this={host}></div>
