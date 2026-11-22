<script lang="ts">
	// Who is part of the project: people of the vault (shared/people), each with the role they have here. They are
	// offered first wherever someone is named in this project (who gave feedback, who was at a meeting), and "Whole team"
	// adds every teammate at once. Someone typed who isn't in the vault yet is added to it. Each change is saved at once.
	import Inbox from '@lucide/svelte/icons/inbox';
	import Plus from '@lucide/svelte/icons/plus';
	import Users from '@lucide/svelte/icons/users';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import Dropdown, { type DropdownOption } from '$lib/components/Dropdown.svelte';
	import { attachedOf, findByName, personNamed, withPerson, type Attached } from '$lib/people';
	import { attempt, refresh } from '$lib/ui.svelte';
	import type { FeedbackRole, Person, ProjectPerson } from '$lib/types';
	import { ROLES } from '$lib/v1';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project.slug);
	const roleOptions: DropdownOption[] = ROLES.map((r) => ({ value: r.role, label: r.name }));
	const listId = `vault-people-${Math.random().toString(36).slice(2)}`;

	let everyone = $state<Person[]>([]);
	const loadEveryone = () =>
		api
			.listPeople()
			.then((list) => (everyone = list))
			.catch(() => {});
	loadEveryone();

	let name = $state('');
	let role = $state<FeedbackRole>('teammate');
	let busy = $state(false);

	/** The people of the vault who aren't part of this project yet: what the name field offers. */
	const others = $derived(everyone.filter((p) => !data.people.some((q) => q.person === p.id)));
	const known = $derived(findByName(everyone, name));

	/** By role, in the order the roles are listed (coach first), each group by name. */
	const groups = $derived(
		ROLES.map((r) => ({
			...r,
			people: data.people.filter((p) => p.role === r.role).sort((a, b) => a.name.localeCompare(b.name))
		})).filter((g) => g.people.length)
	);

	async function store(people: Attached[]) {
		busy = true;
		const saved = await attempt(() => api.saveProjectPeople(project, people));
		busy = false;
		if (saved) await refresh();
		return !!saved;
	}

	async function add() {
		if (!name.trim() || busy) return;
		busy = true;
		const person = await attempt(() => personNamed(everyone, name));
		busy = false;
		if (!person) return;
		if (await store(withPerson(attachedOf(data.people), person.id, role))) {
			name = '';
			await loadEveryone();
		}
	}

	const setRole = (person: ProjectPerson, next: string) => next !== person.role && store(withPerson(attachedOf(data.people), person.person, next as FeedbackRole));

	const remove = (person: ProjectPerson) => store(attachedOf(data.people).filter((p) => p.person !== person.person));
</script>

<svelte:head><title>People | {data.project.name} | Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner">
		<section class="panel">
			<div class="panel-head">
				<h2>Who is part of this project</h2>
				<a class="btn small ghost" href="/people"><Users size={14} />Everyone in the vault</a>
			</div>
			<p class="sub" style="margin:0">
				Its coach, teammates, client and anyone else, from the people of your vault: the same person in every project they are part of, here with the
				role they have in this one. They are offered first when you note who gave feedback or who was at a meeting, and <strong>Whole team</strong> adds every
				teammate at once.
			</p>

			<div class="form-row">
				<label class="field grow">
					Name
					<input
						bind:value={name}
						list={others.length ? listId : undefined}
						placeholder={others.length ? 'Pick someone from the vault, or type a new name' : 'e.g. Frank de Vries'}
						autocomplete="off"
						onkeydown={(e) => e.key === 'Enter' && add()}
					/>
					{#if others.length}
						<datalist id={listId}>
							{#each others as p (p.id)}<option value={p.name}>{p.organisation || 'In the vault'}</option>{/each}
						</datalist>
					{/if}
				</label>
				<div class="field" style="width:200px">
					Role
					<Dropdown bind:value={() => role, (v) => (role = v as FeedbackRole)} options={roleOptions} label="Role" full />
				</div>
				<button class="btn primary" style="align-self:flex-end" onclick={add} disabled={busy || !name.trim()}><Plus size={15} />Add</button>
			</div>
			{#if name.trim() && !known}
				<div class="hint">{name.trim()} isn't in the vault yet: adding them here adds them to your people too.</div>
			{/if}
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
						{#each g.people as person (person.person)}
							<li>
								<a class="people-name" href="/people?person={encodeURIComponent(person.person)}">{person.name}</a>
								{#if person.portflowUser !== null}<span class="badge" title="Someone in Portflow">Portflow</span>{/if}
								<span class="spacer-h"></span>
								<a class="btn small ghost" href="/feedback?from={encodeURIComponent(person.tag)}" title="The feedback {person.name} gave"><Inbox size={14} />Feedback</a>
								<div style="width:160px">
									<Dropdown bind:value={() => person.role, (v) => setRole(person, v)} options={roleOptions} label="Role of {person.name}" disabled={busy} full />
								</div>
								<button class="icon-btn" aria-label="Remove {person.name} from the project" title="Remove from the project (they stay in the vault)" disabled={busy} onclick={() => remove(person)}>
									<X size={15} />
								</button>
							</li>
						{/each}
					</ul>
				</section>
			{/each}
		{/if}
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
		color: inherit;
	}
</style>
