<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import Target from '@lucide/svelte/icons/target';
	import { deleteProjectFlow, projectUrl, renameProjectFlow } from '$lib/actions';
	import MmmBadge from '$lib/components/MmmBadge.svelte';
	import PreviewBadge from '$lib/components/PreviewBadge.svelte';
	import { previewDialogs } from '$lib/mock/dialogs.svelte';
	import { previews } from '$lib/mock/flag.svelte';
	import { dayLabel } from '$lib/semesters';
	import { daysLeft, daysLeftText, endDay, kindName } from '$lib/v1';
	import type { LayoutData } from './$types';

	let { data, children }: { data: LayoutData; children: Snippet } = $props();

	const project = $derived(data.project);
	const base = $derived(projectUrl(project.slug));
	const path = $derived(page.url.pathname);

	// A thread and a deliverable have a header of their own (with a way back to the project); the project's tabs
	// belong to the other pages.
	const ownHeader = $derived(!!page.params.thread || !!page.route.id?.endsWith('/deliverables/view'));

	// Preview: the project's kind, goal and end (wireframe B2), when it has them.
	const meta = $derived(data.meta);
	const end = $derived(meta ? endDay(meta, data.semesters) : null);
	const left = $derived(daysLeft(end));
	const editMeta = () => (previewDialogs.project = { slug: project.slug, name: project.name, meta });

	const tabs = $derived([
		{ href: base, label: 'Overview', n: null, exact: true },
		{ href: `${base}/threads`, label: 'Threads', n: project.threadCount, exact: false },
		{ href: `${base}/deliverables`, label: 'Deliverables', n: project.deliverableCount, exact: false },
		{ href: `${base}/attachments`, label: 'Attachments', n: project.attachmentCount, exact: false },
		{ href: `${base}/stories`, label: 'Stories', n: project.storyCount, exact: false },
		{ href: `${base}/board`, label: 'Board', n: project.openTaskCount, exact: false },
		{ href: `${base}/templates`, label: 'Templates', n: null, exact: false }
	]);
	const active = (t: { href: string; exact: boolean }) => (t.exact ? path === t.href : path === t.href || path.startsWith(t.href + '/'));
</script>

{#if !ownHeader}
	<div class="header-wrap">
		<div class="header-top">
			<div>
				<h1>{project.name}</h1>
				{#if project.description}<div class="sub">{project.description}</div>{/if}
			</div>
			<div class="header-actions">
				{#if previews.enabled}<button class="btn small ghost" onclick={editMeta}>{meta ? 'Goal & end' : 'Set goal & end'}</button>{/if}
				<button class="btn small ghost" onclick={() => renameProjectFlow(project.slug, project.name)}>Rename</button>
				<button class="btn small ghost danger" onclick={() => deleteProjectFlow(project.slug, project.name)}>Delete</button>
			</div>
		</div>
		{#if previews.enabled}
			{#if meta}
				<p class="pj-goal"><Target size={15} /><span><strong>Goal</strong> {meta.goal}</span></p>
				<div class="subline pj-facts">
					<span class="badge">{kindName(meta.kind)}</span>
					<span title={end ? dayLabel(end) : ''}>{daysLeftText(left)}{#if end} · ends {dayLabel(end)}{/if}</span>
					{#if meta.intendedMmm}<span class="pj-intended">Intended <MmmBadge level={meta.intendedMmm} /></span>{/if}
					{#if meta.client}<span>Client: {meta.client}</span>{/if}
					{#if meta.mainQuestion}<span class="pj-mq" title={meta.mainQuestion}>Main question: {meta.mainQuestion}</span>{/if}
					<PreviewBadge />
				</div>
			{:else}
				<div class="notice info pj-unset">
					This project has no goal or end date yet: the most important thing a project has.
					<button class="link accent" onclick={editMeta}>Set them</button>
					<PreviewBadge />
				</div>
			{/if}
		{/if}
		<nav class="tabs" aria-label="Project">
			{#each tabs as t (t.href)}
				<a class="tab" class:active={active(t)} href={t.href}>
					{t.label}{#if t.n}<span class="n">{t.n}</span>{/if}
				</a>
			{/each}
		</nav>
	</div>
{/if}

{@render children()}
