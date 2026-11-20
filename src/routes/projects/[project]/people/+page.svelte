<script lang="ts">
	// The project's people: its coach, teammates, client and the rest, kept in its project.md. They are offered first
	// wherever someone is named in this project (who gave feedback, who was at a meeting), and "Whole team" adds every
	// teammate at once. Each change is saved straight away.
	import Inbox from '@lucide/svelte/icons/inbox';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import Dropdown, { type DropdownOption } from '$lib/components/Dropdown.svelte';
	import ProjectBranding from '$lib/components/ProjectBranding.svelte';
	import { attempt, refresh } from '$lib/ui.svelte';
	import type { FeedbackRole, ProjectPerson } from '$lib/types';
	import { ROLES } from '$lib/v1';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project.slug);
	const roleOptions: DropdownOption[] = ROLES.map((r) => ({ value: r.role, label: r.name }));

	let name = $state('');
	let role = $state<FeedbackRole>('teammate');
	let busy = $state(false);

	/** By role, in the order the roles are listed (coach first), each group by name. */
	const groups = $derived(
		ROLES.map((r) => ({
			...r,
			people: data.people.filter((p) => p.role === r.role).sort((a, b) => a.name.localeCompare(b.name))
		})).filter((g) => g.people.length)
	);

	async function store(people: Omit<ProjectPerson, 'project' | 'tag'>[]) {
		busy = true;
		const saved = await attempt(() => api.saveProjectPeople(project, people));
		busy = false;
		if (saved) await refresh();
		return !!saved;
	}

	const plain = (list: ProjectPerson[]) => list.map(({ name, role, portflowUser }) => ({ name, role, portflowUser }));

	async function add() {
		const n = name.trim();
		if (!n || busy) return;
		if (await store([...plain(data.people), { name: n, role, portflowUser: null }])) name = '';
	}

	const setRole = (person: ProjectPerson, next: string) =>
		next !== person.role && store(plain(data.people).map((p, i) => (data.people[i] === person ? { ...p, role: next as FeedbackRole } : p)));

	const remove = (person: ProjectPerson) => store(plain(data.people).filter((_, i) => data.people[i] !== person));

	// For the PDFs: who they are for when no name is given (a company project's client, else the first client listed),
	// and the other projects to take your side from.
	const clientName = $derived(data.meta?.client ?? data.people.find((p) => p.role === 'client')?.name ?? null);
	const others = $derived(data.projects.filter((p) => p.slug !== project).map((p) => ({ slug: p.slug, name: p.name })));
</script>

<svelte:head><title>People | {data.project.name} | Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner">
		<section class="panel">
			<div class="panel-head">
				<h2>Who is part of this project</h2>
			</div>
			<p class="sub" style="margin:0">
				Its coach, teammates, client and anyone else. They are offered first when you note who gave feedback or who was at a meeting, and
				<strong>Whole team</strong> adds every teammate at once. Kept in the project's <code>project.md</code>.
			</p>

			<div class="form-row">
				<label class="field grow">
					Name
					<input bind:value={name} placeholder="e.g. Frank de Vries" autocomplete="off" onkeydown={(e) => e.key === 'Enter' && add()} />
				</label>
				<div class="field" style="width:200px">
					Role
					<Dropdown bind:value={() => role, (v) => (role = v as FeedbackRole)} options={roleOptions} label="Role" full />
				</div>
				<button class="btn primary" style="align-self:flex-end" onclick={add} disabled={busy || !name.trim()}><Plus size={15} />Add</button>
			</div>
		</section>

		{#if groups.length === 0}
			<div class="empty-state">
				<h1>No people yet</h1>
				<p>Add your coach, your teammates and the client above.</p>
			</div>
		{:else}
			{#each groups as g (g.role)}
				<section class="panel">
					<h3 class="people-head">{g.people.length > 1 && g.role !== 'other' ? `${g.name}s` : g.name} <span class="count">{g.people.length}</span></h3>
					<ul class="people-list">
						{#each g.people as person (person)}
							<li>
								<span class="people-name">{person.name}</span>
								{#if person.portflowUser !== null}<span class="badge" title="Someone in Portflow">Portflow</span>{/if}
								<span class="spacer-h"></span>
								<a class="btn small ghost" href="/feedback?from={encodeURIComponent(person.tag)}" title="The feedback {person.name} gave"><Inbox size={14} />Feedback</a>
								<div style="width:160px">
									<Dropdown bind:value={() => person.role, (v) => setRole(person, v)} options={roleOptions} label="Role of {person.name}" disabled={busy} full />
								</div>
								<button class="icon-btn" aria-label="Remove {person.name}" title="Remove from the project" disabled={busy} onclick={() => remove(person)}>
									<X size={15} />
								</button>
							</li>
						{/each}
					</ul>
				</section>
			{/each}
		{/if}

		<ProjectBranding {project} {others} client={clientName} study={data.meta?.kind === 'study'} />
	</div>
</div>

<style>
	.people-head {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 0;
		font-size: 14px;
	}
	.people-head .count {
		color: var(--faint);
		font-weight: 500;
	}
	.people-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
	}
	.people-list li {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 0;
		border-top: 1px solid var(--line);
		flex-wrap: wrap;
	}
	.people-list li:first-child {
		border-top: 0;
	}
	.people-name {
		font-weight: 600;
	}
</style>
