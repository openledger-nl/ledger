<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import FileText from '@lucide/svelte/icons/file-text';
	import House from '@lucide/svelte/icons/house';
	import NotebookPen from '@lucide/svelte/icons/notebook-pen';
	import Inbox from '@lucide/svelte/icons/inbox';
	import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
	import Plus from '@lucide/svelte/icons/plus';
	import Settings from '@lucide/svelte/icons/settings';
	import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { prettyKeyOf } from '$lib/commands.svelte';
	import { deliverableTarget, deliverableUrl, newDeliverableFlow, newProjectFlow, newThreadFlow, projectUrl, threadUrl } from '$lib/actions';
	import { orderThreads } from '$lib/order';
	import { listOrder } from '$lib/order.svelte';
	import { layout, toggleSidebar } from '$lib/layout.svelte';
	import { portflow, portflowFooter } from '$lib/portflow.svelte';
	import { openVaultPicker } from '$lib/vaults.svelte';
	import type { DeliverableSummary, ProjectSummary } from '$lib/types';
	import EpsilonIcon from '$lib/components/EpsilonIcon.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';

	/** `vaultName`: the open vault, on the switch button at the bottom. */
	let { projects, vaultName = '' }: { projects: ProjectSummary[]; vaultName?: string } = $props();

	const OPEN_KEY = 'ledger-open-project';
	const sidebarKey = $derived(prettyKeyOf('sidebar.toggle'));

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

	// Every project's deliverables, by project, for the unfolded project's list. They are looked up again whenever the
	// vault changes (the projects are loaded afresh then).
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

	// "just now" and "5 min ago" are worked out when drawn, so the footer redraws every half minute to keep them true.
	let tick = $state(0);
	$effect(() => {
		const timer = setInterval(() => tick++, 30_000);
		return () => clearInterval(timer);
	});

	const pf = $derived.by(() => {
		void tick;
		return portflowFooter(portflow.status);
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
		<a class="icon-btn side-settings" class:active={on('/settings')} href="/settings" title="Settings" aria-label="Settings"><Settings size={18} /></a>
		<button class="icon-btn side-hide" title={sidebarKey ? `Hide sidebar (${sidebarKey})` : 'Hide sidebar'} aria-label="Hide sidebar" onclick={toggleSidebar}>
			<PanelLeftClose size={18} />
		</button>
	</div>

	<a class="nav-item" class:active={path === '/'} href="/"><span class="nav-ico"><House size={16} /></span>Home</a>
	{#if portflow.enabled}
		<a class="nav-item" class:active={on('/epsilon')} href="/epsilon"><span class="nav-ico"><EpsilonIcon size={16} /></span>Epsilon+</a>
	{/if}
	<a class="nav-item" class:active={on('/feedback')} href="/feedback"><span class="nav-ico"><Inbox size={16} /></span>Feedback</a>
	<a class="nav-item" class:active={on('/meetings')} href="/meetings"><span class="nav-ico"><NotebookPen size={16} /></span>Meeting notes</a>

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

	<!-- The bottom left: how the Portflow sync stands (the details are in Settings and in the tooltip), and the button
	     that switches vaults. -->
	<div class="sync-box">
		{#if portflow.enabled}
			<a class="sync-line" href="/settings/sync#portflow" title={[pf.canvas, pf.error].filter(Boolean).join(' | ')}>
				<span class="dot {pf.dot}"></span><span class="sync-title">{pf.title}</span>
				{#if pf.when}<span class="sync-when">{pf.when}</span>{/if}
			</a>
		{/if}
		<button class="vault-switch" onclick={openVaultPicker} title="Switch vault">
			<span class="vault-switch-name">{vaultName || 'Vault'}</span><ChevronsUpDown size={14} class="vault-switch-chev" />
		</button>
	</div>

</aside>
