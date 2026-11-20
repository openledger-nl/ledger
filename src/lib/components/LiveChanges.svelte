<script lang="ts">
	import { untrack } from 'svelte';
	import Check from '@lucide/svelte/icons/check';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronUp from '@lucide/svelte/icons/chevron-up';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import { authorOf } from '$lib/editorChanges';
	import { CHANGE_KIND, readable, type ChangeEditor, type ChangeTracker, type TrackedVersion } from '$lib/liveChanges.svelte';

	/**
	 * The changes in the text being written, as the text shows them (the bars in the editor's margin are editorChanges.ts;
	 * the list and the settings are in the side window, ChangesPanel.svelte): the card of a change (a click on its bar),
	 * with what it replaced and Undo; after a version AI made, a line offering to go through its changes, and while doing
	 * that, a small bar at the bottom of the window. It also asks the backend for the changes a moment after typing stops.
	 */
	let {
		tracker,
		project,
		path,
		text,
		enabled,
		versions,
		editor
	}: {
		tracker: ChangeTracker;
		project: string;
		/** The live file's path. */
		path: string;
		/** The text as it is in the editor now. */
		text: string;
		/** A live markdown file to follow. */
		enabled: boolean;
		versions: TrackedVersion[];
		editor: ChangeEditor | undefined;
	} = $props();

	$effect.pre(() => {
		tracker.project = project;
		tracker.text = text;
		tracker.enabled = enabled;
		tracker.versions = versions;
		tracker.editor = editor;
	});

	$effect.pre(() => {
		tracker.path = path;
		untrack(() => tracker.reset());
	});

	// The changes, a moment after the text rests.
	$effect(() => {
		const [t, since, file, on, count] = [text, tracker.against, path, enabled, versions.length];
		if (!on || !count) {
			tracker.data = null;
			return;
		}
		const timer = setTimeout(
			() => {
				api.deliverableLiveChanges(project, file, t, since || null).then(
					(got) => {
						if (t === tracker.text && since === tracker.against && file === tracker.path) tracker.data = got;
					},
					() => {
						// A version that is gone (renamed, or deleted by hand): back to the newest.
						if (since) tracker.against = '';
					}
				);
			},
			untrack(() => tracker.data) ? 350 : 0
		);
		return () => clearTimeout(timer);
	});

	// After a version AI made: how many passages it changed, for the offer.
	$effect(() => {
		const v = tracker.toReview;
		const since = tracker.before?.version;
		tracker.offered = 0;
		if (!v || !since || !enabled) return;
		let stale = false;
		api
			.deliverableLiveChanges(project, path, untrack(() => text), since)
			.then((got) => {
				if (!stale) tracker.offered = got.changes.filter((c) => c.by !== null && got.sources[c.by]?.version === v.version).length;
			})
			.catch(() => {});
		return () => (stale = true);
	});

	// Once the comparison with the version before it has come, the review opens its first change; with none left, it is done.
	let started = false;
	$effect(() => {
		if (!tracker.reviewing) {
			started = false;
			return;
		}
		if (tracker.data?.against !== tracker.before?.version) return;
		const queue = tracker.queue;
		untrack(() => {
			if (!queue.length) tracker.finish(true);
			else if (!started) {
				started = true;
				tracker.show(queue[0].id);
			}
		});
	});

	// The change has gone (undone, or typed away): so has its card.
	$effect(() => {
		if (tracker.popover && tracker.data && !tracker.current) tracker.close();
	});

	let card: HTMLElement | undefined = $state();
	let cardHeight = $state(0);

	function onkey(e: KeyboardEvent) {
		if (e.key === 'Escape' && tracker.popover) {
			tracker.close();
			e.stopPropagation();
		}
	}

	function onpointer(e: PointerEvent) {
		const target = e.target as HTMLElement | null;
		if (!tracker.popover || card?.contains(target) || target?.closest('[data-change]') || target?.closest('.lc-review, .cp')) return;
		tracker.close();
	}

	const placed = $derived.by(() => {
		const popover = tracker.popover;
		if (!popover) return { top: 0, left: 0 };
		const width = 380;
		const left = Math.max(12, Math.min(popover.rect.left, window.innerWidth - width - 16));
		const below = popover.rect.bottom + 8;
		const top = below + cardHeight > window.innerHeight - 12 ? Math.max(12, popover.rect.top - cardHeight - 8) : below;
		return { top, left };
	});

	const words = (n: number) => `${n} ${n === 1 ? 'word' : 'words'}`;
