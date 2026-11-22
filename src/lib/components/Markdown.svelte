<script lang="ts">
	import { onDestroy } from 'svelte';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { goto } from '$app/navigation';
	import { locate, plain, rangeIn, textOf, type Mark } from '$lib/anchors';
	import { deliverableUrl, threadUrl } from '$lib/actions';
	import { linkUrl, projectLinks } from '$lib/links.svelte';
	import { openInPortflow } from '$lib/portflow.svelte';
	import type { EvidenceMention } from '$lib/types';
	import { toast } from '$lib/ui.svelte';
	import { imageLoader, isVaultSrc, type ImageBase } from '$lib/vaultImages';

	/**
	 * `html` is rendered by the backend, which escapes raw HTML and drops unsafe link targets. With `images`, pictures
	 * in the vault are shown too, read from there. With `marks`, passages of the text are lit up (feedback on a
	 * selection), found again by their words each time the page or the list changes; `onmark` hears a click on one.
	 */
	let {
		html,
		images,
		marks,
		onmark,
		mentions
	}: {
		html: string;
		images?: ImageBase;
		marks?: Mark[];
		onmark?: (id: string) => void;
		/** The evidence items a Portflow reading guide's `@` mentions can be: a mention of one leads to its deliverable. */
		mentions?: EvidenceMention[];
	} = $props();

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
				else img.replaceWith(Object.assign(document.createElement('span'), { className: 'missing', textContent: `Couldn't load ${img.alt || img.dataset.src}` }));
			});
		}
	});

	// ---- [[links]] -------------------------------------------------------------------------------
	// The backend leaves `[[target]]` as <a class="wikilink" data-wikilink="target"> with no address. What the target is
	// (a thread, a deliverable, a piece of feedback, a meeting day, a person) is looked up here, in the same list the
	// editor completes from; one that is nothing is drawn faint, as in Obsidian.

	const links = projectLinks();

	$effect(() => {
		void shown;
		const index = links.index;
		for (const a of root.querySelectorAll<HTMLAnchorElement>('a.wikilink')) {
			const name = a.dataset.wikilink ?? '';
			const target = index?.resolve(name) ?? null;
			a.classList.toggle('unresolved', !target);
			if (!target) {
				a.removeAttribute('href');
				a.title = index ? `Nothing is called "${name}"` : 'Links work inside a project';
				continue;
			}
			a.href = linkUrl(target);
			a.title =
				target.kind === 'feedback'
					? `Feedback ${target.detail}`
					: target.kind === 'thread'
						? 'Thread'
						: target.kind === 'meeting'
							? `Meeting notes of ${target.id}`
							: target.kind === 'person'
								? `${target.title}${target.detail ? `, ${target.detail}` : ''}`
								: `Deliverable ${target.id}${target.project !== links.project ? ` in ${target.project}` : ''}`;
			// Feedback has no name to type, and a person is written by id: without words of their own, the link says what
			// the feedback says, or the person's name.
			if ((target.kind === 'feedback' || target.kind === 'person') && !a.dataset.alias) a.textContent = target.title;
		}
	});

	// ---- @ mentions --------------------------------------------------------------------------------
	// A Portflow mention (`[@Analysis](portflow:evidence-2110705)`) arrives as <a class="mention" data-mention="…">. One
	// of an evidence item leads to its deliverable (or, until that's in Ledger, its thread); any other opens Portflow.

	/** The number at the end of a mention's id: `evidence-2110705` → 2110705. */
	const numberOf = (id: string) => Number(/(\d+)$/.exec(id)?.[1] ?? NaN);

	$effect(() => {
		void shown;
		const project = links.project;
		for (const a of root.querySelectorAll<HTMLAnchorElement>('a.mention')) {
			// A person mentioned in a meeting's notes: the feedback they gave.
			if (a.dataset.person) {
				a.href = `/feedback?from=${encodeURIComponent(a.dataset.person)}`;
				a.title = `The feedback ${a.textContent?.replace(/^@/, '') ?? ''} gave`;
				continue;
			}
			const id = a.dataset.mention ?? '';
			const item = a.dataset.mentionType === 'evidence' || id.startsWith('evidence') ? mentions?.find((m) => m.portflowId === numberOf(id)) : undefined;
			if (item?.deliverable && project) {
				a.href = deliverableUrl(project, item.deliverable);
				a.title = `${item.name}: open the deliverable`;
			} else if (item?.thread && project) {
				a.href = threadUrl(project, item.thread);
				a.title = `${item.name}: its thread "${item.threadTitle ?? item.thread}"`;
			} else {
				a.removeAttribute('href');
				a.title = 'In Portflow';
			}
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
			if (anchor.classList.contains('mention')) {
				if (href) void goto(href);
				else void openInPortflow(`"${(anchor.textContent ?? '').replace(/^@/, '')}"`);
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
