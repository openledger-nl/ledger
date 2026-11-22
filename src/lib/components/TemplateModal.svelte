<script lang="ts">
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import type { ProjectKind, ReaderPreset, Template, TemplateReader } from '$lib/types';
	import { PROJECT_KINDS } from '$lib/v1';
	import MarkdownEditor from './MarkdownEditor.svelte';
	import Modal from './Modal.svelte';
	import Switch from './Switch.svelte';

	/** `template` is null when creating one. The modal is mounted fresh each time, so it only reads the initial values. */
	let { scope, template, onclose }: { scope: string; template: Template | null; onclose: () => void } = $props();

	function start() {
		return {
			name: template?.name ?? '',
			trigger: template?.trigger ?? '',
			questions: template ? [...template.questions] : [''],
			body: template?.body ?? '',
			kinds: template ? [...template.kinds] : [],
			readers: template ? template.readers.map((r) => ({ ...r })) : [],
			readersShown: template?.readersShown ?? false
		};
	}
	const initial = start();

	let name = $state(initial.name);
	let trigger = $state(initial.trigger);
	let questions = $state<string[]>(initial.questions);
	let body = $state(initial.body);
	let kinds = $state<ProjectKind[]>(initial.kinds);
	let readers = $state<TemplateReader[]>(initial.readers);
	let readersShown = $state(initial.readersShown);
	// The skeleton opens in source mode: its %% notes to the writer are only seen there.
	let sourceMode = $state(true);
	let someone = $state('');
	let saving = $state(false);

	let presets = $state<ReaderPreset[]>([]);
	$effect(() => {
		api.templateReaderPresets().then((p) => (presets = p)).catch(() => {});
	});
	const presetOf = (who: string) => presets.find((p) => p.id === who);
	const unused = $derived(presets.filter((p) => !readers.some((r) => r.who === p.id)));

	function move(i: number, by: -1 | 1) {
		const j = i + by;
		if (j < 0 || j >= questions.length) return;
		[questions[i], questions[j]] = [questions[j], questions[i]];
	}

	function toggleKind(kind: ProjectKind) {
		kinds = kinds.includes(kind) ? kinds.filter((k) => k !== kind) : PROJECT_KINDS.map((k) => k.kind).filter((k) => k === kind || kinds.includes(k));
	}
	const kindsHint = $derived(
		kinds.length === 0 || kinds.length === PROJECT_KINDS.length
			? 'Offered in every project. Pick kinds to offer it only in those.'
			: `Offered only in ${PROJECT_KINDS.filter((k) => kinds.includes(k.kind))
					.map((k) => k.name.toLowerCase())
					.join(' and ')} projects.`
	);

	function addReader(who: string) {
		who = who.trim();
		if (!who || readers.some((r) => r.who.toLowerCase() === who.toLowerCase())) return;
		readers.push({ who, needs: '', keepFrom: '' });
	}
	function addSomeone() {
		addReader(someone);
		someone = '';
	}

	async function save() {
		saving = true;
		const draft = { name, trigger, questions, body, kinds, readers, readersShown };
		const saved = await attempt(() => (template ? api.updateTemplate(scope, template.id, draft) : api.createTemplate(scope, draft)));
		saving = false;
		if (!saved) return;
		toast(template ? 'Template saved' : 'Template created');
		await refresh();
		onclose();
	}

	async function remove() {
		if (!template) return;
		const ok = await confirmDialog({
			title: `Delete "${template.name}"?`,
			message: `Prompts already started from it keep the answers they were given.${scope ? ' If a shared template has the same name, this project uses that one again.' : ''}`,
			confirmLabel: 'Delete template',
			danger: true
		});
		if (!ok) return;
		if ((await attempt(() => api.deleteTemplate(scope, template.id))) !== undefined) {
			await refresh();
			onclose();
		}
	}
</script>

