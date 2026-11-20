<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronUp from '@lucide/svelte/icons/chevron-up';
	import { api } from '$lib/api';
	import { authorLabel } from '$lib/mmm';
	import type { Authorship, Comparison, HistoryBlock, HistorySource, ProvenanceVersion } from '$lib/types';
	import type { ImageBase } from '$lib/vaultImages';
	import Dropdown from './Dropdown.svelte';
	import Markdown from './Markdown.svelte';

	/**
	 * What changed between two versions of a markdown deliverable (`view` "changes"), or who wrote what is in one (`view`
	 * "authors"). Both are worked out by the backend from the published versions (history.rs); this only shows them.
	 *
	 * `to` is the text shown (a version, or "live"); `from` the one it is compared with; `focus` a version whose words
	 * alone are lit up in "authors", to see where they ended up.
	 */
	let {
		project,
		path,
		view,
		versions,
		hasDraft,
		images,
		from = $bindable(),
		to = $bindable(),
		focus = $bindable()
	}: {
		project: string;
		/** The live file's path. */
		path: string;
		view: 'changes' | 'authors';
		/** The published versions, oldest first. */
		versions: Pick<ProvenanceVersion, 'version' | 'by' | 'run'>[];
		/** Whether the live file holds something no version does. */
		hasDraft: boolean;
		images?: ImageBase;
		from: string;
		to: string;
		focus: string;
	} = $props();

	const options = $derived([
		...versions.map((v) => ({ value: v.version, label: `v${v.version} | ${authorLabel(v)}` })),
		...(hasDraft ? [{ value: 'live', label: 'Live, not published yet' }] : [])
	]);

	// A version can only be lit up in a text that came after it.
	$effect(() => {
		const i = options.findIndex((o) => o.value === to);
		if (focus && options.findIndex((o) => o.value === focus) > i) focus = '';
	});

	// ---- loading -------------------------------------------------------------------------------------

	let comparison = $state<Comparison | null>(null);
	let authorship = $state<Authorship | null>(null);
	let error = $state('');
	let loading = $state(false);

	$effect(() => {
		const [v, f, t, fo] = [view, from, to, focus];
		if (!t || (v === 'changes' && (!f || f === t))) return;
		let stale = false;
		loading = true;
		error = '';
		const done = (e?: unknown) => {
			if (stale) return;
			loading = false;
			if (e) error = e instanceof Error ? e.message : String(e);
		};
		if (v === 'changes') {
			api.deliverableChanges(project, path, f, t).then((c) => !stale && ((comparison = c), done()), done);
		} else {
			api.deliverableAuthors(project, path, t, fo || null).then((a) => !stale && ((authorship = a), done()), done);
		}
		return () => (stale = true);
	});

	// ---- what is shown -------------------------------------------------------------------------------

	const sources = $derived((view === 'changes' ? comparison?.sources : authorship?.sources) ?? []);
	const kindOf = (i: number | null) => (i === null ? '' : (sources[i]?.by ?? ''));

	/** Changes only: the changed blocks and one unchanged block on either side, the rest folded into a line. */
	let onlyChanges = $state(false);
	/** Changes as the document reads (as in Word), or line by line (as in git). */
	let layout = $state<'document' | 'lines'>('document');

	function escape(text: string): string {
		return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
	}

	function blockHtml(b: HistoryBlock, i: number): string {
		const by = kindOf(b.by);
		const lit = view === 'authors' && focus && b.by !== null ? ' dv-lit' : '';
		const dim = view === 'authors' && focus && b.by === null ? ' dv-dim' : '';
		const tag =
			b.state === 'moved' ? '<span class="dv-tag">Moved here</span>' : b.state === 'movedAway' ? '<span class="dv-tag">A passage moved from here</span>' : '';
		return `<div class="dv-block dv-${b.state}${lit}${dim}" data-by="${by}" data-i="${i}">${tag}${b.html}</div>`;
	}

	const html = $derived.by(() => {
		// Where a few blocks in a row moved away from, one line says so.
		const blocks = ((view === 'changes' ? comparison?.blocks : authorship?.blocks) ?? []).filter(
			(b, i, all) => b.state !== 'movedAway' || all[i - 1]?.state !== 'movedAway'
		);
		if (view === 'authors' || !onlyChanges) return blocks.map(blockHtml).join('');
		const near = (i: number) => blocks[i] && blocks[i].state !== 'same';
		const out: string[] = [];
		let folded = 0;
		blocks.forEach((b, i) => {
			if (b.state !== 'same' || near(i - 1) || near(i + 1)) {
				if (folded) out.push(`<div class="dv-gap">${folded} unchanged ${folded === 1 ? 'paragraph' : 'paragraphs'}</div>`);
				folded = 0;
				out.push(blockHtml(b, i));
			} else folded++;
		});
		if (folded) out.push(`<div class="dv-gap">${folded} unchanged ${folded === 1 ? 'paragraph' : 'paragraphs'}</div>`);
		return out.length ? out.join('') : `<p class="hint">${escape('The two texts are the same.')}</p>`;
	});

	/** Words by who made them: You, Claude Code, AI as declared, not recorded. */
	function byWho(shares: { source: number; words: number }[], list: HistorySource[]) {
		const total = shares.reduce((n, s) => n + s.words, 0);
		const groups = new Map<string, { who: string; by: string; words: number }>();
		for (const s of shares) {
			const src = list[s.source];
			if (!src) continue;
			const g = groups.get(src.who) ?? { who: src.who, by: src.by, words: 0 };
			g.words += s.words;
			groups.set(src.who, g);
		}
		return [...groups.values()]
			.sort((a, b) => b.words - a.words)
			.map((g) => ({ ...g, share: total ? Math.round((g.words / total) * 100) : 0 }));
	}

	const shares = $derived(view === 'changes' ? byWho(comparison?.addedBy ?? [], sources) : byWho(authorship?.words ?? [], sources));

	const name = (i: number) => {
		const s = sources[i];
		return s ? (s.version ? `${s.who} in v${s.version}` : `${s.who}, not published yet`) : '';
	};
	const words = (n: number) => `${n.toLocaleString()} ${n === 1 ? 'word' : 'words'}`;

	// ---- stepping through the changes ------------------------------------------------------------------

	let body: HTMLElement | undefined = $state();
	let at = $state(-1);
	const stops = () =>
		[...(body?.querySelectorAll<HTMLElement>(view === 'changes' ? (layout === 'lines' ? '.dl-hunk' : '.dv-block:not(.dv-same)') : focus ? '.dv-lit' : '.dv-block:has(.dv-by[data-by="ai"])') ?? [])];
	let count = $state(0);
	$effect(() => {
		void html;
		void layout;
		void comparison;
		at = -1;
		queueMicrotask(() => (count = stops().length));
	});

	function step(by: number) {
		const all = stops();
		count = all.length;
		if (!all.length) return;
		at = (at + by + all.length) % all.length;
		const el = all[at];
		el.scrollIntoView({ block: 'center', behavior: 'smooth' });
		el.classList.remove('dv-flash');
		void el.offsetWidth;
		el.classList.add('dv-flash');
	}

	const stopName = $derived(view === 'changes' ? 'change' : focus ? 'passage' : 'AI passage');
