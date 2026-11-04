<script lang="ts">
	// The guide on the home page for a new client: bring in a vault that exists, or set one up in four steps (a semester,
	// sync to GitHub, a first thread, a first deliverable). What a step needs is read from the vault, so it ticks itself off
	// when it is done, wherever it was done, and the guide ends with the first deliverable (onboarding.rs keeps the rest).
	import { goto } from '$app/navigation';
	import CalendarRange from '@lucide/svelte/icons/calendar-range';
	import Check from '@lucide/svelte/icons/check';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import CloudUpload from '@lucide/svelte/icons/cloud-upload';
	import FileText from '@lucide/svelte/icons/file-text';
	import FolderInput from '@lucide/svelte/icons/folder-input';
	import MessageSquareText from '@lucide/svelte/icons/message-square-text';
	import PartyPopper from '@lucide/svelte/icons/party-popper';
	import Rocket from '@lucide/svelte/icons/rocket';
	import { api } from '$lib/api';
	import { deliverableUrl } from '$lib/actions';
	import { nj26Draft } from '$lib/semesters';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import type { Component } from 'svelte';
	import type { OnboardingStatus, ProjectSummary, Semester, SemesterDraft, Settings } from '$lib/types';
	import OnboardingImport from './OnboardingImport.svelte';
	import SemesterModal from './SemesterModal.svelte';
	import EncryptionSettings from './settings/EncryptionSettings.svelte';
	import GitHubSettings from './settings/GitHubSettings.svelte';

	let {
		projects,
		semesters,
		deliverableCount,
		settings,
		onboarding
	}: {
		projects: ProjectSummary[];
		semesters: Semester[];
		/** How many deliverables the vault holds, in all its projects. */
		deliverableCount: number;
		/** Null until they are loaded. */
		settings: Settings | null;
		onboarding: OnboardingStatus;
	} = $props();

	type StepId = 'semester' | 'sync' | 'thread' | 'deliverable';
	interface Step {
		id: StepId;
		title: string;
		icon: Component<{ size?: number }>;
		done: boolean;
		skipped: boolean;
		/** What a finished step shows in place of its form. */
		summary: string;
	}

	/** "Set up a new vault" was chosen, or "Import" is open. */
	let started = $state(false);
	let importing = $state(false);
	/** A skipped step that is being done after all. */
	let revisiting = $state<StepId | null>(null);
	let editor = $state<{ preset?: SemesterDraft } | undefined>(undefined);
	let busy = $state(false);

	// The first thread: in a project that exists, or one made with it.
	let projectName = $state('');
	let threadTitle = $state('');
	let chosenProject = $state('');
	let madeProject = $state<string | null>(null);
	// The first deliverable
	let fileName = $state('');

	const project = $derived(projects.find((p) => p.slug === chosenProject) ?? projects[0]);
	const firstThread = $derived(projects.flatMap((p) => p.threads)[0]);

	const steps = $derived<Step[]>([
		{
			id: 'semester',
			title: 'Set up your semester',
			icon: CalendarRange,
			done: semesters.length > 0,
			skipped: onboarding.skipped.includes('semester'),
			summary: semesters[0]?.name ?? ''
		},
		{
			id: 'sync',
			title: 'Sync with GitHub',
			icon: CloudUpload,
			done: !!settings?.sync.configured,
			skipped: onboarding.skipped.includes('sync'),
			summary: settings?.sync.configured ? `${settings.sync.repo} · ${settings.sync.branch}` : ''
		},
		{
			id: 'thread',
			title: 'Start your first thread',
			icon: MessageSquareText,
			done: projects.some((p) => p.threads.length > 0),
			skipped: false,
			summary: firstThread?.title ?? ''
		},
		{
			id: 'deliverable',
			title: 'Create your first deliverable',
			icon: FileText,
			done: deliverableCount > 0,
			skipped: false,
			summary: ''
		}
	]);
	const passed = (s: Step) => s.done || s.skipped;
	const current = $derived(steps.find((s) => !passed(s)));
	const open = $derived(revisiting ?? current?.id ?? null);
	const finishedCount = $derived(steps.filter(passed).length);
	const welcome = $derived(!started && !importing && finishedCount === 0 && projects.length === 0);

	async function skip(step: 'semester' | 'sync') {
		revisiting = null;
		if (await attempt(() => api.skipOnboardingStep(step))) await refresh();
	}

	async function createThread() {
		busy = true;
		const slug = project?.slug ?? madeProject ?? (await attempt(() => api.createProject(projectName.trim())))?.slug;
		if (slug) {
			madeProject = slug;
			const made = await attempt(() => api.createThread(slug, threadTitle.trim()));
			if (made) {
				threadTitle = '';
				await refresh();
			}
		}
		busy = false;
	}

	async function createDeliverable() {
		if (!project) return;
		busy = true;
		const made = await attempt(() => api.createDeliverable(project.slug, fileName.trim()));
		if (made && (await attempt(() => api.finishOnboarding()))) {
			toast('You are all set up');
			await refresh();
			await goto(deliverableUrl(project.slug, made.path));
		}
		busy = false;
	}

	async function finish(thanks?: string) {
		if (!(await attempt(() => api.finishOnboarding()))) return;
		if (thanks) toast(thanks);
		await refresh();
	}

	async function hide() {
		const ok = await confirmDialog({
			title: 'Hide the setup guide?',
			message: 'What you have set up stays as it is. You can bring the guide back under Settings → Sync & external → Vault.',
			confirmLabel: 'Hide the guide'
		});
		if (ok) await finish();
	}
