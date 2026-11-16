<script lang="ts">
	// The bar over every page: the sidebar button, where you are, search and Capture. Wireframes A1, A4, A6 and A7.
	// - full width: the search box with its shortcut; the sidebar button only once the sidebar is folded (A4).
	// - half width: search is an icon that opens the box over the bar; the sidebar button is a menu that slides the
	//   whole sidebar in over the page (beside the icon rail, if folding makes one) (A6).
	// - narrow: as half, and Capture is an icon (A7).
	// Ctrl K (⌘K) goes to search at every width, Ctrl \ (⌘\) folds the sidebar.
	import Camera from '@lucide/svelte/icons/camera';
	import Menu from '@lucide/svelte/icons/menu';
	import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
	import Search from '@lucide/svelte/icons/search';
	import X from '@lucide/svelte/icons/x';
	import { tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { projectUrl, threadUrl } from '$lib/actions';
	import { prettyHotkey } from '$lib/hotkey';
	import { layout, toggleSidebar } from '$lib/layout.svelte';
	import { attempt } from '$lib/ui.svelte';
	import { groups } from '$lib/components/settings/panels';

	interface Crumb {
		label: string;
		href?: string;
	}

	const searchKey = prettyHotkey('CommandOrControl+K');
	const sidebarKey = prettyHotkey('CommandOrControl+\\');

	// ---- where you are -----------------------------------------------------------------------------

	const tabs: Record<string, string> = {
		threads: 'Threads',
		deliverables: 'Deliverables',
		stories: 'Stories',
		board: 'Board',
		templates: 'Templates'
	};

	/** The trail to the open page, from the route and what its load functions found (project, thread, deliverable). */
	const crumbs = $derived.by((): Crumb[] => {
		const id = page.route.id ?? '';
		const path = page.url.pathname;
		const data = page.data as {
			project?: { name: string };
			thread?: { title: string };
			deliverable?: { name: string; evidence: string | null };
			projects?: { slug: string; threads: { slug: string; title: string }[] }[];
		};
		if (id === '/') return [{ label: 'Home' }];
		if (id.startsWith('/epsilon')) return [{ label: 'Epsilon+' }];
		if (id.startsWith('/search')) return [{ label: 'Search' }];
		if (id.startsWith('/meetings')) return [{ label: 'Meeting notes' }];
		if (id.startsWith('/feedback')) return [{ label: 'Feedback' }];
		if (id.startsWith('/settings')) {
			const group = groups.find((g) => path.startsWith(`/settings/${g.id}`));
			return group ? [{ label: 'Settings', href: '/settings' }, { label: group.label }] : [{ label: 'Settings' }];
		}
		if (id.startsWith('/projects/[project]')) {
			const slug = page.params.project ?? '';
			const base = projectUrl(slug);
			const project = { label: data.project?.name ?? slug, href: base };
			const tab = id.split('/')[3];
			if (!tab || !tabs[tab]) return [{ label: project.label }];
			const list: Crumb = { label: tabs[tab], href: `${base}/${tab}` };
			if (page.params.thread) return [project, list, { label: data.thread?.title ?? page.params.thread }];
			if (id.endsWith('/deliverables/view')) {
				// Its name without the file's extension; opened from a thread (`?thread=<slug>`), the trail goes through it.
				const d = data.deliverable;
				const name = { label: d ? (d.evidence ? d.name : d.name.replace(/\.[^./]+$/, '') || d.name) : 'Deliverable' };
				const from = page.url.searchParams.get('thread');
				const thread = from ? data.projects?.find((p) => p.slug === slug)?.threads.find((t) => t.slug === from) : undefined;
				return thread ? [project, { label: thread.title, href: threadUrl(slug, thread.slug) }, name] : [project, list, name];
			}
			return [project, { label: list.label }];
		}
		return [];
	});

	// ---- search ------------------------------------------------------------------------------------

	let query = $state('');
	let box = $state<HTMLInputElement | null>(null);
	/** Where the box is drawn: in the bar when there is room, otherwise over it, opened from the icon. */
	const compact = $derived(layout.width !== 'full');

	// An open search shows its words in the box, so refining it is one edit away; not while typing, when the box is
	// ahead of the address.
	$effect(() => {
		const q = page.url.searchParams.get('q') ?? '';
		if (page.url.pathname === '/search' && document.activeElement !== box) query = q;
	});

	// Search runs as you type: a moment after the last key, the results page follows the box. The first search from
	// another page is a step of its own (Back returns there); refining it replaces the step.
	let timer: ReturnType<typeof setTimeout> | undefined;
	function search(now = false) {
		clearTimeout(timer);
		const run = () => {
			const q = query.trim();
			const onSearch = page.url.pathname === '/search';
			if (!q && !onSearch) return;
			if (onSearch && (page.url.searchParams.get('q') ?? '') === q) return;
			goto(q ? `/search?q=${encodeURIComponent(q)}` : '/search', { replaceState: onSearch, keepFocus: true, noScroll: true });
		};
		if (now) run();
		else timer = setTimeout(run, 150);
	}

	async function openSearch() {
		if (compact) layout.searchOpen = true;
		await tick();
		box?.focus();
		box?.select();
	}

	function closeSearch() {
		layout.searchOpen = false;
	}

	function submit(e: SubmitEvent) {
		e.preventDefault();
		search(true);
	}

	function onKey(e: KeyboardEvent) {
		if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && (e.code === 'KeyK' || e.key.toLowerCase() === 'k')) {
			e.preventDefault();
			openSearch();
		}
	}

	// ---- the sidebar button ------------------------------------------------------------------------

	// At full width an unfolded sidebar has its own hide button, so the bar has none.
	const sidebarButton = $derived.by(() => {
		if (layout.width !== 'full') return { icon: Menu, label: 'Show sidebar', show: true };
		return { icon: PanelLeftOpen, label: 'Show sidebar', show: layout.sidebarHidden };
	});
