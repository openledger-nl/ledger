<script lang="ts">
	import { EditorView } from '@codemirror/view';
	import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
	import { anchorOf, MAX_QUOTE, offsetIn, textOf, type Anchor } from '$lib/anchors';

	// A comment icon just after the text selected in the document, in any mode: the markdown itself (source), the live
	// preview, or the rendered page (reading). Clicking it hands the selected passage to `oncomment`, which is where the
	// feedback gets written. The selection itself is left alone, so what is commented on stays in sight.

	let { root, oncomment }: { root: HTMLElement | undefined; oncomment: (anchor: Anchor) => void } = $props();

	const GAP = 6;
	const SIZE = 28;

	let target = $state<{ anchor: Anchor; x: number; y: number } | null>(null);
	/** While the pointer is down the selection is still being made. */
	let dragging = false;
	let frame = 0;

	/** The selection inside the document, as a passage and the place just after it; null when there is none. */
	function read(): { anchor: Anchor; x: number; y: number } | null {
		const selection = getSelection();
		if (!root || !selection || selection.isCollapsed || !selection.rangeCount) return null;
		const range = selection.getRangeAt(0);
		const common = range.commonAncestorContainer;
		const inside = common instanceof Element ? common : common.parentElement;
		if (!inside || !root.contains(inside)) return null;

		const editor = inside.closest<HTMLElement>('.cm-editor');
		if (editor) {
			// The editor knows the selection in the markdown, marks and all; the page's own selection may be cut by hidden marks.
			const view = EditorView.findFromDOM(editor);
			const { from, to } = view?.state.selection.main ?? { from: 0, to: 0 };
			if (!view || from === to) return null;
			const anchor = anchorOf(view.state.doc.toString(), from, to, true);
			const end = view.coordsAtPos(to, -1);
			return anchor && end ? { anchor, x: end.right, y: (end.top + end.bottom) / 2 } : null;
		}

		const page = inside.closest<HTMLElement>('.doc-view');
		if (!page) return null;
		const anchor = anchorOf(textOf(page), offsetIn(page, range.startContainer, range.startOffset), offsetIn(page, range.endContainer, range.endOffset), false);
		const rects = range.getClientRects();
		const end = rects[rects.length - 1];
		return anchor && end ? { anchor, x: end.right, y: (end.top + end.bottom) / 2 } : null;
	}

	function update() {
		cancelAnimationFrame(frame);
		frame = requestAnimationFrame(() => (target = dragging ? null : read()));
	}

	$effect(() => {
		const down = (e: PointerEvent) => {
			if ((e.target as HTMLElement | null)?.closest('.sel-comment')) return;
			dragging = true;
			target = null;
		};
		const up = () => {
			dragging = false;
			setTimeout(update); // the selection settles after the pointer is up
		};
		document.addEventListener('selectionchange', update);
		document.addEventListener('pointerdown', down, true);
		document.addEventListener('pointerup', up, true);
		document.addEventListener('scroll', update, true);
		window.addEventListener('resize', update);
		return () => {
			cancelAnimationFrame(frame);
			document.removeEventListener('selectionchange', update);
			document.removeEventListener('pointerdown', down, true);
			document.removeEventListener('pointerup', up, true);
			document.removeEventListener('scroll', update, true);
			window.removeEventListener('resize', update);
		};
	});

	const tooLong = $derived((target?.anchor.text.length ?? 0) > MAX_QUOTE);

	function comment() {
		if (!target || tooLong) return;
		oncomment(target.anchor);
		target = null;
	}
</script>

{#if target}
	<!-- Pressing it must not take the selection away from the text. -->
	<button
		class="sel-comment"
		disabled={tooLong}
		style="left: {Math.max(8, Math.min(target.x + GAP, window.innerWidth - SIZE - 8))}px; top: {Math.max(8, target.y - SIZE / 2)}px"
		title={tooLong ? `Select a shorter passage (up to ${MAX_QUOTE} characters)` : 'Comment on the selected text'}
		aria-label="Comment on the selected text"
		onpointerdown={(e) => e.preventDefault()}
		onclick={comment}
	>
		<MessageSquarePlus size={15} />
	</button>
{/if}
