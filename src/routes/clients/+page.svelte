<script lang="ts">
	// Every client the vault knows: the organisations work projects are done for, kept in shared/clients/ and shared by
	// every project. A work project picks its client in its settings; here clients are added, changed and taken out.
	import { goto } from '$app/navigation';
	import Building2 from '@lucide/svelte/icons/building-2';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { api } from '$lib/api';
	import { projectUrl } from '$lib/actions';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import { matches } from '$lib/threadSearch.svelte';
	import type { Client, ClientDraft } from '$lib/types';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	let query = $state('');
	const shown = $derived(data.clients.filter((c) => matches([c.name, c.website, ...c.projects.map((p) => p.projectName)].join(' '), query)));

	/** The client being edited: its id, or 'new'. */
	const selected = $derived(data.open);
	const client = $derived(data.clients.find((c) => c.id === selected) ?? null);

	const blank = (): ClientDraft => ({ name: '', website: '', notes: '' });
	const draftOf = (c: Client): ClientDraft => ({ name: c.name, website: c.website, notes: c.notes });

	let draft = $state<ClientDraft>(blank());
	let editing = $state<string | null>(null);
	$effect(() => {
		// The form follows the client chosen; edits stay in it until saved.
		const key = selected ?? null;
		if (key === editing) return;
		editing = key;
		draft = client ? draftOf(client) : blank();
	});

	let busy = $state(false);
	const open = (id: string | null) => goto(id ? `/clients?client=${encodeURIComponent(id)}` : '/clients', { keepFocus: true, noScroll: true });

	async function save() {
		if (!draft.name.trim() || busy) return;
		busy = true;
		const saved = await attempt(() => (client ? api.updateClient(client.id, draft) : api.createClient(draft)));
		busy = false;
		if (!saved) return;
		toast(client ? 'Saved' : `${saved.name} is in the vault now`);
		editing = saved.id;
		await open(saved.id);
		await refresh();
	}

	async function remove(c: Client) {
		const where = c.projects.length ? ` Its projects (${c.projects.map((p) => p.projectName).join(', ')}) keep going without a client.` : '';
		const ok = await confirmDialog({
			title: `Take ${c.name} out of the vault?`,
			message: `Its file in shared/clients goes.${where}`,
			confirmLabel: 'Take out',
			danger: true
		});
		if (!ok) return;
		if ((await attempt(() => api.deleteClient(c.id))) === undefined) return;
		await open(null);
		await refresh();
	}
</script>

<svelte:head><title>Clients | Ledger</title></svelte:head>

<div class="header-wrap flat">
	<div class="header-top">
		<div>
			<h1>Clients</h1>
			<div class="sub">The organisations your work projects are for, shared by every project. The people who speak for them are a project's stakeholders.</div>
		</div>
		<div class="header-actions">
			<button class="btn primary" onclick={() => open('new')}><Plus size={16} />New client</button>
		</div>
	</div>
</div>

<div class="page">
	<div class="page-inner">
		<div class="cl-split">
			<div class="cl-list">
				<input class="cl-search" bind:value={query} placeholder="Find a client by name or project" aria-label="Find a client" />
				{#if data.clients.length === 0}
					<div class="empty-state">
						<h1>No clients yet</h1>
						<p>Add the organisations you work for, then pick one in a work project's settings.</p>
					</div>
				{:else if shown.length === 0}
					<p class="hint">Nothing matches "{query.trim()}".</p>
				{:else}
					<ul aria-label="Clients">
						{#each shown as c (c.id)}
							<li>
								<button class="mt-row" class:on={c.id === selected} onclick={() => open(c.id)}>
									<span class="mt-title">{c.name}</span>
									<span class="mt-sub">{c.projects.length ? c.projects.map((p) => p.projectName).join(' | ') : 'No project yet'}</span>
								</button>
							</li>
						{/each}
					</ul>
				{/if}
			</div>

			{#if selected}
				<section class="panel cl-edit" aria-label={client ? client.name : 'New client'}>
					<div class="panel-head">
						<h2>{client ? client.name : 'New client'}</h2>
						{#if client}
							<button class="btn small ghost danger" onclick={() => remove(client)}><Trash2 size={14} />Take out of the vault</button>
						{/if}
					</div>
					<label class="field">
						Name
						<input bind:value={draft.name} placeholder="e.g. Bakkerij De Korenaar" autocomplete="off" data-autofocus={client ? undefined : ''} />
					</label>
					<label class="field">
						Website (optional)
						<input type="url" bind:value={draft.website} placeholder="https://" autocomplete="off" />
					</label>
					<div class="field">
						Notes (optional)
						<MarkdownEditor bind:value={draft.notes} label="Notes about the client" placeholder="What to remember about them" rows={4} />
					</div>
					{#if client}
						<div class="field">
							Projects
							{#if client.projects.length}
								<ul class="cl-projects">
									{#each client.projects as p (p.project)}
										<li><a href={projectUrl(p.project)}>{p.projectName}</a></li>
									{/each}
								</ul>
							{:else}
								<span class="hint">No project yet: pick this client in a work project's settings.</span>
							{/if}
						</div>
					{/if}
					<div class="row-end">
						<button class="btn ghost" onclick={() => open(null)}>Close</button>
						<button class="btn primary" onclick={save} disabled={busy || !draft.name.trim()}>{client ? 'Save' : 'Add to the vault'}</button>
					</div>
				</section>
			{:else if data.clients.length}
				<div class="cl-none hint"><Building2 size={18} />Pick a client to see and change what the vault knows about it.</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.cl-split {
		display: grid;
		grid-template-columns: minmax(220px, 320px) 1fr;
		gap: 20px;
		align-items: start;
	}
	@media (max-width: 860px) {
		.cl-split {
			grid-template-columns: 1fr;
		}
	}
	.cl-list ul {
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.cl-search {
		width: 100%;
	}
	.cl-edit {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.cl-projects {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.cl-none {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 24px;
	}
</style>
