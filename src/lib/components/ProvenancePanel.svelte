<script lang="ts">
	import { api } from '$lib/api';
	import { deliverableUrl, formatSize, threadUrl } from '$lib/actions';
	import { ago, date, dateTime, dayKey, plural } from '$lib/format';
	import { MMM, authorLabel, sizeChange } from '$lib/mmm';
	import { orderProvenance } from '$lib/order';
	import { listOrder } from '$lib/order.svelte';
	import { attempt, refresh } from '$lib/ui.svelte';
	import type { Deliverable, MmmLevel, Provenance, ProvenanceDraft, ProvenanceVersion, VersionUpdate } from '$lib/types';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import MessageCircle from '@lucide/svelte/icons/message-circle';
	import MessageSquare from '@lucide/svelte/icons/message-square';
	import Play from '@lucide/svelte/icons/play';
	import X from '@lucide/svelte/icons/x';
	import Menu, { type MenuItem } from './Menu.svelte';
	import MmmBadge from './MmmBadge.svelte';

	// `draft` is what the live file holds beyond the last version. The page works it out while the file is being edited, so
	// it is not left to what was true when the page loaded.
	let { project, doc, provenance, draft }: { project: string; doc: Deliverable; provenance: Provenance; draft: ProvenanceDraft | null } = $props();

	const summary = $derived(provenance.summary);
	const latest = $derived(doc.versions.at(-1) ?? null);
	/** Whether there is a live file to list: the page's "Live" chip follows the same rule. */
	const hasLive = $derived(!!doc.basePath && (doc.exists || doc.version === null));
	// The order is a setting (Settings, Order); the live file is the newest thing there is, so it goes where newest does.
	const groups = $derived(orderProvenance(provenance.groups, listOrder.order));

	let busy = $state(false);

	async function update(v: ProvenanceVersion, change: VersionUpdate) {
		busy = true;
		const done = await attempt(() => api.updateVersionRecord(project, doc.basePath, v.version, change));
		busy = false;
		if (done) await refresh();
	}

	const setLevel = (v: ProvenanceVersion, value: string) => {
		if (value !== (v.mmm ?? '')) void update(v, { mmm: value as MmmLevel | '' });
	};
	const setBy = (v: ProvenanceVersion, value: string) => {
		if (value !== v.by) void update(v, { by: value as 'human' | 'ai' });
	};

	/** Who made it: a run Ledger recorded can't be argued with, anything else is the person's to say. */
	const authors = (v: ProvenanceVersion): MenuItem[] => [
		{ id: 'human', label: 'You', checked: v.by === 'human' },
		{ id: 'ai', label: 'AI', checked: v.by === 'ai' }
	];

	const levels = (v: ProvenanceVersion): MenuItem[] => [
		...MMM.map((m) => ({ id: m.level, label: m.name, sub: m.tagline, dot: m.color, checked: v.mmm === m.level })),
		...(v.mmm ? [{ id: '', label: 'Remove the label', icon: X, rule: true }] : [])
	];

	/** "3 Oct", or "3 Oct – 9 Oct" when a thread's part in the file spans days. */
	function span(first: string, last: string): string {
		return dayKey(first) === dayKey(last) ? date(first) : `${date(first)} – ${date(last)}`;
	}

	const promptUrl = (thread: string, id: string) => `${threadUrl(project, thread)}?item=pr-${id}`;
</script>

