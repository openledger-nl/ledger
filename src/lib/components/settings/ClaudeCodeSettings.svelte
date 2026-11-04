<script lang="ts">
	// The settings screen's panel for running prompts with an AI harness. Its settings live in the vault's
	// .ledger/claude-code.json (never synced), apart from the rest of the settings.
	import { onMount } from 'svelte';
	import { open } from '@tauri-apps/plugin-dialog';
	import { claudeCodeApi } from '$lib/api';
	import { harnesses, harnessLabel } from '$lib/harness';
	import { attempt, toast } from '$lib/ui.svelte';
	import type { ClaudeCodeCheck, ClaudeCodeSettings, ClaudePermissionMode, HarnessProvider } from '$lib/types';

	const modes: { id: ClaudePermissionMode; label: string; hint: string }[] = [
		{
			id: 'acceptEdits',
			label: 'Edit files',
			hint: 'It reads the project and creates or edits files inside the project’s folder, which is what writing a deliverable needs. Other commands and network access are refused: a run is not interactive, so nothing can approve them.'
		},
		{
			id: 'plan',
			label: 'Plan only',
			hint: 'It reads the project and answers with a plan. Nothing is changed. Good for looking before a real run.'
		},
		{
			id: 'dontAsk',
			label: 'Read only',
			hint: 'It reads the project. Anything that would need approval, every edit included, is refused.'
		}
	];

	let saved = $state<ClaudeCodeSettings | null>(null);
	let cliPath = $state('');
	let provider = $state<HarnessProvider>('claude');
	let effort = $state('');
	const harness = $derived(harnesses.find((h) => h.id === provider)!);
	const efforts = $derived(provider === 'claude' ? ['low', 'medium', 'high', 'xhigh', 'max'] : provider === 'codex' ? ['minimal', 'low', 'medium', 'high', 'xhigh'] : provider === 'agy' ? ['low', 'medium', 'high', 'xhigh', 'max'] : ['off', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max']);
	let mode = $state<ClaudePermissionMode>('acceptEdits');
	let model = $state('');
	let confine = $state(true);
	let autoRun = $state(false);
	let busy = $state<'save' | 'check' | null>(null);
	let checked = $state<ClaudeCodeCheck | null>(null);
	let error = $state<string | null>(null);

	const changed = $derived(
		saved !== null && (provider !== saved.provider || effort !== saved.effort || cliPath.trim() !== saved.cliPath || mode !== saved.permissionMode || model.trim() !== saved.model || confine !== saved.confine || autoRun !== saved.autoRun)
	);

	function show(settings: ClaudeCodeSettings) {
		saved = settings;
		provider = settings.provider;
		effort = settings.effort;
		cliPath = settings.cliPath;
		mode = settings.permissionMode;
		model = settings.model;
		confine = settings.confine;
		autoRun = settings.autoRun;
	}

	onMount(async () => {
		const settings = await attempt(() => claudeCodeApi.settings());
		if (settings) show(settings);
	});

	async function browse() {
		const chosen = await attempt(() => open({ multiple: false, directory: false, title: `Find the ${harness.program} program` }));
		if (typeof chosen === 'string') {
			cliPath = chosen;
			checked = null;
		}
	}

	async function check() {
		busy = 'check';
		checked = null;
		try {
			checked = await claudeCodeApi.check(cliPath, provider);
		} catch (e) {
			checked = { ok: false, version: '', path: null, message: e instanceof Error ? e.message : String(e) };
		}
		busy = null;
	}

	async function save() {
		busy = 'save';
		error = null;
		try {
			show(await claudeCodeApi.saveSettings(cliPath, mode, model, confine, autoRun, provider, effort));
			toast('AI harness settings saved');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
		busy = null;
	}
</script>

<div class="panel">
	<h2>AI harness</h2>
	<p class="sub" style="margin:0">
		Run prompts with an installed CLI in the project’s folder. The harness uses its own authentication, model access and billing;
		Ledger does not supply inference. Review the prompt and command before starting. Deliverables it writes are published and linked to the prompt,
		and a run record is kept in the thread’s <code>runs</code> folder and synced with the vault.
	</p>

	<label class="field">
		Harness
		<select bind:value={provider} onchange={() => { cliPath = ''; model = ''; effort = ''; checked = null; confine = true; if (mode === 'trusted') mode = 'acceptEdits'; }}>
			{#each harnesses as h (h.id)}<option value={h.id}>{h.label}</option>{/each}
		</select>
		<span class="hint">Changing harness clears the executable, model and effort fields and restores the guard-rail requirement. Save to use it for new runs; existing records keep their original harness.</span>
	</label>
	<label class="field">
		The {harness.program} program
		<div class="form-row" style="flex-wrap:nowrap">
			<input class="mono" bind:value={cliPath} oninput={() => (checked = null)} placeholder={`Blank: find ${harness.program} on the PATH`} spellcheck="false" autocomplete="off" />
			<button class="btn" onclick={browse}>Browse…</button>
		</div>
	</label>
	{#if saved && provider === saved.provider && cliPath.trim() === saved.cliPath}
		<div class="hint">
			{#if saved.resolvedPath}
				With the saved setting, Ledger runs <span class="mono">{saved.resolvedPath}</span>
			{:else}
				{saved.problem}
			{/if}
		</div>
	{/if}

	{#if provider !== 'prime'}
	<div class="field">
		What it may do
		<div class="seg" role="group" aria-label="What the AI harness may do" style="align-self:flex-start">
			{#each modes as m (m.id)}
				<button class:on={mode === m.id} aria-pressed={mode === m.id} disabled={provider === 'agy' && m.id === 'dontAsk'} onclick={() => (mode = m.id)}>{m.label}</button>
			{/each}
		</div>
		<span class="hint">
			{#if provider === 'claude'}{modes.find((m) => m.id === mode)?.hint}
			{:else if provider === 'codex'}Edit files requests Codex’s workspace-write sandbox. Plan only and Read only both use its read-only sandbox; they do not enforce a planning workflow. On native Windows, Codex requires a configured Windows sandbox; otherwise it may remain read-only.
			{:else}AGY uses its accept-edits or plan policy. Read only is not supported. Unapproved actions may be refused without failing the run.{/if}
		</span>
	</div>

	{/if}

	<label class="field" style="max-width:320px">
		Model
		<input bind:value={model} placeholder={`Blank: the default of ${harnessLabel(provider)}`} spellcheck="false" autocomplete="off" />
		<span class="hint">A model name accepted by {harnessLabel(provider)} and available to your account.</span>
	</label>

	<label class="field" style="max-width:320px">
		Reasoning effort
		<select bind:value={effort}>
			<option value="">Harness default</option>
			{#each efforts as level (level)}<option value={level}>{level}</option>{/each}
		</select>
		<span class="hint">Passed to the selected CLI. Support can depend on its version and model.</span>
	</label>

	{#if provider === 'claude'}
	<label class="check">
		<input type="checkbox" bind:checked={confine} /> Extra guard rails (recommended)
	</label>
	<div class="hint">
		Passes <code>--settings</code> (a file you can read in the confirmation before each run), <code>--setting-sources user</code> and
		<code>--strict-mcp-config</code>: reads and writes outside the project folder are refused, and so are the vault’s <code>.ledger</code> folder and
		any <code>.env</code> file; the project folder’s own Claude settings (hooks, say) are ignored, and no MCP servers are started, so files that arrive
		in the vault through sync can’t run anything. It needs a recent Claude Code: if a run fails with “unknown option”, update Claude Code or turn this
		off. There is no setting that skips Claude Code’s permission checks.
	</div>

	{:else if provider === 'prime'}
	<label class="check">
		<input type="checkbox" checked={!confine && mode === 'trusted'} onchange={(e) => { confine = !e.currentTarget.checked; mode = e.currentTarget.checked ? 'trusted' : 'acceptEdits'; }} />
		Allow Prime Agent native tool access with my user permissions
	</label>
	<div class="hint">Ledger starts Prime Agent with <code>--mode acp</code> and sends prompts through the Agent Client Protocol.</div>
	<div class="notice bad">
		Prime Agent is not sandboxed. It can execute model-generated Python and commands with your user permissions,
		including access outside the project. Ledger cannot enforce file-only, read-only or plan-only permissions for it.
		Runs are blocked until you explicitly allow this access. The Trusted mode grants native tool access, not file-only access.
	</div>
	{:else}
	<label class="check">
		<input type="checkbox" bind:checked={confine} /> Require Claude Code’s extra guard rails
	</label>
	<div class="notice info">
		{harnessLabel(provider)} cannot use Claude Code’s extra guard rails. Runs are blocked while this requirement is checked.
		Uncheck it only if you accept this harness’s different permissions and sandbox. A project working folder is not a security boundary;
		your CLI configuration may allow access to other files, hooks, tools or network services.
	</div>
	{/if}

	<label class="check">
		<input type="checkbox" bind:checked={autoRun} /> Run a prompt as soon as it is created
	</label>
	<div class="hint">
		Off by default. When it is on, saving a new prompt starts it in the selected harness at once, so you don’t press <b>Run with AI harness</b> and don’t see the
		confirmation of what will be sent first. It runs with the permissions chosen above and uses your harness account and allowance, and it starts before any guiding
		questions from a template are answered. You can still cancel it from the prompt’s card, and a prompt you already have still needs the button.
	</div>

	{#if error}<div class="notice bad">{error}</div>{/if}
	{#if checked}
		<div class="notice" class:ok={checked.ok} class:bad={!checked.ok}>
			{checked.message}
			{#if checked.path}<div class="mono" style="margin-top:4px">{checked.path}</div>{/if}
		</div>
	{/if}

	<div class="row-end" style="justify-content:flex-start">
		<button class="btn" onclick={check} disabled={busy !== null}>{busy === 'check' ? 'Checking…' : 'Check'}</button>
		<button class="btn primary" onclick={save} disabled={busy !== null || !changed}>Save</button>
	</div>
	{#if saved}
		<div class="hint">Settings file: <span class="mono">{saved.settingsFile}</span> · at most {saved.maxConcurrent} runs at once</div>
	{/if}
</div>
