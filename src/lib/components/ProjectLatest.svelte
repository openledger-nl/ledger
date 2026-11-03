<script lang="ts">
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import File from '@lucide/svelte/icons/file';
	import FileImage from '@lucide/svelte/icons/file-image';
	import FileText from '@lucide/svelte/icons/file-text';
	import MessagesSquare from '@lucide/svelte/icons/messages-square';
	import { deliverableTarget, deliverableUrl, newThreadFlow, projectUrl, threadUrl } from '$lib/actions';
	import MmmBadge from '$lib/components/MmmBadge.svelte';
	import { ago, newest, stamp, threadMeta } from '$lib/format';
	import type { DeliverableSummary, ProjectSummary } from '$lib/types';

	// A project's latest threads and deliverables, as the dashboard's project card and the project's own Overview show them.
	// Two sections side by side in a fragment: the card stacks them, the Overview sets them next to each other.

	let { project, deliverables }: { project: ProjectSummary; deliverables: DeliverableSummary[] } = $props();

	const THREADS = 3;
	const DELIVERABLES = 2;
	const DAY = 86_400_000;

	const within = (iso: string, ms: number) => Date.now() - stamp(iso) < ms;
	const ICON = { markdown: FileText, image: FileImage, other: File } as const;

	const threads = $derived(newest(project.threads, (t) => t.lastActivity).slice(0, THREADS));
	const latest = $derived(newest(deliverables, (d) => d.modified).slice(0, DELIVERABLES));
</script>

<section class="proj-section">
	<div class="proj-section-head">
		<h3>Latest threads</h3>
		{#if project.threads.length > THREADS}
			<a href="{projectUrl(project.slug)}/threads">All {project.threads.length}<ArrowRight size={13} /></a>
		{/if}
	</div>
	{#each threads as t (t.slug)}
		<a class="dash-row" href={threadUrl(project.slug, t.slug)}>
			<span class="dash-ico"><MessagesSquare size={16} /></span>
			<span class="dash-main">
				<span class="dash-title">{t.title}</span>
				<span class="dash-meta">{threadMeta(t)}</span>
			</span>
			<span class="dash-when" class:fresh={within(t.lastActivity, DAY)}>{ago(t.lastActivity)}</span>
		</a>
	{:else}
		<div class="dash-empty">
			No threads yet.
			<button class="link accent" onclick={() => newThreadFlow(project.slug)}>Start one</button>
		</div>
	{/each}
</section>

<section class="proj-section">
	<div class="proj-section-head">
		<h3>Latest deliverables</h3>
		{#if deliverables.length > DELIVERABLES}
			<a href="{projectUrl(project.slug)}/deliverables">All {deliverables.length}<ArrowRight size={13} /></a>
		{/if}
	</div>
	{#each latest as d (d.path)}
		{@const Icon = ICON[d.kind]}
		<a class="dash-row" href={deliverableUrl(project.slug, deliverableTarget(d))}>
			<span class="dash-ico"><Icon size={16} /></span>
			<span class="dash-main">
				<span class="dash-title" title={d.path}>{d.name}</span>
				<span class="dash-meta dash-badges">
					{#if d.versions.length}
						<span class="badge published">v{d.versions.at(-1)}</span>
					{:else}
						<span class="badge draft">Draft</span>
					{/if}
					{#if d.mmm}<MmmBadge level={d.mmm} />{/if}
					{#if d.openFeedback}<span class="badge draft">{d.openFeedback} open</span>{/if}
					{#if !d.exists}<span class="badge">live file removed</span>{/if}
				</span>
			</span>
			<span class="dash-when" class:fresh={within(d.modified, DAY)}>{ago(d.modified)}</span>
		</a>
	{:else}
		<div class="dash-empty">
			Nothing delivered yet.
			<a class="link accent" href="{projectUrl(project.slug)}/deliverables">Create one</a>
		</div>
	{/each}
</section>
