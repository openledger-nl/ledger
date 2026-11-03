<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import FileText from '@lucide/svelte/icons/file-text';
	import House from '@lucide/svelte/icons/house';
	import Plus from '@lucide/svelte/icons/plus';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { deliverableTarget, deliverableUrl, newDeliverableFlow, newProjectFlow, newThreadFlow, projectUrl, threadUrl } from '$lib/actions';
	import { ago } from '$lib/format';
	import { orderThreads } from '$lib/order';
	import { listOrder } from '$lib/order.svelte';
	import { drawer, sync } from '$lib/ui.svelte';
	import type { DeliverableSummary, ProjectSummary } from '$lib/types';
	import Wordmark from '$lib/components/Wordmark.svelte';

	let { projects }: { projects: ProjectSummary[] } = $props();

	const OPEN_KEY = 'ledger-open-project';

	let query = $state('');
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

	// Going to a project, from anywhere, unfolds it. Folding it again by hand sticks until another project is chosen.
	$effect(() => {
		const slug = currentProject;
		if (slug) untrack(() => unfold(slug));
	});

	// The open project's deliverables, looked up when it opens and again whenever the vault changes (the projects
	// are loaded afresh then).
	let deliverables = $state<{ slug: string; items: DeliverableSummary[] } | null>(null);
	$effect(() => {
		void projects;
		const slug = open;
		if (!slug) return;
		let stale = false;
		api.listDeliverables(slug)
			.then((items) => {
				if (!stale) deliverables = { slug, items: [...items].sort((a, b) => a.path.localeCompare(b.path)) };
			})
			.catch(() => {});
		return () => (stale = true);
	});

	// Reflect an open search in the box, so refining it is one edit away.
	$effect(() => {
		if (path === '/search') query = page.url.searchParams.get('q') ?? '';
	});

	// What the footer says about the vault and GitHub.
	const footer = $derived.by(() => {
		const s = sync.status;
		if (!s || !s.configured) return { dot: 'off', title: 'Local vault', sub: 'Not synced to GitHub' };
		// Next to where the files go, whether they go up sealed.
		const where = `${s.repo ?? ''} · ${s.encrypted ? 'encrypted' : 'not encrypted'}`;
		if (s.state === 'syncing') return { dot: 'warn pulse', title: 'Syncing…', sub: where };
		if (s.state === 'error') return { dot: 'bad', title: 'Sync problem', sub: s.lastError ?? 'See Settings', error: true };
		if (s.pending) return { dot: 'warn', title: 'Changes waiting to sync', sub: where };
		return { dot: 'ok', title: 'Synced with GitHub', sub: s.lastSyncAt ? `${where} · ${ago(s.lastSyncAt)}` : where };
	});

	function search(e: SubmitEvent) {
		e.preventDefault();
		const q = query.trim();
		if (q) goto(`/search?q=${encodeURIComponent(q)}`);
	}

	// On a narrow window the drawer closes after every choice (but not after folding a project).
	function closeDrawer(e: MouseEvent) {
		const target = (e.target as HTMLElement).closest('a, button');
		if (target && !target.classList.contains('folder-toggle')) drawer.open = false;
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<aside class="sidebar" class:open={drawer.open} onclick={closeDrawer}>
	<div class="brand"><Wordmark /></div>

	<form class="search-wrap" onsubmit={search}>
		<input type="search" placeholder="Search everything…" autocomplete="off" spellcheck="false" bind:value={query} />
	</form>

	<a class="nav-item" class:active={path === '/'} href="/"><span class="nav-ico"><House size={16} /></span>Home</a>

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
				<a class="folder-link" href={projectUrl(p.slug)} onclick={() => unfold(p.slug)}>
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
					{#if deliverables?.slug === p.slug}
						{#each deliverables.items as d (d.path)}
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
		<div class="sync-line"><span class="dot {footer.dot}"></span><span>{footer.title}</span></div>
		<div class="sync-sub" class:error={footer.error} title={footer.sub}>{footer.sub}</div>
		<a class="btn small" class:on={on('/settings')} href="/settings">Settings</a>
	</div>
</aside>
