<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import { afterNavigate } from '$app/navigation';
	import CaptureBar from '$lib/components/CaptureBar.svelte';
	import CommandPalette from '$lib/components/CommandPalette.svelte';
	import Dialogs from '$lib/components/Dialogs.svelte';
	import ScreenshotModal from '$lib/components/ScreenshotModal.svelte';
	import FileDrop from '$lib/components/FileDrop.svelte';
	import ProjectDialogs from '$lib/components/ProjectDialogs.svelte';
	import Rail from '$lib/components/Rail.svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import TopBar from '$lib/components/TopBar.svelte';
	import Toasts from '$lib/components/Toasts.svelte';
	import UpdateNotice from '$lib/components/UpdateNotice.svelte';
	import VaultPicker from '$lib/components/VaultPicker.svelte';
	import { initTheme, startBackendEvents } from '$lib/ui.svelte';
	import { layout, startLayout } from '$lib/layout.svelte';
	import { startCommands } from '$lib/commands.svelte';
	import { startUpdates } from '$lib/updates.svelte';
	import { startPortflow } from '$lib/portflow.svelte';
	import { captureDebug, startLogEvents } from '$lib/log.svelte';
	import { closeVaultPicker, vaultPicker } from '$lib/vaults.svelte';
	import type { LayoutData } from './$types';

	let { data, children }: { data: LayoutData; children: Snippet } = $props();

	// The screenshot overlay is a window of its own and shows no app chrome.
	const bare = $derived(page.url.pathname.startsWith('/screenshot-overlay'));
	// No vault yet (the first start): the vault picker is all there is.
	const noVault = $derived(!bare && data.vaults !== null && !data.vaults.open);
	/** The open vault's name, on the switch button at the bottom of the sidebar. */
	const vaultName = $derived(data.vaults?.vaults.find((v) => v.open)?.name ?? '');

	// A debug capture notes each page opened (its path: project and thread names are in it).
	afterNavigate(({ to }) => {
		if (to && !bare) captureDebug('page', to.url.pathname);
	});

	onMount(() => {
		if (bare) return;
		initTheme();
		const stopLayout = startLayout();
		const stopLog = startLogEvents();
		const stopEvents = startBackendEvents();
		const stopUpdates = startUpdates();
		const stopPortflow = startPortflow();
		const stopCommands = startCommands();
		return () => {
			stopCommands();
			stopPortflow();
			stopLog();
			stopEvents();
			stopUpdates();
			stopLayout();
		};
	});
</script>

{#if bare}
	{@render children()}
{:else if noVault && data.vaults}
	<VaultPicker vaults={data.vaults} />
	<Dialogs />
	<Toasts />
{:else}
	<!-- The shell: sidebar (or, folded, nothing or an icon rail), then the top bar over the page. How it folds with the window's width is
	     in layout.svelte.ts and the "shell" part of app.css. -->
	<div
		class="app fold-{layout.fold}"
		class:sidebar-hidden={layout.sidebarHidden}
		class:unfolding={layout.motion === 'unfold'}
		class:folding={layout.motion === 'fold'}
	>
		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
		<div class="scrim" class:open={layout.drawerOpen} onclick={() => (layout.drawerOpen = false)}></div>

		<Sidebar projects={data.projects} {vaultName} conflicts={data.conflictCount} />
		<Rail projects={data.projects} />
		<main>
			<TopBar />
			{@render children()}
		</main>
		<CaptureBar />
	</div>

	<ScreenshotModal projects={data.projects} />
	<FileDrop projects={data.projects} />
	<UpdateNotice />
	<ProjectDialogs />
	<CommandPalette />
	{#if vaultPicker.vaults}
		<VaultPicker vaults={vaultPicker.vaults} modal onclose={closeVaultPicker} />
	{/if}
	<Dialogs />
	<Toasts />
{/if}
