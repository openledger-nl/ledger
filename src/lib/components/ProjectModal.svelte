<script lang="ts">
	// Starting a project means choosing a goal and an end; everything else hangs from it. Also used to set the kind,
	// goal and end of a project made before v1. Wireframe B1; requirements: Projects.
	import { goto } from '$app/navigation';
	import { api } from '$lib/api';
	import { projectUrl } from '$lib/actions';
	import { mmmInfo } from '$lib/mmm';
	import { currentSemester, dayLabel, nearestSemester, today, workDays } from '$lib/semesters';
	import { attempt, refresh, toast } from '$lib/ui.svelte';
	import { daysLeft, daysLeftText, endDay, PROJECT_KINDS, ROLES } from '$lib/v1';
	import type { Client, FeedbackRole, MmmLevel, Person, ProjectKind, ProjectMeta, Semester } from '$lib/types';
	import { personNamed, withPerson, type Attached } from '$lib/people';
	import X from '@lucide/svelte/icons/x';
	import UserPlus from '@lucide/svelte/icons/user-plus';
	import Dropdown from './Dropdown.svelte';
	import DateInput from './DateInput.svelte';
	import Modal from './Modal.svelte';
	import MmmPicker from './MmmPicker.svelte';

	let {
		project = null,
		onclose
	}: {
		/** The project to set the facts of; null makes a new one. */
		project?: { slug: string; name: string; meta: ProjectMeta | null } | null;
		onclose: () => void;
	} = $props();

	// The form starts from the project once; edits stay in the form until saved.
	// svelte-ignore state_referenced_locally
	const m = project?.meta;
	// svelte-ignore state_referenced_locally
	let name = $state(project?.name ?? '');
	let kind = $state<ProjectKind>(m?.kind ?? 'study');
	let goal = $state(m?.goal ?? '');
	let endMode = $state<'semester' | 'date' | 'none'>(m ? (m.end === 'semester' ? 'semester' : m.end ? 'date' : 'none') : 'semester');
	let endDate = $state(m?.end && m.end !== 'semester' ? m.end : '');
	let intended = $state<MmmLevel | ''>(m?.intendedMmm ?? '');
	let mainQuestion = $state(m?.mainQuestion ?? '');
	// A work project's client: one of the vault's clients, by name; someone new is added to the vault on saving.
	// svelte-ignore state_referenced_locally
	let client = $state(m?.clientName ?? '');
	let clients = $state<Client[]>([]);
	api.listClients().then((list) => (clients = list), () => {});
	const clientListId = `project-clients-${Math.random().toString(36).slice(2)}`;
	/** The client's id: the one called `client`, or a new one made now. Null without a name. */
	async function clientId(): Promise<string | null> {
		const name = client.trim();
		if (kind !== 'work' || !name) return null;
		const known = clients.find((c) => c.name.trim().toLowerCase() === name.toLowerCase());
		return known?.id ?? (await api.createClient({ name, website: '', notes: '' })).id;
	}
	let busy = $state(false);

	// Its people: the coach, the teammates, the stakeholders, from the people of the vault (someone new is added to it on
	// saving). Kept with the project whether or not it has a goal yet.
	type Listed = { person: string | null; name: string; role: FeedbackRole };
	let people = $state<Listed[]>([]);
	let everyone = $state<Person[]>([]);
	api.listPeople().then((list) => (everyone = list), () => {});
	// svelte-ignore state_referenced_locally
	if (project) api.listProjectPeople(project.slug).then((list) => (people = list.map(({ person, name, role }) => ({ person, name, role }))), () => {});
	let newName = $state('');
	let newRole = $state<FeedbackRole>('teammate');
	const roleOptions = ROLES.map((r) => ({ value: r.role, label: r.name }));
	const peopleListId = `project-people-${Math.random().toString(36).slice(2)}`;
	function addPerson() {
		const name = newName.trim();
		if (!name) return;
		const known = everyone.find((p) => p.name.trim().toLowerCase() === name.toLowerCase());
		people = [...people.filter((p) => !known || p.person !== known.id), { person: known?.id ?? null, name: known?.name ?? name, role: newRole }];
		newName = '';
	}

	/** The list as it is saved: someone new is added to the vault first. */
	async function attached(): Promise<Attached[]> {
		let list: Attached[] = [];
		for (const p of people) {
			const id = p.person ?? (await personNamed(everyone, p.name)).id;
			list = withPerson(list, id, p.role);
		}
		return list;
	}

	let semesters = $state<Semester[]>([]);
	api.listSemesters().then((s) => (semesters = s), () => {});
	const semester = $derived(currentSemester(semesters) ?? nearestSemester(semesters));

	// Only a study project can end with the semester; the others start with a date of their own.
	$effect(() => {
		if (kind !== 'study' && endMode === 'semester') endMode = 'date';
	});

	const meta = $derived<Omit<ProjectMeta, 'slug' | 'tag'>>({
		kind,
		goal: goal.trim(),
		end: endMode === 'semester' ? 'semester' : endMode === 'date' ? endDate || null : null,
		semester: kind === 'study' ? (semester?.name ?? null) : null,
		intendedMmm: intended || null,
		mainQuestion: mainQuestion.trim() || null,
		client: null
	});
	const end = $derived(endDay({ ...meta, slug: '', tag: '' }, semesters));
	const left = $derived(daysLeft(end));
	const work = $derived(end && left !== null && left > 0 ? workDays(today(), end, semester?.vacations ?? []) : null);
	const ready = $derived(name.trim() !== '' && goal.trim() !== '' && (endMode !== 'date' || endDate !== '') && (endMode !== 'semester' || !!semester));

	async function save() {
		if (!ready || busy) return;
		busy = true;
		let slug = project?.slug;
		if (!project) {
			const created = await attempt(() => api.createProject(name.trim()));
			slug = created?.slug;
		} else if (name.trim() !== project.name) {
			const renamed = project.slug;
			await attempt(() => api.renameProject(renamed, name.trim()));
		}
		if (slug) {
			const s = slug;
			await attempt(async () => api.saveProjectMeta({ ...meta, client: await clientId(), slug: s, tag: s }));
			await attempt(async () => api.saveProjectPeople(s, await attached()));
		}
		busy = false;
		if (!slug) return;
		onclose();
		await refresh();
		if (!project) {
			toast('Project created');
			await goto(projectUrl(slug));
		}
	}