<div class="pv-summary">
	<h3 class="pv-title">Summary</h3>
	<div class="pv-line">
		{#if summary.label}
			<MmmBadge level={summary.label.level} tagline />
			<span class="pv-meta">
				as declared for v{summary.label.version}{summary.label.version !== latest ? `, latest is v${latest}` : ''}
			</span>
		{:else}
			<span class="pv-meta">No AI-use label declared yet.</span>
		{/if}
	</div>
	<div class="pv-counts">
		{plural(summary.versions, 'version')}
		· {summary.ai} made with AI · {summary.human} by you{summary.unrecorded ? ` · ${summary.unrecorded} not recorded` : ''}
	</div>
</div>

<!-- The live file is a version of its own, the newest: always listed, and a draft when it holds more than the last version. -->
{#snippet liveRow()}
{#if hasLive}
	<div class="pv-live" class:draft={!!draft} class:current={doc.version === null}>
		<div class="pv-line">
			<a class="vchip" class:on={doc.version === null} href={deliverableUrl(project, doc.basePath)}>Live</a>
			<strong>{draft ? `Draft${draft.since ? ` after v${draft.since}` : ''}` : latest ? `Same as v${latest}` : 'Not published yet'}</strong>
		</div>
		{#if draft}
			<span class="pv-meta">the live file changed {ago(draft.modified)}</span>
			{#if draft.stoppedRun}
				<div class="pv-warn">
					Claude Code’s run of “{draft.stoppedRun.promptTitle}” changed this file{draft.stoppedRun.status === 'finished'
						? ''
						: ` and was ${draft.stoppedRun.status}`}, and no version holds it yet. Publishing starts with “made with AI” ticked.
				</div>
			{/if}
		{/if}
	</div>
{/if}
{/snippet}

{#if listOrder.order === 'desc'}{@render liveRow()}{/if}

{#each groups as group (group.thread?.slug ?? '—')}
	<section class="pv-group">
		<div class="pv-thread">
			{#if group.thread}
				{#if group.thread.exists}
					<a href={threadUrl(project, group.thread.slug)} title="Open the thread">{group.thread.title}</a>
				{:else}
					<span class="name" title={group.thread.slug}>{group.thread.title}</span>
				{/if}
			{:else}
				<span class="name">Not in any thread</span>
			{/if}
			<span class="pv-meta">{span(group.firstAt, group.lastAt)}</span>
		</div>
		{#if !group.thread}
			<div class="pv-hint">Published without a thread, or before Ledger kept track of where.</div>
		{/if}

		<div class="pv-entries">
			{#each group.entries as e (e.kind + ':' + (e.kind === 'version' ? e.version : e.id))}
				{#if e.kind === 'prompt'}
					<div class="pv-row">
						<span class="glyph" title="Prompt"><MessageSquare size={13} /></span>
						<span>Asked in</span>
						{#if group.thread?.exists}
							<a href={promptUrl(group.thread.slug, e.id)}>{e.title}</a>
						{:else}
							<span>{e.title}</span>
						{/if}
					</div>
				{:else if e.kind === 'run'}
					<div class="pv-row">
						<span class="glyph" title="Claude Code run"><Play size={13} /></span>
						<span>
							Claude Code run{e.status === 'finished' ? '' : `, ${e.status}`}
							· {e.model || 'default model'} · {e.seconds < 60 ? `${e.seconds} s` : `${Math.floor(e.seconds / 60)} min ${e.seconds % 60} s`}
							{e.version ? (e.status === 'finished' ? `· published v${e.version}` : `· its changes went into v${e.version}`) : '· published nothing'}
						</span>
					</div>
				{:else if e.kind === 'feedback'}
					<div class="pv-row">
						<span class="glyph" title="Feedback"><MessageCircle size={13} /></span>
						<a href="{threadUrl(project, group.thread?.slug ?? '')}?item=f-{e.id}">
							Feedback{e.status === 'open' ? ' (open)' : ' (resolved)'}{e.version ? ` on v${e.version}` : ''}: {e.excerpt}
						</a>
					</div>
				{:else}
					<div class="pv-version" class:current={doc.version === e.version}>
						<div class="pv-line">
							<a class="vchip" class:on={doc.version === e.version} href={deliverableUrl(project, e.path)}>v{e.version}</a>
							{#if e.run}
								<span class="pv-by ai">{authorLabel(e)}</span>
							{:else}
								<Menu
									class="pv-pick pv-by {e.by}"
									label="Made by, v{e.version}"
									title="Who made v{e.version}"
									items={authors(e)}
									onpick={(id) => setBy(e, id)}
									disabled={busy}
								>
									{authorLabel(e)}<ChevronDown size={12} />
								</Menu>
							{/if}
							<Menu
								class="pv-pick"
								label="AI-use label, v{e.version}"
								title={e.mmm ? 'Change the AI-use label' : 'Set the AI-use label'}
								items={levels(e)}
								onpick={(id) => setLevel(e, id)}
								disabled={busy}
							>
								{#if e.mmm}<MmmBadge level={e.mmm} />{:else}<span class="pv-unset">Set label…</span>{/if}<ChevronDown size={12} />
							</Menu>
						</div>
						<div class="pv-line">
							<span class="pv-when">{dateTime(e.at)}</span>
							{#if e.sizeChange}<span class="pv-size">{sizeChange(e.sizeChange)}</span>{/if}
						</div>
						{#if e.note}<p class="pv-note">{e.note}</p>{/if}
						{#if e.prompt}
							<div class="pv-meta">
								from the prompt
								{#if e.prompt.exists}
									<a href={promptUrl(e.prompt.threadSlug, e.prompt.id)}>{e.prompt.title}</a>
								{:else}
									<span>{e.prompt.title}</span>
								{/if}
							</div>
						{/if}
						<details class="pv-details">
							<summary>Details</summary>
							<dl class="kv">
								<dt>File</dt>
								<dd class="mono">{e.path}</dd>
								<dt>Size</dt>
								<dd>{formatSize(e.size)}</dd>
								<dt>SHA-256</dt>
								<dd class="mono">{e.sha256 ?? 'not worked out for this kind of file'}</dd>
								<dt>Published</dt>
								<dd>{dateTime(e.at)}</dd>
								<dt>Record</dt>
								<dd class="mono">{e.record ? `versions/${e.record}.md` : 'none, this version is older than records'}</dd>
								{#if e.post}
									<dt>Note</dt>
									<dd><a href="{threadUrl(project, e.post.threadSlug)}?item=p-{e.post.id}">in its thread</a></dd>
								{/if}
								{#if e.prompt}
									<dt>Prompt</dt>
									<dd class="mono">{e.prompt.id}</dd>
								{/if}
								{#if e.run}
									<dt>Run</dt>
									<dd class="mono">{e.run.id}</dd>
									<dt>Model</dt>
									<dd>{e.run.model || 'default'}</dd>
									<dt>Mode</dt>
									<dd>{e.run.permissionMode || 'default'}</dd>
									<dt>Run</dt>
									<dd>{e.run.status} · {e.run.seconds} s{e.run.turns !== null ? ` · ${plural(e.run.turns, 'turn')}` : ''}</dd>
								{/if}
							</dl>
						</details>
					</div>
				{/if}
			{/each}
		</div>
	</section>
{:else}
	<div class="pv-hint">Nothing has produced this file yet. Versions you publish, and the prompts and runs behind them, are listed here by thread.</div>
{/each}

{#if listOrder.order === 'asc'}{@render liveRow()}{/if}
