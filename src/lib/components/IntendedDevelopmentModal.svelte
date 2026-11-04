<script lang="ts">
	// Intended development: what the student aims for this semester. A level per layer (a whole row of Gamma's grid) and
	// per level-only goal (PS, PL), and the semester's overview: its learning plan (an evidence item, delivered in
	// Portflow) and its study guide. Kept in the semester's file; Gamma marks the rows aimed for and shows only that level's
	// description.
	import { untrack } from 'svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { portflowApi } from '$lib/api';
	import { attempt } from '$lib/ui.svelte';
	import type { EvidenceChoice, Gamma } from '$lib/types';

	let { view, semesterName, onclose, onsaved }: { view: Gamma; semesterName: string; onclose: () => void; onsaved: (view: Gamma) => void } = $props();

	// The five layers of HBO-i, in Fontys's order; those not in this semester's goal set are left out.
	const ALL_LAYERS = [
		['U', 'User Interaction'],
		['S', 'Software'],
		['O', 'Organisational Processes'],
		['H', 'Hardware Interfacing'],
		['I', 'Infrastructure']
	] as const;

	// The dialog starts from the goals as they are when it opens.
	const start = untrack(() => $state.snapshot(view.goals));
	let layers = $state<Record<string, string>>(Object.fromEntries(Object.entries(start.layers).map(([k, v]) => [k, String(v)])));
	let goals = $state<Record<string, string>>(Object.fromEntries(Object.entries(start.goals).map(([k, v]) => [k, String(v)])));
	let learningPlan = $state(start.learningPlan ?? '');
	let studyGuide = $state(start.studyGuide ?? '');
	let choices = $state<EvidenceChoice[]>([]);
	let saving = $state(false);

	$effect(() => {
		portflowApi.evidenceChoices().then(
			(all) => (choices = all),
			() => {}
		);
	});

	const shownLayers = $derived(ALL_LAYERS.filter(([l]) => view.rows.some((r) => r.layer === l) || layers[l]));
	const levels = $derived(Array.from({ length: Math.max(view.maxLevel, 3) }, (_, i) => i + 1));
	const problem = $derived(studyGuide.trim() && !/^https?:\/\//.test(studyGuide.trim()) ? 'The study guide is a web address (https://…)' : '');

	const numbers = (picked: Record<string, string>) =>
		Object.fromEntries(Object.entries(picked).filter(([, v]) => v).map(([k, v]) => [k, Number(v)]));

	async function save() {
		if (!view.semester) return;
		saving = true;
		const fresh = await attempt(() =>
			portflowApi.setIntended(view.semester!, {
				layers: numbers(layers),
				goals: numbers(goals),
				learningPlan: learningPlan || null,
				studyGuide: studyGuide.trim() || null
			})
		);
		saving = false;
		if (fresh) onsaved(fresh);
	}
</script>

<Modal title="Intended development · {semesterName}" wide {onclose}>
	<p class="hint intro">
		The level you aim for this semester. A layer counts for its whole row; Gamma keeps showing the whole grid, marks what you aim
		for and shows only that level's description.
	</p>

	<div class="targets">
		<div class="targets-col">
			<h3>Layers</h3>
			{#each shownLayers as [key, name] (key)}
				<label class="target">
					<span>{name}</span>
					<select bind:value={layers[key]}>
						<option value="">Not this semester</option>
						{#each levels as l (l)}<option value={String(l)}>Level {l}</option>{/each}
					</select>
				</label>
			{/each}
		</div>
		{#if view.skills.length}
			<div class="targets-col">
				<h3>Other goals</h3>
				{#each view.skills as skill (skill.outcome)}
					<label class="target">
						<span title={skill.name}>{skill.name}</span>
						<select bind:value={goals[skill.key]}>
							<option value="">Not this semester</option>
							{#each skill.levels as l (l.id)}<option value={String(l.level)}>Level {l.level}</option>{/each}
						</select>
					</label>
				{/each}
			</div>
		{/if}
	</div>

	<h3>Overview</h3>
	<label class="field">
		Learning plan
		<select bind:value={learningPlan}>
			<option value="">None</option>
			{#each choices as c (c.id)}<option value={c.id}>{c.name} · {c.projectName}</option>{/each}
			{#if learningPlan && !choices.some((c) => c.id === learningPlan)}<option value={learningPlan}>{learningPlan}</option>{/if}
		</select>
		<span class="hint">The evidence item you deliver in Portflow. Only items in a project linked to a collection are listed.</span>
	</label>
	<label class="field">
		Study guide
		<input bind:value={studyGuide} placeholder="https://…" />
	</label>

	{#snippet footer()}
		<span class="spacer-h"></span>
		{#if problem}<span class="hint error-text">{problem}</span>{/if}
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={save} disabled={saving || !!problem}>Save</button>
	{/snippet}
</Modal>

<style>
	.intro {
		margin: 0 0 14px;
	}
	h3 {
		font-size: 13px;
		margin: 0 0 8px;
		color: var(--muted);
		font-weight: 600;
	}
	.targets {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
		gap: 18px;
		margin-bottom: 18px;
	}
	.targets-col {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.target {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		font-size: 13px;
	}
	.target span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.target select {
		flex: none;
		width: 150px;
	}
</style>