</script>

<Modal title={project ? 'Goal, kind and end' : 'New project'} wide {onclose}>
	<label class="field">
		<span>Name</span>
		<input bind:value={name} placeholder="e.g. Thrym Player" data-autofocus={project ? undefined : ''} />
	</label>

	<div class="field">
		<span>Kind</span>
		<div class="seg" role="group" aria-label="Kind of project" style="align-self:flex-start">
			{#each PROJECT_KINDS as k (k.kind)}
				<button class:on={kind === k.kind} aria-pressed={kind === k.kind} onclick={() => (kind = k.kind)}>{k.name}</button>
			{/each}
		</div>
		<span class="hint">{PROJECT_KINDS.find((k) => k.kind === kind)?.hint} A group project is a study project in your own vault.</span>
	</div>

	<label class="field">
		<span>Goal <span class="req">required</span></span>
		<textarea bind:value={goal} rows="2" placeholder="What must this project achieve, and for whom? e.g. Give the bakery a rota tool their staff can use on a phone." data-autofocus={project ? '' : undefined}></textarea>
		<span class="hint">The most important thing a project has: shown on every project screen and fed to every deliverable.</span>
	</label>

	<div class="field">
		<span>End</span>
		<div class="pm-ends">
			{#if kind === 'study'}
				<label class="check">
					<input type="radio" bind:group={endMode} value="semester" disabled={!semester} />
					{#if semester}End of the semester ({semester.name}, {dayLabel(semester.end)}){:else}End of the semester (set a semester up in Settings first){/if}
				</label>
			{/if}
			<div class="pm-own">
				<label class="check"><input type="radio" bind:group={endMode} value="date" /> A date of your own</label>
				<span class="pm-date" onfocusin={() => (endMode = 'date')}><DateInput bind:value={endDate} label="End date" min={today()} /></span>
			</div>
			{#if kind !== 'study'}
				<label class="check"><input type="radio" bind:group={endMode} value="none" /> No end date</label>
			{/if}
		</div>
		<span class="hint">
			{daysLeftText(left)}{#if work !== null}, {work} work days{/if}.
		</span>
	</div>

	<div class="field">
		<span>Intended AI-use label (optional)</span>
		<MmmPicker bind:value={intended} label="Intended AI-use label" />
		<span class="hint">
			{#if intended === 'man-made'}
				Man-Made: every feature in which AI writes is switched off for this project. You can always change it.
			{:else if intended}
				{mmmInfo(intended)?.meaning} Offered when you publish a version; a different label is pointed out, never blocked.
			{:else}
				The label you mean the work to get. Offered when you publish a version, and compared with what you publish.
			{/if}
		</span>
	</div>

	{#if kind === 'study'}
		<label class="field">
			<span>Main question (optional)</span>
			<input bind:value={mainQuestion} placeholder="e.g. How can a music player make offline listening as easy as streaming?" />
		</label>
	{:else if kind === 'work'}
		<label class="field">
			<span>Client (optional)</span>
			<input bind:value={client} list={clients.length ? clientListId : undefined} placeholder="e.g. Bakkerij De Korenaar" autocomplete="off" />
			{#if clients.length}
				<datalist id={clientListId}>
					{#each clients as c (c.id)}<option value={c.name}>In the vault</option>{/each}
				</datalist>
			{/if}
			<span class="hint">One of the clients of your vault (under Shared, Clients); a new name is added to it.</span>
		</label>
	{/if}

	<div class="field">
		<span>People (optional)</span>
		{#if people.length}
			<ul class="pm-people">
				{#each people as p, i (i)}
					<li>
						<span class="pm-person">{p.name}</span>
						<span class="badge">{ROLES.find((r) => r.role === p.role)?.name ?? p.role}</span>
						<button class="icon-btn" aria-label="Remove {p.name}" title="Remove" onclick={() => (people = people.filter((_, j) => j !== i))}><X size={14} /></button>
					</li>
				{/each}
			</ul>
		{/if}
		<div class="form-row pm-add-person">
			<input
				bind:value={newName}
				list={everyone.length ? peopleListId : undefined}
				placeholder="Name, e.g. Frank de Vries"
				aria-label="Name of someone in the project"
				autocomplete="off"
				onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), addPerson())}
			/>
			{#if everyone.length}
				<datalist id={peopleListId}>
					{#each everyone.filter((p) => !people.some((q) => q.person === p.id)) as p (p.id)}<option value={p.name}>In the vault</option>{/each}
				</datalist>
			{/if}
			<Dropdown bind:value={() => newRole, (v) => (newRole = v as FeedbackRole)} options={roleOptions} label="Their role" />
			<button class="btn small" onclick={addPerson} disabled={!newName.trim()}><UserPlus size={14} />Add</button>
		</div>
		<span class="hint">Your coach, teammates and stakeholders, from the people of your vault (someone new is added to it): offered when you note feedback or who was in a meeting, where "Whole team" adds every teammate at once.</span>
	</div>

	{#snippet footer()}
		<span class="spacer-h"></span>
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={save} disabled={!ready || busy}>{project ? 'Save' : 'Create project'}</button>
	{/snippet}
</Modal>
