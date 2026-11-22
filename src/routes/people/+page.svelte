<script lang="ts">
	// Everyone the vault knows: coaches, teammates, clients and anyone else, kept in shared/people/ and shared by every
	// project. A person shows only in the projects they are attached to, with the role they have in each; that is set on
	// the project's People tab. Here they are added, changed and taken out of the vault.
	import { goto } from '$app/navigation';
	import Inbox from '@lucide/svelte/icons/inbox';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import Users from '@lucide/svelte/icons/users';
	import { api } from '$lib/api';
	import { projectUrl } from '$lib/actions';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import { matches } from '$lib/threadSearch.svelte';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import type { Person, PersonDraft } from '$lib/types';
	import { roleName } from '$lib/v1';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	let query = $state('');
	const shown = $derived(data.people.filter((p) => matches([p.name, p.organisation, p.email, ...p.projects.map((a) => a.projectName)].join(' '), query)));

	/** The person being edited: their id, or 'new'. */
	const selected = $derived(data.open);
	const person = $derived(data.people.find((p) => p.id === selected) ?? null);

	const blank = (): PersonDraft => ({ name: '', email: '', organisation: '', portflowUser: null, notes: '' });
	const draftOf = (p: Person): PersonDraft => ({ name: p.name, email: p.email, organisation: p.organisation, portflowUser: p.portflowUser, notes: p.notes });

	let draft = $state<PersonDraft>(blank());
	let editing = $state<string | null>(null);
	$effect(() => {
		// The form follows the person chosen; edits stay in it until saved.
		const key = selected ?? null;
		if (key === editing) return;
		editing = key;
		draft = person ? draftOf(person) : blank();
	});

	let busy = $state(false);
	const open = (id: string | null) => goto(id ? `/people?person=${encodeURIComponent(id)}` : '/people', { keepFocus: true, noScroll: true });

	async function save() {
		if (!draft.name.trim() || busy) return;
		busy = true;
		const saved = await attempt(() => (person ? api.updatePerson(person.id, draft) : api.createPerson(draft)));
		busy = false;
		if (!saved) return;
		toast(person ? 'Saved' : `${saved.name} is in the vault now`);
		editing = saved.id;
		await open(saved.id);
		await refresh();
	}

	async function remove(p: Person) {
		const where = p.projects.length ? ` and from ${p.projects.map((a) => a.projectName).join(', ')}` : '';
		const ok = await confirmDialog({
			title: `Take ${p.name} out of the vault?`,
			message: `Their file in shared/people goes${where}. Meetings and feedback that name them keep their name as it was written.`,
			confirmLabel: 'Take out',
			danger: true
		});
		if (!ok) return;
		if ((await attempt(() => api.deletePerson(p.id))) === undefined) return;
		await open(null);
		await refresh();
	}
</script>

<svelte:head><title>People | Ledger</title></svelte:head>

<div class="header-wrap flat">
	<div class="header-top">
		<div>
			<h1>People</h1>
			<div class="sub">Everyone in your vault, shared by every project. Someone shows only in the projects you add them to, with their role there.</div>
		</div>
		<div class="header-actions">
			<button class="btn primary" onclick={() => open('new')}><Plus size={16} />New person</button>
		</div>
	</div>
</div>

<div class="page">
	<div class="page-inner">
		<div class="pp-split">
			<div class="pp-list">
				<input class="pp-search" bind:value={query} placeholder="Find someone by name, organisation or project" aria-label="Find someone" />
				{#if data.people.length === 0}
					<div class="empty-state">
						<h1>No people yet</h1>
						<p>Add your coach, your teammates and your clients, then add them to the projects they are part of.</p>
					</div>
				{:else if shown.length === 0}
					<p class="hint">Nobody matches "{query.trim()}".</p>
				{:else}
					<ul aria-label="People">
						{#each shown as p (p.id)}
							<li>
								<button class="mt-row" class:on={p.id === selected} onclick={() => open(p.id)}>
									<span class="mt-title">{p.name}</span>
									<span class="mt-sub">
										{#if p.projects.length}{p.projects.map((a) => `${roleName(a.role)} in ${a.projectName}`).join(' | ')}{:else}In no project yet{/if}
									</span>
								</button>
							</li>
						{/each}
					</ul>
				{/if}
			</div>

			{#if selected}
				<section class="panel pp-edit" aria-label={person ? person.name : 'New person'}>
					<div class="panel-head">
						<h2>{person ? person.name : 'New person'}</h2>
						{#if person}
							<div class="row-end">
								<a class="btn small ghost" href="/feedback?from={encodeURIComponent(person.tag)}"><Inbox size={14} />Feedback</a>
								<button class="btn small ghost danger" onclick={() => remove(person)}><Trash2 size={14} />Take out of the vault</button>
							</div>
						{/if}
					</div>
					<label class="field">
						Name
						<input bind:value={draft.name} placeholder="e.g. Frank de Vries" autocomplete="off" data-autofocus={person ? undefined : ''} />
					</label>
					<div class="form-row">
						<label class="field grow">
							Organisation (optional)
							<input bind:value={draft.organisation} placeholder="e.g. Fontys ICT" autocomplete="off" />
						</label>
						<label class="field grow">
							Email (optional)
							<input type="email" bind:value={draft.email} placeholder="name@example.nl" autocomplete="off" />
						</label>
					</div>
					<div class="field">
						Notes (optional)
						<MarkdownEditor bind:value={draft.notes} label="Notes about them" placeholder="What to remember about them" rows={4} />
					</div>
					{#if person}
						<div class="field">
							Projects
							{#if person.projects.length}
								<ul class="pp-projects">
									{#each person.projects as a (a.project)}
										<li><a href={`${projectUrl(a.project)}/people`}>{a.projectName}</a> <span class="badge">{roleName(a.role)}</span></li>
									{/each}
								</ul>
							{:else}
								<span class="hint">In no project yet: add them on a project's People tab.</span>
							{/if}
						</div>
						<div class="hint">Their tag is <code>#{person.tag}</code>, in meeting notes and feedback; it stays when you rename them.</div>
					{/if}
					<div class="row-end">
						<button class="btn ghost" onclick={() => open(null)}>Close</button>
						<button class="btn primary" onclick={save} disabled={busy || !draft.name.trim()}>{person ? 'Save' : 'Add to the vault'}</button>
					</div>
				</section>
			{:else if data.people.length}
				<div class="pp-none hint"><Users size={18} />Pick someone to see and change what the vault knows about them.</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.pp-split {
		display: grid;
		grid-template-columns: minmax(220px, 320px) 1fr;
		gap: 20px;
		align-items: start;
	}
	@media (max-width: 860px) {
		.pp-split {
			grid-template-columns: 1fr;
		}
	}
	.pp-list ul {
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.pp-search {
		width: 100%;
	}
	.pp-edit {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.pp-projects {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.pp-none {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 24px;
	}
</style>
