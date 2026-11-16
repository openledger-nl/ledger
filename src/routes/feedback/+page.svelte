<script lang="ts">
	// All feedback in one inbox, across projects: from Ledger, Portflow and FeedPulse, personal and group, filtered by
	// who gave it. Feedback is resolved only when you say so. Wireframe E1; requirements: Feedback.
	import Check from '@lucide/svelte/icons/check';
	import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import { mockApi } from '$lib/mock/api';
	import { dateTime } from '$lib/format';
	import { projectTagNames } from '$lib/projectTags';
	import { refresh, toast } from '$lib/ui.svelte';
	import { personName, roleName } from '$lib/v1';
	import type { FeedbackItem, FeedbackSource } from '$lib/types';
	import PreviewBadge from '$lib/components/PreviewBadge.svelte';
	import Dropdown from '$lib/components/Dropdown.svelte';
	import { dayLabel } from '$lib/semesters';
	import { personTagOptions } from '$lib/tagIndex';
	import RecordFeedbackModal from '$lib/components/RecordFeedbackModal.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const SOURCES: { id: FeedbackSource; name: string }[] = [
		{ id: 'ledger', name: 'Ledger' },
		{ id: 'portflow', name: 'Portflow' },
		{ id: 'feedpulse', name: 'FeedPulse' }
	];

	let status = $state<'open' | 'resolved' | 'all'>('open');
	let source = $state<FeedbackSource | ''>('');
	let scope = $state<'personal' | 'group' | ''>('');
	let person = $state('');
	let onProject = $state('');
	let recording = $state(false);

	const tags = $derived(projectTagNames(data.projects, data.meta, data.notes));
	const people = $derived([...new Set(data.feedback.flatMap((f) => (f.from ? [f.from] : [])))].sort());
	const projectsWithFeedback = $derived(tags.list.filter((t) => data.feedback.some((f) => f.target.project === t)));
	const counts = $derived({
		open: data.feedback.filter((f) => f.status === 'open').length,
		resolved: data.feedback.filter((f) => f.status === 'resolved').length
	});
	const shown = $derived(
		data.feedback.filter(
			(f) =>
				(status === 'all' || f.status === status) &&
				(!source || f.source === source) &&
				(!scope || f.scope === scope) &&
				(!person || f.from === person) &&
				(!onProject || f.target.project === onProject)
		)
	);

	function where(f: FeedbackItem): string {
		const t = f.target;
		const p = tags.name(t.project);
		if (t.on === 'deliverable') return `${t.deliverable.replace(/\.md$/, '')}${t.version ? ` v${t.version}` : ''} · ${p}`;
		if (t.on === 'story') return `${t.story} · ${p}`;
		return p;
	}

	/** "meeting notes 28-10-2026": where feedback given in a meeting was written down. */
	const dailyOf = (day: string | null) => (day ? `meeting notes ${dayLabel(day)}` : null);

	const sourceOptions = [{ value: '', label: 'Every source' }, ...SOURCES.map((s) => ({ value: s.id, label: s.name }))];
	const scopeOptions = [
		{ value: '', label: 'Personal and group' },
		{ value: 'personal', label: 'Personal' },
		{ value: 'group', label: 'Group' }
	];
	const personOptions = $derived([{ value: '', label: 'From anyone' }, ...people.map((p) => ({ value: p, label: personName(p), hint: p }))]);
	const projectOptions = $derived([{ value: '', label: 'Every project' }, ...projectsWithFeedback.map((t) => ({ value: t, label: tags.name(t) }))]);

	async function setStatus(f: FeedbackItem, next: 'open' | 'resolved') {
		await mockApi.setFeedbackStatus(f.id, next);
		await refresh();
	}

	async function recorded() {
		recording = false;
		status = 'open';
		toast('Feedback recorded');
		await refresh();
	}
</script>

<svelte:head><title>Feedback · Ledger</title></svelte:head>

<div class="header-wrap flat">
	<div class="header-top">
		<div>
			<h1>Feedback</h1>
			<div class="sub">From coaches, experts, clients and teammates, and your own notes. Only you mark it resolved.</div>
		</div>
		<div class="header-actions">
			<PreviewBadge />
			<button class="btn primary" onclick={() => (recording = true)}><MessageSquarePlus size={16} />Record feedback</button>
		</div>
	</div>
</div>

<div class="page">
	<div class="page-inner">
		<div class="filter-bar">
			<div class="seg" role="group" aria-label="Status">
				<button class:on={status === 'open'} aria-pressed={status === 'open'} onclick={() => (status = 'open')}>Open {counts.open}</button>
				<button class:on={status === 'resolved'} aria-pressed={status === 'resolved'} onclick={() => (status = 'resolved')}>Resolved {counts.resolved}</button>
				<button class:on={status === 'all'} aria-pressed={status === 'all'} onclick={() => (status = 'all')}>All</button>
			</div>
			<Dropdown bind:value={() => source, (v) => (source = v as FeedbackSource | '')} options={sourceOptions} label="Source" />
			<Dropdown bind:value={() => scope, (v) => (scope = v as 'personal' | 'group' | '')} options={scopeOptions} label="Personal or group" />
			<Dropdown bind:value={person} options={personOptions} label="From" />
			<Dropdown bind:value={onProject} options={projectOptions} label="Project" />
		</div>

		<div class="fb-list">
			{#each shown as f (f.id)}
				<article class="card fb-item" class:resolved={f.status === 'resolved'} class:own={f.kind === 'own'}>
					<header class="fb-head">
						<div class="fb-who">
							{#if f.kind === 'own'}
								<strong>Your note</strong>
							{:else if f.from}
								<strong title={f.from}>{personName(f.from)}</strong><span class="fb-role">{roleName(f.role)}</span>
							{/if}
							<span class="badge">{SOURCES.find((s) => s.id === f.source)?.name}</span>
							{#if f.scope === 'group'}<span class="badge accent">Group</span>{/if}
						</div>
						<span class="fb-when">{dateTime(f.at)}</span>
					</header>
					<div class="fb-where">On {where(f)}{#if f.given} · {f.given}{/if}{#if dailyOf(f.daily)} · <a href="/meetings?day={f.daily}">{dailyOf(f.daily)}</a>{/if}</div>
					<p class="fb-body">{f.body}</p>
					{#if f.followUps.length}
						<div class="fb-follow"><span>What you did with it:</span>{#each f.followUps as u (u)}<span class="chip plain">{u}</span>{/each}</div>
					{/if}
					<footer class="actions">
						{#if f.status === 'open'}
							<button class="btn small" onclick={() => setStatus(f, 'resolved')}><Check size={13} />Mark resolved</button>
						{:else}
							<span class="hint">Resolved {f.resolvedAt ? dateTime(f.resolvedAt) : ''}</span>
							<button class="btn small ghost" onclick={() => setStatus(f, 'open')}><RotateCcw size={13} />Reopen</button>
						{/if}
					</footer>
				</article>
			{:else}
				<div class="empty-state"><p>{data.feedback.length ? 'No feedback matches.' : 'No feedback yet.'}</p></div>
			{/each}
		</div>
	</div>
</div>

{#if recording}
	<RecordFeedbackModal
		projectTags={tags.list}
		projectName={tags.name}
		vaultProjects={data.projects.map((p) => p.slug)}
		peopleOptions={personTagOptions(data.notes, data.feedback)}
		notes={data.notes}
		onclose={() => (recording = false)}
		onsaved={recorded}
	/>
{/if}