</script>

<div class="dv">
	<div class="dv-bar">
		{#if view === 'changes'}
			<span class="dv-label">Compare</span>
			<Dropdown bind:value={from} options={options.filter((o) => o.value !== to)} label="Compare from" />
			<span class="dv-label">with</span>
			<Dropdown bind:value={to} options={options.filter((o) => o.value !== from)} label="Compare with" />
			<div class="seg" role="group" aria-label="Show the changes">
				<button class:on={layout === 'document'} aria-pressed={layout === 'document'} onclick={() => (layout = 'document')}>As a document</button>
				<button class:on={layout === 'lines'} aria-pressed={layout === 'lines'} onclick={() => (layout = 'lines')}>Line by line</button>
			</div>
			{#if layout === 'document'}
				<label class="check dv-check"><input type="checkbox" bind:checked={onlyChanges} /> Only the changes</label>
			{/if}
		{:else}
			<span class="dv-label">Who wrote</span>
			<Dropdown bind:value={to} options={options} label="The text shown" />
			<span class="dv-label">Light up</span>
			<Dropdown
				bind:value={focus}
				options={[
					{ value: '', label: 'Everything' },
					...versions.filter((v) => options.findIndex((o) => o.value === v.version) <= options.findIndex((o) => o.value === to)).map((v) => ({ value: v.version, label: `Only v${v.version} | ${authorLabel(v)}` }))
				]}
				label="Light up"
			/>
		{/if}
		<span class="spacer-h"></span>
		{#if count}
			<span class="dv-steps">
				<button class="btn ghost small icon-only" title="Previous {stopName}" aria-label="Previous {stopName}" onclick={() => step(-1)}><ChevronUp size={15} /></button>
				<span class="hint">{at < 0 ? count : `${at + 1} of ${count}`} {at < 0 ? `${stopName}${count === 1 ? '' : 's'}` : ''}</span>
				<button class="btn ghost small icon-only" title="Next {stopName}" aria-label="Next {stopName}" onclick={() => step(1)}><ChevronDown size={15} /></button>
			</span>
		{/if}
	</div>

	{#if error}
		<div class="notice bad">{error}</div>
	{:else if view === 'changes' && comparison}
		<div class="dv-sum">
			<span class="dv-plus">+{words(comparison.wordsAdded)}</span>
			<span class="dv-minus">-{words(comparison.wordsRemoved)}</span>
			{#if comparison.moved}<span>{comparison.moved} moved</span>{/if}
			{#if shares.length}
				<span class="dv-sep">|</span>
				<span>Added by</span>
				{#each shares as s (s.who)}
					<span class="dv-who" data-by={s.by}><span class="dv-dot"></span>{s.who} {s.share}%</span>
				{/each}
			{/if}
		</div>
	{:else if view === 'authors' && authorship}
		<div class="dv-sum">
			{#if authorship.trace}
				{@const t = authorship.trace}
				<span>
					{name(t.source)} wrote {words(t.wrote)}. <strong>{t.kept.toLocaleString()}</strong> ({t.wrote ? Math.round((t.kept / t.wrote) * 100) : 0}%) {t.kept === 1 ? 'is' : 'are'} still here{#if t.removedBy.length}; taken out since: {t.removedBy.map((r) => `${r.words.toLocaleString()} by ${name(r.source)}`).join(', ')}{/if}.
				</span>
			{:else}
				<span class="dv-bar-chart" aria-hidden="true">
					{#each shares as s (s.who)}<span class="dv-part" data-by={s.by} style="flex:{s.words}"></span>{/each}
				</span>
				{#each shares as s (s.who)}
					<span class="dv-who" data-by={s.by}><span class="dv-dot"></span>{s.who} {s.share}%</span>
				{/each}
			{/if}
		</div>
	{/if}

	<div class="dv-body" class:busy={loading} bind:this={body}>
		{#if view === 'changes' && comparison && layout === 'lines'}
			{#if !comparison.hunks.length}
				<p class="hint">The two texts are the same.</p>
			{/if}
			{#each comparison.hunks as hunk, h (h)}
				<div class="dl-hunk">
					<div class="dl-head mono">Line {hunk.newStart}</div>
					{#each hunk.lines as line, l (l)}
						<div class="dl-line dl-{line.tag} mono" data-by={line.tag === 'add' ? kindOf(line.by) : ''} title={line.tag === 'add' && line.by !== null ? name(line.by) : undefined}>
							<span class="dl-no">{line.old ?? ''}</span><span class="dl-no">{line.new ?? ''}</span><span class="dl-sign">{line.tag === 'add' ? '+' : line.tag === 'del' ? '-' : ''}</span><span class="dl-text">{#each line.parts as part, p (p)}{#if part.emph}<span class="dl-emph">{part.text}</span>{:else}{part.text}{/if}{/each}</span>
						</div>
					{/each}
				</div>
			{/each}
		{:else if (view === 'changes' && comparison) || (view === 'authors' && authorship)}
			<div class="doc-view dv-doc" class:dv-authors={view === 'authors'}><Markdown {html} {images} /></div>
		{:else if !error}
			<p class="hint">Working it out...</p>
		{/if}
	</div>
</div>

<style>
	.dv {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.dv-bar {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 8px;
	}
	.dv-label {
		color: var(--muted);
		font-size: 13px;
	}
	.dv-check {
		font-size: 13px;
	}
	.dv-steps {
		display: inline-flex;
		align-items: center;
		gap: 2px;
	}
	.dv-sum {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px 12px;
		font-size: 13px;
		color: var(--muted);
	}
	.dv-plus {
		color: var(--green);
		font-weight: 600;
	}
	.dv-minus {
		color: var(--red);
		font-weight: 600;
	}
	.dv-sep {
		color: var(--faint);
	}
	.dv-who {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}
	.dv-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--green);
	}
	.dv-who[data-by='ai'] .dv-dot {
		background: var(--accent);
	}
	.dv-who[data-by='unknown'] .dv-dot {
		background: var(--faint);
	}
	.dv-bar-chart {
		display: inline-flex;
		width: 120px;
		height: 6px;
		border-radius: 3px;
		overflow: hidden;
		background: var(--line);
	}
	.dv-part {
		background: var(--green);
	}
	.dv-part[data-by='ai'] {
		background: var(--accent);
	}
	.dv-part[data-by='unknown'] {
		background: var(--faint);
	}
	.dv-body.busy {
		opacity: 0.6;
	}

	/* ---- as a document: Word's track changes, coloured by who made them ---- */
	.dv-doc :global(.dv-block) {
		position: relative;
		padding-left: 12px;
		border-left: 2px solid transparent;
		border-radius: 2px;
		transition: background 0.4s;
	}
	.dv-doc :global(.dv-block:not(.dv-same)) {
		border-left-color: var(--green);
	}
	.dv-doc :global(.dv-block[data-by='ai']:not(.dv-same)) {
		border-left-color: var(--accent);
	}
	.dv-doc :global(.dv-block.dv-removed) {
		border-left-color: var(--red);
		color: var(--muted);
		text-decoration: line-through;
		text-decoration-color: var(--red);
	}
	.dv-doc :global(.dv-block.dv-moved),
	.dv-doc :global(.dv-block.dv-movedAway) {
		border-left-style: dashed;
	}
	.dv-doc :global(.dv-block.dv-movedAway) {
		border-left-color: var(--faint);
		padding: 2px 0 6px 12px;
	}
	.dv-doc :global(.dv-tag) {
		display: block;
		font-size: 11px;
		color: var(--faint);
		margin-bottom: 2px;
	}
	.dv-doc :global(ins.dv-ins) {
		text-decoration: none;
		background: var(--green-soft);
		border-bottom: 1px solid var(--green);
	}
	.dv-doc :global(ins.dv-ins[data-by='ai']) {
		background: var(--accent-soft);
		border-bottom-color: var(--accent);
	}
	.dv-doc :global(del.dv-del) {
		color: var(--muted);
		text-decoration-color: var(--red);
	}
	.dv-doc :global(del.dv-del + ins.dv-ins) {
		margin-left: 3px;
	}
	.dv-doc :global(.dv-gap) {
		margin: 4px 0 12px 12px;
		font-size: 12px;
		color: var(--faint);
	}
	.dv-doc :global(.dv-flash) {
		background: var(--amber-soft);
	}

	/* ---- who wrote what: your words plain, AI's softly tinted, a line in the margin by who wrote most of a block ---- */
	.dv-authors :global(.dv-block) {
		border-left-color: transparent;
	}
	.dv-authors :global(.dv-block[data-by='ai']) {
		border-left-color: var(--accent);
	}
	.dv-authors :global(.dv-block[data-by='unknown']) {
		border-left-color: var(--line);
	}
	.dv-authors :global(.dv-by[data-by='ai']) {
		background: var(--accent-soft);
		border-radius: 2px;
	}
	.dv-authors :global(.dv-by[data-by='unknown']) {
		text-decoration: underline dotted var(--faint);
	}
	.dv-authors :global(.dv-lit) {
		border-left-color: var(--accent);
	}
	.dv-authors :global(.dv-lit .dv-by) {
		background: var(--accent-soft);
	}
	.dv-authors :global(.dv-dim) {
		opacity: 0.5;
	}

	/* ---- line by line, as git shows it ---- */
	.dl-hunk {
		border: 1px solid var(--line);
		border-radius: 8px;
		overflow: hidden;
		margin-bottom: 12px;
		background: var(--field);
		font-size: 12.5px;
	}
	.dl-head {
		padding: 4px 10px;
		color: var(--faint);
		background: var(--card-2, var(--card));
		border-bottom: 1px solid var(--line);
	}
	.dl-line {
		display: grid;
		grid-template-columns: 3.2em 3.2em 1.4em 1fr;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.dl-no {
		color: var(--faint);
		text-align: right;
		padding-right: 8px;
		user-select: none;
	}
	.dl-sign {
		color: var(--faint);
		user-select: none;
	}
	.dl-add {
		background: var(--green-soft);
	}
	.dl-add[data-by='ai'] {
		background: var(--accent-soft);
	}
	.dl-del {
		background: var(--red-soft);
	}
	.dl-emph {
		border-radius: 2px;
		font-weight: 600;
	}
	.dl-add .dl-emph {
		box-shadow: inset 0 -2px var(--green);
	}
	.dl-add[data-by='ai'] .dl-emph {
		box-shadow: inset 0 -2px var(--accent);
	}
	.dl-del .dl-emph {
		text-decoration: line-through;
		text-decoration-color: var(--red);
	}
	:global(.dl-hunk.dv-flash) {
		outline: 2px solid var(--amber);
	}
</style>