</script>

<svelte:window onkeydown={onKey} />

<header class="topbar" class:searching={compact && layout.searchOpen}>
	{#if sidebarButton.show}
		<button class="tb-icon" title="{sidebarButton.label} ({sidebarKey})" aria-label={sidebarButton.label} onclick={toggleSidebar}>
			<sidebarButton.icon size={18} />
		</button>
	{/if}

	<nav class="tb-crumbs" aria-label="You are here">
		{#each crumbs as c, i (i)}
			{#if i > 0}<span class="tb-sep" aria-hidden="true">/</span>{/if}
			{#if c.href}
				<a class="tb-crumb" href={c.href}>{c.label}</a>
			{:else}
				<span class="tb-crumb current" aria-current="page">{c.label}</span>
			{/if}
		{/each}
	</nav>

	<div class="tb-actions">
		{#if !compact || layout.searchOpen}
			<form class="tb-search" role="search" onsubmit={submit}>
				<Search size={14} />
				<input
					bind:this={box}
					bind:value={query}
					oninput={() => search()}
					type="search"
					placeholder="Search everything"
					aria-label="Search everything"
					autocomplete="off"
					spellcheck="false"
					onkeydown={(e) => e.key === 'Escape' && (compact ? closeSearch() : box?.blur())}
					onblur={() => compact && !query.trim() && closeSearch()}
				/>
				{#if compact}
					<button type="button" class="tb-icon" aria-label="Close search" onclick={closeSearch}><X size={16} /></button>
				{:else}
					<kbd>{searchKey}</kbd>
				{/if}
			</form>
		{:else}
			<button class="tb-icon" title="Search ({searchKey})" aria-label="Search" onclick={openSearch}><Search size={18} /></button>
		{/if}

		<button class="btn small tb-capture" title="Take a screenshot for a thread" onclick={() => attempt(api.takeScreenshot)}>
			<Camera size={15} /><span class="tb-label">Capture</span>
		</button>
	</div>
</header>
