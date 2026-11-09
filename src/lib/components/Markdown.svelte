<script lang="ts">
	import { onDestroy } from 'svelte';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { goto } from '$app/navigation';
	import { locate, plain, rangeIn, textOf, type Mark } from '$lib/anchors';
	import { linkUrl, projectLinks } from '$lib/links.svelte';
	import { toast } from '$lib/ui.svelte';
	import { imageLoader, isVaultSrc, type ImageBase } from '$lib/vaultImages';

	/**
	 * `html` is rendered by the backend, which escapes raw HTML and drops unsafe link targets. With `images`, pictures
	 * in the vault are shown too, read from there. With `marks`, passages of the text are lit up (feedback on a
	 * selection), found again by their words each time the page or the list changes; `onmark` hears a click on one.
	 */
	let { html, images, marks, onmark }: { html: string; images?: ImageBase; marks?: Mark[]; onmark?: (id: string) => void } = $props();

	let root: HTMLDivElement;
	const pictures = imageLoader(() => images);
	onDestroy(pictures.dispose);

	// The backend writes `<img src="chart.png">`, which the window would look for beside the app: a vault picture waits
	// in data-src until it has been read.
	const shown = $derived(images ? html.replace(/<img src="([^"]*)"/g, (img, src) => (isVaultSrc(src) ? `<img data-src="${src}"` : img)) : html);

	$effect(() => {
		void shown;
		for (const img of root.querySelectorAll<HTMLImageElement>('img[data-src]')) {
			pictures.load(img.dataset.src ?? '').then((url) => {
				if (url) img.src = url;
				else img.replaceWith(Object.assign(document.createElement('span'), { className: 'missing', textContent: `Couldn’t load ${img.alt || img.dataset.src}` }));
			});
		}
	});

	// ---- [[links]] -------------------------------------------------------------------------------
	// The backend leaves `[[target]]` as <a class="wikilink" data-wikilink="target"> with no address. What the target is
	// (a thread, a deliverable, a piece of feedback) is looked up here, in the same list the editor completes from; one
	// that is nothing is drawn faint, as in Obsidian.

	const links = projectLinks();

	$effect(() => {
		void shown;
		const index = links.index;
		const project = links.project;
		for (const a of root.querySelectorAll<HTMLAnchorElement>('a.wikilink')) {
			const name = a.dataset.wikilink ?? '';
			const target = index?.resolve(name) ?? null;
			a.classList.toggle('unresolved', !target);
			if (!target || !project) {
				a.removeAttribute('href');
				a.title = index ? `Nothing in this project is called “${name}”` : 'Links work inside a project';
				continue;
			}
			a.href = linkUrl(project, target);
			a.title = target.kind === 'feedback' ? `Feedback ${target.detail}` : target.kind === 'thread' ? 'Thread' : `Deliverable ${target.id}`;
			// Feedback has no name to type: without words of its own, the link says what the feedback says.
			if (target.kind === 'feedback' && !a.dataset.alias) a.textContent = target.title;
		}
	});

	// ---- lit-up passages -----------------------------------------------------------------------
	// Painted with the CSS Custom Highlight API, which marks ranges of text without touching the page's elements (the
	// page is replaced whole whenever `html` changes). Without it, the passages are simply not lit.

	const TONES = ['open', 'resolved', 'new'] as const;
	const canHighlight = typeof CSS !== 'undefined' && 'highlights' in CSS;
	/** Where each mark is on the page now. */
	let placed = new Map<string, Range>();
	let painted = false;

	function clear() {
		if (!painted) return;
		for (const tone of TONES) CSS.highlights.delete(`fb-${tone}`);
		painted = false;
	}

	$effect(() => {
		void shown;
		const list = marks;
		placed = new Map();
		if (!canHighlight) return;
		if (!list?.length) return clear(); // a page without marks leaves the highlights of another alone
		const text = plain(textOf(root), false);
		const ranges: Record<Mark['tone'], Range[]> = { open: [], resolved: [], new: [] };
		for (const mark of list) {
			const at = locate(text, mark.anchor);
			const range = at && rangeIn(root, at.from, at.to);
			if (!range) continue;
			placed.set(mark.id, range);
			ranges[mark.tone].push(range);
		}
		for (const tone of TONES) {
			if (ranges[tone].length) CSS.highlights.set(`fb-${tone}`, new Highlight(...ranges[tone]));
			else CSS.highlights.delete(`fb-${tone}`);
		}
		painted = true;
	});
	onDestroy(clear);

	/** Scrolls the passage of mark `id` into view; false when it is not on the page. */
	export function reveal(id: string): boolean {
		const range = placed.get(id);
		if (!range) return false;
		range.startContainer.parentElement?.scrollIntoView({ block: 'center', behavior: 'smooth' });
		return true;
	}

	/** The text position under the pointer, wherever the engine offers it. */
	function pointAt(x: number, y: number): { node: Node; offset: number } | null {
		const doc = document as Document & { caretRangeFromPoint?: (x: number, y: number) => Range | null };
		const range = doc.caretRangeFromPoint?.(x, y);
		if (range) return { node: range.startContainer, offset: range.startOffset };
		const caret = document.caretPositionFromPoint?.(x, y);
		return caret ? { node: caret.offsetNode, offset: caret.offset } : null;
	}

	// Links open in the system browser; the app window itself never navigates away. A click on a lit-up passage (not the
	// end of a selection) is reported.
	function onclick(e: MouseEvent) {
		const anchor = (e.target as HTMLElement).closest('a');
		if (anchor) {
			e.preventDefault();
			const href = anchor.getAttribute('href') ?? '';
			if (anchor.classList.contains('wikilink')) {
				if (href) void goto(href);
				else toast(anchor.title);
				return;
			}
			if (/^(https?:|mailto:)/i.test(href)) openUrl(href).catch(() => {});
			return;
		}
		if (!onmark || !placed.size || !getSelection()?.isCollapsed) return;
		const at = pointAt(e.clientX, e.clientY);
		const hit = at && [...placed].find(([id, range]) => id !== 'new' && range.isPointInRange(at.node, at.offset));
		if (hit) onmark(hit[0]);
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="md" {onclick} bind:this={root}>{@html shown}</div>
