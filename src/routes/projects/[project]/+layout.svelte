<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Target from '@lucide/svelte/icons/target';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import Users from '@lucide/svelte/icons/users';
	import { deleteProjectFlow, projectUrl } from '$lib/actions';
	import Menu from '$lib/components/Menu.svelte';
	import MmmBadge from '$lib/components/MmmBadge.svelte';
	import { projectDialogs } from '$lib/dialogs.svelte';
	import { dayLabel } from '$lib/semesters';
	import { daysLeft, daysLeftText, endDay, kindName, ROLES } from '$lib/v1';
	import type { LayoutData } from './$types';

	let { data, children }: { data: LayoutData; children: Snippet } = $props();

	const project = $derived(data.project);
	const base = $derived(projectUrl(project.slug));
	const path = $derived(page.url.pathname);

	// A thread and a deliverable have a header of their own (with a way back to the project); the project's tabs
	// belong to the other pages.
	const ownHeader = $derived(!!page.params.thread || !!page.route.id?.endsWith('/deliverables/view'));

	// The project's kind, goal and end (wireframe B2), when it has them.
	const meta = $derived(data.meta);
	const end = $derived(meta ? endDay(meta, data.semesters) : null);
	const left = $derived(daysLeft(end));
	const editMeta = () => (projectDialogs.project = { slug: project.slug, name: project.name, meta });

	const tabs = $derived([
		{ href: base, label: 'Overview', n: null, exact: true },
		{ href: `${base}/threads`, label: 'Threads', n: project.threadCount, exact: false },
		{ href: `${base}/deliverables`, label: 'Deliverables', n: project.deliverableCount, exact: false },
		{ href: `${base}/attachments`, label: 'Attachments', n: project.attachmentCount, exact: false },
		{ href: `${base}/stories`, label: 'Stories', n: project.storyCount, exact: false },
		{ href: `${base}/board`, label: 'Board', n: project.openTaskCount, exact: false },
		{ href: `${base}/people`, label: 'People', n: data.people.length || null, exact: false },
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
				<button class="btn small ghost" onclick={editMeta}>Edit</button>
				<Menu
					class="btn small ghost more"
					label="More about this project"
					items={[{ id: 'delete', label: 'Delete project', icon: Trash2, danger: true }]}
					onpick={() => deleteProjectFlow(project.slug, project.name)}
					align="end"
				>
					<Ellipsis size={16} />
				</Menu>
			</div>
		</div>
		{#if meta}
			<p class="pj-goal"><Target size={15} /><span><strong>Goal</strong> {meta.goal}</span></p>
			<div class="subline pj-facts">
				<span class="badge">{kindName(meta.kind)}</span>
				<span title={end ? dayLabel(end) : ''}>{daysLeftText(left)}{#if end} | ends {dayLabel(end)}{/if}</span>
				{#if meta.intendedMmm}<span class="pj-intended">Intended <MmmBadge level={meta.intendedMmm} /></span>{/if}
				{#if meta.client}<span>Client: <a href="/clients?client={encodeURIComponent(meta.client)}">{meta.clientName ?? meta.client}</a></span>{/if}
				{#if meta.mainQuestion}<span class="pj-mq" title={meta.mainQuestion}>Main question: {meta.mainQuestion}</span>{/if}
			</div>
		{/if}
		{#if data.people.length}
			<div class="subline pj-facts pj-people" aria-label="People">
				<Users size={14} />
				{#each ROLES.filter((r) => data.people.some((p) => p.role === r.role)) as r (r.role)}
					<span><strong>{r.role !== 'other' && data.people.filter((p) => p.role === r.role).length > 1 ? `${r.name}s` : r.name}</strong> {data.people.filter((p) => p.role === r.role).map((p) => p.name).join(', ')}</span>
				{/each}
			</div>
		{/if}
		{#if !meta}
			<div class="notice info pj-unset">
				This project has no goal or end date yet: the most important thing a project has.
				<button class="link accent" onclick={editMeta}>Set them</button>
			</div>
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
