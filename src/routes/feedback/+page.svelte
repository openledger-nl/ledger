<script lang="ts">
	// All feedback in one inbox, across projects: what was given in Ledger and what came in from Portflow, filtered by
	// who gave it. Feedback is resolved only when you say so. Wireframe E1; requirements: Feedback.
	import { page } from '$app/state';
	import Check from '@lucide/svelte/icons/check';
	import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import { api } from '$lib/api';
	import { baseName, deliverableUrl, liveDeliverable, threadUrl } from '$lib/actions';
	import { dateTime } from '$lib/format';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import { feedbackTag, givenName, roleName } from '$lib/v1';
	import type { FeedbackRole, InboxFeedback } from '$lib/types';
	import Dropdown from '$lib/components/Dropdown.svelte';
	import Markdown from '$lib/components/Markdown.svelte';
	import { dayLabel } from '$lib/semesters';
	import { personTagOptions } from '$lib/tagIndex';
	import RecordFeedbackModal from '$lib/components/RecordFeedbackModal.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	type Source = 'ledger' | 'portflow';
	const sourceOf = (f: InboxFeedback): Source => (f.portflowComment !== null ? 'portflow' : 'ledger');

	let status = $state<'open' | 'resolved' | 'all'>('open');
	let source = $state<Source | ''>('');
	// A link can open the inbox on one person's feedback: /feedback?from=coach/frank.
	let person = $state(page.url.searchParams.get('from') ?? '');
	let onProject = $state('');
	let recording = $state(false);

	/** Who gave what, by their tag: `coach/frank`; your own notes have none. */
	const people = $derived(
		[...new Map(data.feedback.flatMap((f) => (feedbackTag(f) ? [[feedbackTag(f) as string, f] as const] : []))).entries()].sort((a, b) =>
			(a[1].from ?? '').localeCompare(b[1].from ?? '')
		)
	);
	const projectsWithFeedback = $derived([...new Map(data.feedback.map((f) => [f.project, f.projectName])).entries()].sort((a, b) => a[1].localeCompare(b[1])));
	const counts = $derived({
		open: data.feedback.filter((f) => f.status === 'open').length,
		resolved: data.feedback.filter((f) => f.status === 'resolved').length
	});
	const shown = $derived(
		data.feedback.filter(
			(f) =>
				(status === 'all' || f.status === status) &&
				(!source || sourceOf(f) === source) &&
				(!person || feedbackTag(f) === person) &&
				(!onProject || f.project === onProject)
		)
	);

	/** What it lands on: a deliverable (or one of its versions), a story, an evidence item, or the project. */
	function where(f: InboxFeedback): { label: string; href: string | null } {
		if (f.deliverablePath) {
			const version = f.deliverableVersion ? ` v${f.deliverableVersion}` : '';
			return { label: `${baseName(liveDeliverable(f.deliverablePath, f.deliverableVersion))}${version}`, href: deliverableUrl(f.project, f.deliverablePath) };
		}
		if (f.story) return { label: `${f.story}${f.storyTitle ? ` ${f.storyTitle}` : ''}`, href: `/projects/${f.project}/stories` };
		if (f.evidence) return { label: `${f.evidence}${f.evidenceVersion ? ` v${f.evidenceVersion}` : ''}`, href: null };
		return { label: 'the whole project', href: null };
	}

	const sourceOptions = [
		{ value: '', label: 'Every source' },
		{ value: 'ledger', label: 'Ledger' },
		{ value: 'portflow', label: 'Portflow' }
	];
	const personOptions = $derived([
		{ value: '', label: 'From anyone' },
		...people.map(([tag, f]) => ({ value: tag, label: f.from ?? tag, hint: `${roleName((f.fromRole as FeedbackRole) || 'other')} · #${tag}` }))
	]);
	const projectOptions = $derived([{ value: '', label: 'Every project' }, ...projectsWithFeedback.map(([slug, name]) => ({ value: slug, label: name }))]);

	async function setStatus(f: InboxFeedback, next: 'open' | 'resolved') {
		if (await attempt(() => api.setFeedbackStatus(f.project, f.threadSlug, f.id, next))) await refresh();
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
			<button class="btn primary" onclick={() => (recording = true)} disabled={!data.projects.length}><MessageSquarePlus size={16} />Record feedback</button>
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
			<Dropdown bind:value={() => source, (v) => (source = v as Source | '')} options={sourceOptions} label="Source" />
			<Dropdown bind:value={person} options={personOptions} label="From" />
			<Dropdown bind:value={onProject} options={projectOptions} label="Project" />
		</div>

		<div class="fb-list">
			{#each shown as f (f.id)}
				{@const on = where(f)}
				{@const given = givenName(f.given)}
				<article class="card fb-item" class:resolved={f.status === 'resolved'} class:own={f.kind === 'own'}>
					<header class="fb-head">
						<div class="fb-who">
							{#if f.kind === 'own'}
								<strong>Your note</strong>
							{:else if f.from}
								<strong>{f.from}</strong>{#if f.fromRole}<span class="fb-role">{roleName(f.fromRole as FeedbackRole)}</span>{/if}
							{:else}
								<strong>Someone</strong>
							{/if}
							{#if sourceOf(f) === 'portflow'}<span class="badge">Portflow</span>{/if}
						</div>
						<span class="fb-when">{dateTime(f.createdAt)}</span>
					</header>
					<div class="fb-where">
						On {#if on.href}<a href={on.href}>{on.label}</a>{:else}{on.label}{/if} · {f.projectName}
						· in <a href="{threadUrl(f.project, f.threadSlug)}?item=f-{f.id}">{f.threadTitle}</a>
						{#if given} · {given.toLowerCase()}{/if}
						{#if f.daily} · <a href="/meetings?day={f.daily}">meeting notes {dayLabel(f.daily)}</a>{/if}
					</div>
					<div class="fb-body"><Markdown html={f.html} /></div>
					<footer class="actions">
						{#if f.status === 'open'}
							<button class="btn small" onclick={() => setStatus(f, 'resolved')}><Check size={13} />Mark resolved</button>
						{:else}
							<span class="hint">Resolved {dateTime(f.updatedAt)}</span>
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
		projects={data.projects}
		peopleOptions={personTagOptions(data.notes, data.feedback, data.people)}
		people={data.people}
		notes={data.notes}
		onclose={() => (recording = false)}
		onsaved={recorded}
	/>
{/if}