<Modal title={template ? `Edit ${template.name}` : scope ? 'New project template' : 'New template'} wide {onclose}>
	<label class="field">
		Name
		<input bind:value={name} placeholder="e.g. Interview summary" />
	</label>
	<label class="field">
		Use when...
		<input bind:value={trigger} placeholder="e.g. you have just spoken to a customer" />
	</label>
	{#if !scope}
		<div class="field">
			<span>For</span>
			<div class="seg small" role="group" aria-label="Kinds of project">
				{#each PROJECT_KINDS as k (k.kind)}
					<button class:on={kinds.includes(k.kind)} aria-pressed={kinds.includes(k.kind)} title={k.hint} onclick={() => toggleKind(k.kind)}>{k.name}</button>
				{/each}
			</div>
			<span class="hint">{kindsHint}</span>
		</div>
	{/if}
	<div class="field">
		<span>Readers</span>
		{#if readers.length > 0}
			<div class="q-list">
				{#each readers as r, i (r.who)}
					<div class="reader">
						<div class="reader-head">
							<b>{presetOf(r.who)?.name ?? r.who}</b>
							{#if presetOf(r.who)}<span class="hint">{presetOf(r.who)?.hint}</span>{/if}
							<span class="spacer-h"></span>
							<button class="icon-btn" style="color:var(--muted)" aria-label="Remove {presetOf(r.who)?.name ?? r.who}" onclick={() => readers.splice(i, 1)}><X size={16} /></button>
						</div>
						<div class="reader-fields">
							<input bind:value={r.needs} placeholder="What they need from it (optional)" aria-label="What {presetOf(r.who)?.name ?? r.who} needs" />
							<input bind:value={r.keepFrom} placeholder="Keep from them (optional)" aria-label="What to keep from {presetOf(r.who)?.name ?? r.who}" />
						</div>
					</div>
				{/each}
			</div>
		{/if}
		<div class="chips">
			{#each unused as p (p.id)}
				<button class="chip plain" title={p.hint} onclick={() => addReader(p.id)}><Plus size={12} />{p.name}</button>
			{/each}
			<input
				class="someone"
				bind:value={someone}
				placeholder="Someone else"
				aria-label="Add a reader in your own words"
				onkeydown={(e) => {
					if (e.key === 'Enter') {
						e.preventDefault();
						addSomeone();
					}
				}}
			/>
			{#if someone.trim()}<button class="btn small ghost" onclick={addSomeone}>Add</button>{/if}
		</div>
		<div class="shown">
			<Switch checked={readersShown} label="Show the readers in the document" onchange={(next) => (readersShown = next)} />
			<span>
				Show the readers in the document
				<span class="hint">
					{readersShown
						? 'A new deliverable gets a "Readers:" line under its title, and a %% note for Claude.'
						: 'Only a %% note under the title, seen in source mode: Claude reads it, readers of the document don\'t.'}
				</span>
			</span>
		</div>
		<span class="hint">Claude writes the deliverable for them: their words, the detail they need, and nothing that is kept from them.</span>
	</div>
	<div class="field">
		<span>Guiding questions</span>
		<div class="q-list">
			{#each questions as _, i (i)}
				<div class="q-row">
					<MarkdownEditor bind:value={questions[i]} rows={2} label="Guiding question {i + 1}" placeholder="A question to answer, or skip, when a deliverable is started" />
					<div class="btns">
						<button class="icon-btn" style="color:var(--muted)" aria-label="Move up" disabled={i === 0} onclick={() => move(i, -1)}><ArrowUp size={16} /></button>
						<button class="icon-btn" style="color:var(--muted)" aria-label="Move down" disabled={i === questions.length - 1} onclick={() => move(i, 1)}><ArrowDown size={16} /></button>
						<button class="icon-btn" style="color:var(--muted)" aria-label="Remove question" onclick={() => questions.splice(i, 1)}><X size={16} /></button>
					</div>
				</div>
			{/each}
		</div>
		<div><button class="btn small ghost" onclick={() => questions.push('')}><Plus size={14} />Add question</button></div>
	</div>
	<div class="field">
		<div class="skeleton-head">
			<span>Deliverable skeleton</span>
			<span class="spacer-h"></span>
			<div class="seg small" role="group" aria-label="Editor mode">
				<button class:on={sourceMode} aria-pressed={sourceMode} onclick={() => (sourceMode = true)}>Source</button>
				<button class:on={!sourceMode} aria-pressed={!sourceMode} onclick={() => (sourceMode = false)}>Live preview</button>
			</div>
		</div>
		<MarkdownEditor
			bind:value={body}
			variant="doc"
			source={sourceMode}
			minHeight="160px"
			label="Deliverable skeleton"
			placeholder={'# Title\n\n## Section\n%% What to write here, and why %%'}
			spellcheck={false}
		/>
		<span class="hint">
			New deliverables created from this template start with this text. Write notes to the writer between %% marks: they are only seen in
			source mode, and Claude follows them without copying them.
		</span>
	</div>

	{#snippet footer()}
		{#if template}
			<button class="btn ghost danger" onclick={remove}>Delete</button>
		{/if}
		<span class="spacer-h"></span>
		<button class="btn ghost" onclick={onclose}>Cancel</button>
		<button class="btn primary" onclick={save} disabled={saving || !name.trim()}>Save</button>
	{/snippet}
</Modal>

<style>
	.reader {
		border: 1px solid var(--line);
		border-radius: 8px;
		padding: 8px 10px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.reader-head {
		display: flex;
		align-items: baseline;
		gap: 8px;
		min-width: 0;
	}
	.reader-head .hint {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}
	.reader-fields {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
	}
	.reader-fields input {
		flex: 1 1 14em;
		min-width: 0;
	}
	.chips button.chip {
		cursor: pointer;
	}
	.someone {
		flex: 0 1 12em;
		min-width: 8em;
		padding: 3px 10px;
		font-size: 12px;
		border-radius: 999px;
	}
	.shown {
		display: flex;
		gap: 10px;
		align-items: flex-start;
		margin-top: 4px;
	}
	.shown > span {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.skeleton-head {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
