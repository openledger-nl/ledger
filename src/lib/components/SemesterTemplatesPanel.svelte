<script lang="ts">
	// The semester templates (shared/templates/semesters): what a new semester can start from on the Semesters page. A
	// new vault starts with NJ26; each is the vault's to change or delete.
	import { spanLabel } from '$lib/semesters';
	import type { SemesterTemplate } from '$lib/types';
	import SemesterModal from './SemesterModal.svelte';
	import TemplateRow from './TemplateRow.svelte';

	let { templates }: { templates: SemesterTemplate[] } = $props();

	/** `undefined` is closed; `null` makes a new one. */
	let editing = $state<SemesterTemplate | null | undefined>(undefined);

	const about = (t: SemesterTemplate) =>
		[spanLabel(t.start, t.end), `${t.sprints.length} sprints`, t.vacations.length ? `${t.vacations.length} ${t.vacations.length === 1 ? 'vacation' : 'vacations'}` : '']
			.filter(Boolean)
			.join(' | ');
</script>

<div class="panel">
	<div class="header-top">
		<h2>Semester templates</h2>
		<button class="btn primary small" onclick={() => (editing = null)}>New semester template</button>
	</div>
	<p class="sub" style="margin:0">
		What a new semester can start from on the Semesters page: its days, vacations and sprints. A semester made from one is its own, and stays as it is
		when the template changes.
	</p>
	{#if templates.length === 0}
		<div class="hint">None yet. Make one here, or save a semester as a template on the Semesters page.</div>
	{:else}
		{#each templates as t (t.id)}
			<TemplateRow name={t.name} about={about(t)} onedit={() => (editing = t)} />
		{/each}
	{/if}
</div>

{#if editing !== undefined}
	<SemesterModal semester={editing} existing={[]} template onclose={() => (editing = undefined)} />
{/if}
