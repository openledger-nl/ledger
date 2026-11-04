<script lang="ts">
	// Who gave a piece of feedback, and how. With Portflow on, the people your portfolio is shared with are offered
	// (teachers first, with their role); anyone else can be typed as a name. With Portflow off it is a typed name.
	import { onMount } from 'svelte';
	import { portflowApi } from '$lib/api';
	import { portflow } from '$lib/portflow.svelte';
	import type { FeedbackFrom, PortflowPerson } from '$lib/types';

	let { value = $bindable() }: { value: FeedbackFrom } = $props();

	let people = $state<PortflowPerson[]>([]);
	const id = `people-${Math.random().toString(36).slice(2)}`;

	onMount(async () => {
		if (!portflow.enabled) return;
		try {
			people = (await portflowApi.people()).filter((p) => p.active);
		} catch {
			people = [];
		}
	});

	function typed(name: string) {
		const match = people.find((p) => p.name.toLowerCase() === name.trim().toLowerCase());
		value = {
			...value,
			name: name || null,
			portflowUser: match?.id ?? null,
			role: match?.role || null,
			given: value.given ?? (name.trim() ? 'in person' : null)
		};
	}
</script>

<div class="form-row">
	<label class="field grow">
		From
		<input
			list={people.length ? id : undefined}
			value={value.name ?? ''}
			oninput={(e) => typed(e.currentTarget.value)}
			placeholder={people.length ? 'Pick someone with access, or type a name' : 'Who gave it (optional)'}
			autocomplete="off"
		/>
		{#if people.length}
			<datalist {id}>
				{#each people as p (p.id)}<option value={p.name}>{p.role}</option>{/each}
			</datalist>
		{/if}
	</label>
	<label class="field">
		How
		<select value={value.given ?? ''} onchange={(e) => (value = { ...value, given: e.currentTarget.value || null })}>
			<option value="">—</option>
			<option value="in person">In person</option>
			<option value="email">By email</option>
			<option value="other">Otherwise</option>
		</select>
	</label>
</div>
{#if value.portflowUser && value.role}
	<div class="hint">{value.name} is a {value.role} in your Portflow portfolio.</div>
{/if}