</script>

<svelte:window onkeydown={onkey} onpointerdown={onpointer} onscrollcapture={() => tracker.follow()} onresize={() => tracker.follow()} />

{#if enabled && tracker.toReview && tracker.offered && !tracker.reviewing}
	{@const v = tracker.toReview}
	<div class="lc-offer">
		<Sparkles size={15} />
		<span><strong>{tracker.reviewedBy}</strong> changed {tracker.offered} {tracker.offered === 1 ? 'passage' : 'passages'} in v{v.version}.</span>
		<span class="spacer-h"></span>
		<button class="btn ghost small" onclick={() => tracker.markReviewed(v.version)}>Dismiss</button>
		<button class="btn primary small" onclick={() => tracker.startReview()}>Review</button>
	</div>
{/if}

{#if tracker.reviewing && tracker.toReview}
	<div class="lc-review" role="status">
		<Sparkles size={14} />
		<span>Reviewing {tracker.reviewedBy}'s changes in v{tracker.toReview.version}</span>
		<span class="lc-left">{tracker.queue.length} left</span>
		<button class="btn ghost small icon-only" title="Previous change" aria-label="Previous change" onclick={() => tracker.step(-1)}><ChevronUp size={15} /></button>
		<button class="btn ghost small icon-only" title="Next change" aria-label="Next change" onclick={() => tracker.step(1)}><ChevronDown size={15} /></button>
		<button class="btn ghost small" onclick={() => tracker.finish(true)}>Keep the rest</button>
		<button class="btn ghost small" onclick={() => tracker.finish(false)}>Stop</button>
	</div>
{/if}

{#if tracker.popover && tracker.current}
	{@const c = tracker.current}
	{@const since = tracker.data?.against}
	<div class="lc-card" style:top="{placed.top}px" style:left="{placed.left}px" bind:this={card} bind:clientHeight={cardHeight} role="dialog" aria-label="Change">
		<div class="lc-head">
			<span class="lc-dot" data-by={authorOf(c, tracker.sources)}></span>
			<strong>{CHANGE_KIND[c.kind]}</strong>
			<span class="lc-who">{tracker.whoMade(c)}</span>
			<span class="spacer-h"></span>
			{#if tracker.reviewing}<span class="lc-left">{tracker.queue.findIndex((q) => q.id === c.id) + 1} of {tracker.queue.length}</span>{/if}
			<button class="lc-x" title="Close" aria-label="Close" onclick={() => tracker.close()}><X size={14} /></button>
		</div>
		{#if c.kind === 'added'}
			<p class="lc-note">New since v{since}.</p>
		{:else if c.kind === 'moved'}
			<p class="lc-note">This passage was somewhere else in v{since}{c.wordsAdded || c.wordsRemoved ? ', and changed on the way' : ''}.</p>
		{:else if c.old.trim()}
			<div class="lc-label">{c.kind === 'removed' ? 'What was here' : `Before, in v${since}`}</div>
			<div class="lc-old" class:gone={c.kind === 'removed'}>{readable(c.old)}</div>
		{/if}
		{#if c.wordsAdded || c.wordsRemoved}
			<div class="lc-words">
				{#if c.wordsAdded}<span class="plus">+{words(c.wordsAdded)}</span>{/if}
				{#if c.wordsRemoved}<span class="minus">-{words(c.wordsRemoved)}</span>{/if}
			</div>
		{/if}
		<div class="lc-actions">
			{#if tracker.reviewing}
				<button class="btn ghost small icon-only" onclick={() => tracker.step(-1)} title="Previous change" aria-label="Previous change"><ChevronUp size={14} /></button>
				<button class="btn ghost small icon-only" onclick={() => tracker.step(1)} title="Next change" aria-label="Next change"><ChevronDown size={14} /></button>
				<span class="spacer-h"></span>
				<button class="btn small" onclick={() => tracker.undo(c)}><Undo2 size={14} />Undo</button>
				<button class="btn primary small" onclick={() => tracker.keep(c)}><Check size={14} />Keep</button>
			{:else}
				<button class="btn small" onclick={() => tracker.undo(c)} title="Put back what v{since} said here"><Undo2 size={14} />Undo this change</button>
				<span class="spacer-h"></span>
				{#if tracker.changes.length > 1}
					<button class="btn ghost small icon-only" onclick={() => tracker.step(-1)} title="Previous change" aria-label="Previous change"><ChevronUp size={14} /></button>
					<button class="btn ghost small icon-only" onclick={() => tracker.step(1)} title="Next change" aria-label="Next change"><ChevronDown size={14} /></button>
				{/if}
			{/if}
		</div>
	</div>
{/if}

<style>
	/* The offer to go through what AI changed: one quiet line above the text, in the AI colour. */
	.lc-offer {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 8px 6px 12px;
		margin-bottom: 10px;
		border-radius: 10px;
		background: var(--accent-soft);
		color: var(--ink);
		font-size: 13px;
	}
	.lc-offer :global(svg) {
		color: var(--accent);
		flex: none;
	}

	/* While reviewing: a small bar at the bottom of the window, out of the text's way. */
	.lc-review {
		position: fixed;
		left: 50%;
		bottom: 22px;
		transform: translateX(-50%);
		z-index: 40;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 8px 6px 14px;
		border-radius: 12px;
		background: var(--card);
		border: 1px solid var(--line);
		box-shadow: var(--shadow-toast);
		font-size: 13px;
		white-space: nowrap;
	}
	.lc-review :global(svg) {
		color: var(--accent);
	}
	.lc-left {
		color: var(--muted);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
	}
	.lc-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--green);
		flex: none;
	}
	.lc-dot[data-by='ai'] {
		background: var(--accent);
	}
	.lc-dot[data-by='unknown'] {
		background: var(--faint);
	}
	.lc-dot[data-by='gone'] {
		background: var(--red);
	}

	/* What a change replaced, under the passage, like a comment card. */
	.lc-card {
		position: fixed;
		z-index: 45;
		width: 380px;
		max-width: calc(100vw - 32px);
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 10px 12px;
		border-radius: 12px;
		background: var(--card);
		border: 1px solid var(--line);
		box-shadow: var(--shadow-toast);
		font-size: 13px;
		animation: lc-in 120ms var(--ease-out);
	}
	@keyframes lc-in {
		from {
			opacity: 0;
			transform: translateY(-4px);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.lc-card {
			animation: none;
		}
	}
	.lc-head {
		display: flex;
		align-items: center;
		gap: 7px;
	}
	.lc-who {
		color: var(--muted);
		font-size: 12px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.lc-x {
		display: inline-grid;
		place-items: center;
		width: 22px;
		height: 22px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--faint);
		cursor: pointer;
	}
	.lc-x:hover {
		background: var(--card-2, var(--bg));
		color: var(--ink);
	}
	.lc-label {
		font-size: 12px;
		color: var(--faint);
	}
	.lc-old {
		max-height: 168px;
		overflow: auto;
		white-space: pre-wrap;
		padding: 8px 10px;
		border-radius: 8px;
		background: var(--field);
		border: 1px solid var(--line);
		color: var(--muted);
		line-height: 1.5;
	}
	.lc-old.gone {
		text-decoration: line-through;
		text-decoration-color: var(--red);
	}
	.lc-note {
		margin: 0;
		color: var(--muted);
	}
	.lc-words {
		display: flex;
		gap: 10px;
		font-size: 12px;
		font-weight: 600;
	}
	.lc-words .plus {
		color: var(--green);
	}
	.lc-words .minus {
		color: var(--red);
	}
	.lc-actions {
		display: flex;
		align-items: center;
		gap: 6px;
	}
</style>
