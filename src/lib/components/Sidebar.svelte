<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import FileText from '@lucide/svelte/icons/file-text';
	import House from '@lucide/svelte/icons/house';
	import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
	import Plus from '@lucide/svelte/icons/plus';
	import Settings from '@lucide/svelte/icons/settings';
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { deliverableTarget, deliverableUrl, newDeliverableFlow, newProjectFlow, newThreadFlow, projectUrl, threadUrl } from '$lib/actions';
	import { deliverableTime, newest } from '$lib/format';
	import { orderThreads } from '$lib/order';
	import { listOrder } from '$lib/order.svelte';
	import { sync, syncFooter } from '$lib/ui.svelte';
	import { layout, toggleSidebar } from '$lib/layout.svelte';
	import { portflow } from '$lib/portflow.svelte';
	import type { DeliverableSummary, ProjectSummary } from '$lib/types';
	import GammaIcon from '$lib/components/GammaIcon.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';

	let { projects }: { projects: ProjectSummary[] } = $props();

	const OPEN_KEY = 'ledger-open-project';

	/** The project that is unfolded: one at a time, null when none is. */
	let open = $state<string | null>(load());
	const path = $derived(page.url.pathname);
	const on = (prefix: string) => path === prefix || path.startsWith(prefix + '/');
	const currentProject = $derived(page.params.project);
	/** A thread or a deliverable being open is shown on its own row, not on its project's. */
	const inThread = $derived(!!page.params.thread);
	const viewing = $derived(page.route.id?.endsWith('/deliverables/view') ? page.url.searchParams.get('path') : null);

	function load(): string | null {
		try {
			return localStorage.getItem(OPEN_KEY) || null;
		} catch {
			return null;
		}
	}

	function remember() {
		try {
			if (open) localStorage.setItem(OPEN_KEY, open);
			else localStorage.removeItem(OPEN_KEY);
		} catch {
			// The tree still works for this session.
		}
	}

	/** Unfolds a project and, with it, folds the one that was open. */
	function unfold(slug: string) {
		open = slug;
		remember();
	}

	function toggle(slug: string) {
		open = open === slug ? null : slug;
		remember();
	}

	/** Choosing a project unfolds it and goes to its page. Choosing the one that is already unfolded folds it instead and
	 *  goes nowhere: what is open stays open, be it the project's page, a thread or a deliverable. */
	function choose(e: MouseEvent, slug: string) {
		const plain = e.button === 0 && !e.ctrlKey && !e.metaKey && !e.shiftKey && !e.altKey;
		if (open === slug && plain) {
			e.preventDefault();
			toggle(slug);
		} else {
			unfold(slug);
		}
	}

	// Going to a project, from anywhere, unfolds it. Folding it again by hand sticks until another project is chosen.
	$effect(() => {
		const slug = currentProject;
		if (slug) untrack(() => unfold(slug));
	});

	// Every project's deliverables, by project: the unfolded project's list and the Latest section read them. They are
	// looked up again whenever the vault changes (the projects are loaded afresh then).
	let deliverables = $state<Record<string, DeliverableSummary[]>>({});
	$effect(() => {
		const slugs = projects.map((p) => p.slug);
		let stale = false;
		Promise.all(slugs.map((slug) => api.listDeliverables(slug).then((items) => [slug, items] as const, () => [slug, []] as const))).then((all) => {
			if (!stale) deliverables = Object.fromEntries(all);
		});
		return () => (stale = true);
	});
	const byPath = (items: DeliverableSummary[]) => [...items].sort((a, b) => a.path.localeCompare(b.path));

	/** What was last worked on across all projects: the newest threads and deliverables. */
	const LATEST = 3;
	const latestThreads = $derived(
		newest(projects.flatMap((project) => project.threads.map((thread) => ({ thread, project }))), (x) => x.thread.lastActivity).slice(0, LATEST)
	);
	const latestDeliverables = $derived(
		newest(projects.flatMap((project) => (deliverables[project.slug] ?? []).map((d) => ({ d, project }))), (x) => deliverableTime(x.d).at).slice(0, LATEST)
	);

	// "just now" and "5 min ago" are worked out when drawn, so the footer redraws every half minute to keep them true.
	let tick = $state(0);
	$effect(() => {
		const timer = setInterval(() => tick++, 30_000);
		return () => clearInterval(timer);
	});

	const footer = $derived.by(() => {
		void tick;
		return syncFooter(sync.status);
	});

	// On a narrow window the drawer closes after every choice (but not after folding a project).
	function closeDrawer(e: MouseEvent) {
		// Folding a project goes nowhere, so the drawer stays.
		if (e.defaultPrevented) return;
		const target = (e.target as HTMLElement).closest('a, button');
		if (target && !target.classList.contains('folder-toggle')) layout.drawerOpen = false;
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<aside class="sidebar" class:open={layout.drawerOpen} onclick={closeDrawer} aria-label="Sidebar">
	<div class="brand">
		<a href="/" class="brand-link" aria-label="Home"><Wordmark /></a>
		<button class="icon-btn side-hide" title="Hide sidebar (Ctrl+\)" aria-label="Hide sidebar" onclick={toggleSidebar}>
			<PanelLeftClose size={18} />
		</button>
	</div>

	<a class="nav-item" class:active={path === '/'} href="/"><span class="nav-ico"><House size={16} /></span>Home</a>
	{#if portflow.enabled}
		<a class="nav-item" class:active={on('/gamma')} href="/gamma"><span class="nav-ico"><GammaIcon size={16} /></span>Gamma</a>
	{/if}
	<a class="nav-item" class:active={on('/settings')} href="/settings"><span class="nav-ico"><Settings size={16} /></span>Settings</a>

	{#if latestThreads.length || latestDeliverables.length}
		<div class="section-head"><span>Latest</span></div>
		<nav class="space-list" aria-label="Latest">
			{#if latestThreads.length}
				<div class="tree-label plain">Threads</div>
				{#each latestThreads as { thread, project } (project.slug + '/' + thread.slug)}
					<a class="tree-item" class:active={path === threadUrl(project.slug, thread.slug)} href={threadUrl(project.slug, thread.slug)}>
						<span class="dot"></span>
						<span class="stack">
							<span class="name">{thread.title}</span>
							<span class="of">{project.name}</span>
						</span>
					</a>
				{/each}
			{/if}
			{#if latestDeliverables.length}
				<div class="tree-label plain">Deliverables</div>
				{#each latestDeliverables as { d, project } (project.slug + '/' + d.path)}
					{@const target = deliverableTarget(d)}
					<a
						class="tree-item"
						class:active={currentProject === project.slug && (viewing === target || viewing === d.path)}
						href={deliverableUrl(project.slug, target)}
						title={d.path}
					>
						<span class="file"><FileText size={13} /></span>
						<span class="stack">
							<span class="name">{d.name}</span>
							<span class="of">{project.name}</span>
						</span>
					</a>
				{/each}
			{/if}
		</nav>
	{/if}

	<div class="section-head">
		<span>Projects</span>
		<button class="icon-btn" title="New project" aria-label="New project" onclick={newProjectFlow}><Plus size={16} /></button>
	</div>
	<nav class="space-list">
		{#each projects as p (p.slug)}
			{@const unfolded = open === p.slug}
			<div class="folder-row" class:active={on(projectUrl(p.slug)) && !inThread && viewing === null}>
				<button
					class="folder-toggle"
					aria-label={unfolded ? `Fold ${p.name}` : `Unfold ${p.name}`}
					aria-expanded={unfolded}
					onclick={() => toggle(p.slug)}
				>
					{#if unfolded}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
				</button>
				<a class="folder-link" href={projectUrl(p.slug)} onclick={(e) => choose(e, p.slug)}>
					<span class="name">{p.name}</span>
					<span class="count">{p.threads.length || ''}</span>
				</a>
			</div>
			{#if unfolded}
				<div class="folder-children">
					<a class="tree-label" class:active={path === `${projectUrl(p.slug)}/threads`} href="{projectUrl(p.slug)}/threads">
						Threads<span class="count">{p.threads.length || ''}</span>
					</a>
					{#each orderThreads(p.threads, listOrder.order) as t (t.slug)}
						<a class="tree-item" class:active={path === threadUrl(p.slug, t.slug)} href={threadUrl(p.slug, t.slug)}>
							<span class="dot"></span>
							<span class="name">{t.title}</span>
							<span class="count">{t.postCount || ''}</span>
						</a>
					{:else}
						<div class="space-empty">No threads yet</div>
					{/each}
					<button class="tree-item add" onclick={() => newThreadFlow(p.slug)}><Plus size={14} />New thread</button>

					<a class="tree-label" class:active={path === `${projectUrl(p.slug)}/deliverables`} href="{projectUrl(p.slug)}/deliverables">
						Deliverables<span class="count">{p.deliverableCount || ''}</span>
					</a>
					{#if deliverables[p.slug]}
						{#each byPath(deliverables[p.slug]) as d (d.path)}
							{@const target = deliverableTarget(d)}
							<a class="tree-item" class:active={viewing === target || viewing === d.path} href={deliverableUrl(p.slug, target)} title={d.path}>
								<span class="file"><FileText size={13} /></span>
								<span class="name">{d.name}</span>
							</a>
						{:else}
							<div class="space-empty">No deliverables yet</div>
						{/each}
					{/if}
					<button class="tree-item add" onclick={() => newDeliverableFlow(p.slug)}><Plus size={14} />New deliverable</button>
				</div>
			{/if}
		{:else}
			<div class="space-empty">No projects yet</div>
		{/each}
	</nav>

	<div class="spacer"></div>

	<div class="sync-box">
		<div class="sync-line">
			<span class="dot {footer.dot}"></span><span>{footer.title}</span>
			{#if footer.when}<span class="sync-when">· {footer.when}</span>{/if}
		</div>
		<div class="sync-sub" title={footer.where}>{footer.where}</div>
		{#if footer.encryption}<div class="sync-sub">{footer.encryption}</div>{/if}
		{#if footer.error}<div class="sync-sub error">{footer.error}</div>{/if}
	</div>
</aside>