</script>

{#if importing && settings}
	<section class="onb" aria-label="Import a vault">
		<OnboardingImport
			{settings}
			{projects}
			{semesters}
			onback={() => (importing = false)}
			onempty={() => {
				importing = false;
				started = true;
			}}
			ondone={() => finish()}
		/>
	</section>
{:else if welcome}
	<section class="onb onb-welcome" aria-label="Welcome">
		<h2>Welcome to Ledger</h2>
		<p class="sub">
			Everything you write lives in a folder of markdown files, a vault, that you can open in any editor and sync to a private GitHub repository. Do you
			have a vault already?
		</p>
		<div class="onb-choices">
			<button class="onb-choice" onclick={() => (importing = true)} disabled={!settings}>
				<span class="onb-choice-icon"><FolderInput size={20} /></span>
				<strong>Import an existing vault</strong>
				<span class="onb-choice-sub">From a folder on this computer, or from the GitHub repository it is synced to.</span>
			</button>
			<button class="onb-choice primary" onclick={() => (started = true)}>
				<span class="onb-choice-icon"><Rocket size={20} /></span>
				<strong>Set up a new vault</strong>
				<span class="onb-choice-sub">Four short steps: a semester, sync to GitHub, a first thread and a first deliverable.</span>
			</button>
		</div>
	</section>
{:else}
	<section class="onb" aria-label="Set up your vault">
		<header class="onb-head">
			<div>
				<h2>Set up your vault</h2>
				<p class="sub">{current ? 'Four short steps, and you are ready to work.' : 'Everything is set up.'}</p>
			</div>
			<div class="onb-meta">
				<span class="badge accent">{finishedCount} of {steps.length}</span>
				<button class="link" onclick={hide}>Hide the guide</button>
			</div>
		</header>

		<ol class="onb-steps">
			{#each steps as step, i (step.id)}
				{@const Icon = step.icon}
				<li class="onb-step" class:done={step.done} class:skipped={step.skipped && !step.done} class:current={open === step.id}>
					<span class="onb-mark" aria-hidden="true">
						{#if step.done}<Check size={14} />{:else}{i + 1}{/if}
					</span>
					<div class="onb-main">
						<div class="onb-title">
							<Icon size={15} />
							<span>{step.title}</span>
							{#if step.skipped && !step.done}<span class="badge">Skipped</span>{/if}
							{#if step.skipped && !step.done && open !== step.id}
								<button class="link accent" onclick={() => (revisiting = step.id)}>Set it up now</button>
							{/if}
						</div>
						{#if step.done && step.summary}<div class="onb-summary">{step.summary}</div>{/if}

						{#if open === step.id && !step.done}
							<div class="onb-body">
								{#if step.id === 'semester'}
									<p>
										A semester gives the dashboard its calendar: the weeks, the vacations and the sprints that your work is counted in.
										Start from the NJ26 preset, or lay out your own.
									</p>
									<div class="onb-actions">
										<button class="btn primary" onclick={() => (editor = { preset: nj26Draft() })}>Use the NJ26 preset</button>
										<button class="btn" onclick={() => (editor = {})}>Create my own</button>
										<button class="btn ghost" onclick={() => skip('semester')}>Skip for now</button>
									</div>
								{:else if step.id === 'sync'}
									<p>
										Keep the vault in a <strong>private</strong> GitHub repository, so it survives this computer and follows you to others.
									</p>
									{#if settings}
										<details class="more">
											<summary><ChevronRight size={14} class="disclosure" />Encrypt it first</summary>
											<p class="hint">
												Encryption has to be on before the first sync: a repository that already holds your files in plain text is never
												converted. Names of files and folders are hidden as well.
											</p>
											<EncryptionSettings {settings} />
										</details>
										<GitHubSettings {settings} bare />
									{/if}
									<div class="onb-actions">
										<button class="btn ghost" onclick={() => skip('sync')}>Skip for now</button>
										<span class="hint">You can set it up later under Settings.</span>
									</div>
								{:else if step.id === 'thread'}
									<p>
										A <strong>project</strong> holds the work on one subject: a course, a client, a team. A <strong>thread</strong> is where you
										think it through, with posts, prompts and feedback.
									</p>
									<form
										class="onb-form"
										onsubmit={(e) => {
											e.preventDefault();
											void createThread();
										}}
									>
										{#if projects.length > 1}
											<label class="field">
												Project
												<select bind:value={chosenProject}>
													{#each projects as p (p.slug)}<option value={p.slug}>{p.name}</option>{/each}
												</select>
											</label>
										{:else if !project && !madeProject}
											<label class="field">
												Project
												<input bind:value={projectName} placeholder="e.g. Pricing overhaul" autocomplete="off" />
											</label>
										{/if}
										<label class="field">
											Thread
											<input bind:value={threadTitle} placeholder="e.g. What should we charge?" autocomplete="off" />
										</label>
										<div class="onb-actions">
											<button class="btn primary" type="submit" disabled={busy || !threadTitle.trim() || (!project && !madeProject && !projectName.trim())}>
												Create thread
											</button>
										</div>
									</form>
								{:else}
									<p>
										A <strong>deliverable</strong> is a file in the project that your prompts and feedback are about: a brief, a report, a plan.
										Give it a name; you write it in Ledger.
									</p>
									<form
										class="onb-form"
										onsubmit={(e) => {
											e.preventDefault();
											void createDeliverable();
										}}
									>
										{#if projects.length > 1}
											<label class="field">
												Project
												<select bind:value={chosenProject}>
													{#each projects as p (p.slug)}<option value={p.slug}>{p.name}</option>{/each}
												</select>
											</label>
										{/if}
										<label class="field">
											File name
											<input bind:value={fileName} placeholder="e.g. pricing-brief.md" autocomplete="off" spellcheck="false" />
										</label>
										<div class="onb-actions">
											<button class="btn primary" type="submit" disabled={busy || !project || !fileName.trim()}>
												Create deliverable
											</button>
										</div>
									</form>
								{/if}
							</div>
						{/if}
					</div>
				</li>
			{/each}
		</ol>

		{#if !current && !revisiting}
			<div class="onb-done">
				<PartyPopper size={18} />
				<span>You are ready to work.</span>
				<button class="btn primary small" onclick={() => finish('You are all set up')}>Close the guide</button>
			</div>
		{/if}
	</section>
{/if}

{#if editor}
	<SemesterModal semester={null} existing={semesters} preset={editor.preset} onclose={() => (editor = undefined)} />
{/if}
