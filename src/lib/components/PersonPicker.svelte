<script lang="ts">
	// Who gave a piece of feedback, and how. The project's own people come first (its coach, teammates, client, with
	// their role); with Portflow on, the people your portfolio is shared with follow (teachers first, with their role).
	// Anyone else can be typed as a name.
	import { GIVEN_WAYS } from '$lib/v1';
	import { onMount } from 'svelte';
	import { api, portflowApi } from '$lib/api';
	import { portflow } from '$lib/portflow.svelte';
	import { roleName } from '$lib/v1';
	import type { FeedbackFrom, PortflowPerson, ProjectPerson } from '$lib/types';
	import Dropdown, { type DropdownOption } from './Dropdown.svelte';

	let { value = $bindable(), project = null }: { value: FeedbackFrom; project?: string | null } = $props();

	let people = $state<PortflowPerson[]>([]);
	let team = $state<ProjectPerson[]>([]);
	const GIVEN: DropdownOption[] = [
		{ value: '', label: '—' },
		...GIVEN_WAYS.map((g) => ({ value: g.given, label: g.name }))
	];
	const id = `people-${Math.random().toString(36).slice(2)}`;

	onMount(async () => {
		if (project) team = await api.listProjectPeople(project).catch(() => []);
		if (!portflow.enabled) return;
		try {
			people = (await portflowApi.people()).filter((p) => p.active);
		} catch {
			people = [];
		}
	});

	function typed(name: string) {
		const key = name.trim().toLowerCase();
		const own = team.find((p) => p.name.toLowerCase() === key);
		const match = people.find((p) => p.name.toLowerCase() === key || (own?.portflowUser != null && p.id === own.portflowUser));
		value = {
			...value,
			name: name || null,
			portflowUser: own?.portflowUser ?? match?.id ?? null,
			role: own ? roleName(own.role).toLowerCase() : match?.role || null,
			given: value.given ?? (name.trim() ? 'in person' : null)
		};
	}
</script>

<div class="form-row">
	<label class="field grow">
		From
		<input
			list={people.length || team.length ? id : undefined}
			value={value.name ?? ''}
			oninput={(e) => typed(e.currentTarget.value)}
			placeholder={team.length || people.length ? 'Pick someone from the project, or type a name' : 'Who gave it (optional)'}
			autocomplete="off"
		/>
		{#if people.length || team.length}
			<datalist {id}>
				{#each team as p (p.tag)}<option value={p.name}>{roleName(p.role)} in this project</option>{/each}
				{#each people.filter((p) => !team.some((t) => t.name.toLowerCase() === p.name.toLowerCase())) as p (p.id)}<option value={p.name}>{p.role}</option>{/each}
			</datalist>
		{/if}
	</label>
	<div class="field">
		How
		<!-- The empty string is "not said"; it is stored as null. -->
		<Dropdown
			bind:value={() => value.given ?? '', (v) => v !== (value.given ?? '') && (value = { ...value, given: v || null })}
			options={GIVEN}
			label="How it was given"
			full
		/>
	</div>
</div>
{#if value.name && team.some((p) => p.name.toLowerCase() === value.name?.trim().toLowerCase())}
	<div class="hint">{value.name} is {value.role ? `the project’s ${value.role}` : 'in this project'}.</div>
{:else if value.portflowUser && value.role}
	<div class="hint">{value.name} is a {value.role} in your Portflow portfolio.</div>
{/if}
