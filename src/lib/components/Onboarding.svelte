<script lang="ts">
	// The guide on the home page for a new client: bring in a vault that exists, or set one up step by step (what Ledger is
	// for first, a semester when that is study, a first project of that kind, a first thread, a first deliverable). What a
	// step needs is read
	// from the vault, so it ticks itself off when it is done, wherever it was done, and the guide ends with the first
	// deliverable (onboarding.rs keeps the rest).
	import Dropdown from '$lib/components/Dropdown.svelte';
	import { goto } from '$app/navigation';
	import Briefcase from '@lucide/svelte/icons/briefcase';
	import CalendarRange from '@lucide/svelte/icons/calendar-range';
	import Check from '@lucide/svelte/icons/check';
	import Compass from '@lucide/svelte/icons/compass';
	import FileText from '@lucide/svelte/icons/file-text';
	import FolderInput from '@lucide/svelte/icons/folder-input';
	import FolderPlus from '@lucide/svelte/icons/folder-plus';
	import GraduationCap from '@lucide/svelte/icons/graduation-cap';
	import MessageSquareText from '@lucide/svelte/icons/message-square-text';
	import Palette from '@lucide/svelte/icons/palette';
	import PartyPopper from '@lucide/svelte/icons/party-popper';
	import Rocket from '@lucide/svelte/icons/rocket';
	import { api } from '$lib/api';
	import { deliverableUrl } from '$lib/actions';
	import { draftOf } from '$lib/semesters';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import type { Component } from 'svelte';
	import type { OnboardingStatus, ProjectSummary, Purpose, Semester, SemesterDraft, SemesterTemplate, Settings } from '$lib/types';
	import OnboardingImport from './OnboardingImport.svelte';
	import ProjectModal from './ProjectModal.svelte';
	import SemesterModal from './SemesterModal.svelte';

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

	type StepId = 'purpose' | 'semester' | 'project' | 'thread' | 'deliverable';
	interface Step {
		id: StepId;
		title: string;
		icon: Component<{ size?: number }>;
		done: boolean;
		skipped: boolean;
		/** What a finished step shows in place of its form. */
		summary: string;
	}

	const purposes: { id: Purpose; title: string; sub: string; icon: Component<{ size?: number }> }[] = [
		{ id: 'study', title: 'Study', sub: 'Courses and projects in semesters and sprints, with a calendar to count your work in.', icon: GraduationCap },
		{ id: 'work', title: 'Work', sub: 'Clients and projects that run as long as they run.', icon: Briefcase },
		{ id: 'hobby', title: 'Hobby', sub: 'Projects of your own, at your own pace.', icon: Palette }
	];

	/** "Set up a new vault" was chosen, or "Import" is open. */
	let started = $state(false);
	let importing = $state(false);
	/** A skipped step that is being done after all, or what Ledger is for, being changed. */
	let revisiting = $state<StepId | null>(null);
	let editor = $state<{ preset?: SemesterDraft } | undefined>(undefined);
	// What a semester can start from: NJ26 in a new vault.
	let semesterTemplates = $state<SemesterTemplate[]>([]);
	api.listSemesterTemplates().then((t) => (semesterTemplates = t), () => {});
	let busy = $state(false);
	/** The New project window, open for the first project. */
	let making = $state(false);

	// The first thread and deliverable, in the project chosen (the first one, unless there are more).
	let threadTitle = $state('');
	let chosenProject = $state('');
	// The first deliverable
	let fileName = $state('');

	const project = $derived(projects.find((p) => p.slug === chosenProject) ?? projects[0]);
	const firstThread = $derived(projects.flatMap((p) => p.threads)[0]);

	// Not asked yet.
	const purpose = $derived(onboarding.purpose ?? null);
	const chosen = $derived(purposes.find((p) => p.id === purpose));
	// Only study works in semesters, so the semester step comes with that answer.
	const steps = $derived<Step[]>([
		{
			id: 'purpose',
			title: 'What do you want to use Ledger for first?',
			icon: chosen?.icon ?? Compass,
			done: purpose !== null,
			skipped: false,
			summary: chosen?.title ?? ''
		},
		...(purpose === 'study'
			? [
					{
						id: 'semester' as const,
						title: 'Set up your semester',
						icon: CalendarRange,
						done: semesters.length > 0,
						skipped: onboarding.skipped.includes('semester'),
						summary: semesters[0]?.name ?? ''
					}
				]
			: []),
		{
			id: 'project',
			title: 'Set up your first project',
			icon: FolderPlus,
			done: projects.length > 0,
			skipped: false,
			summary: projects[0]?.name ?? ''
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
	const stepCount = $derived(['Three', 'Four', 'Five'][steps.length - 3] ?? String(steps.length));

	async function choosePurpose(answer: Purpose) {
		busy = true;
		if (await attempt(() => api.setOnboardingPurpose(answer))) {
			revisiting = null;
			await refresh();
		}
		busy = false;
	}

	async function skip(step: 'semester') {
		revisiting = null;
		if (await attempt(() => api.skipOnboardingStep(step))) await refresh();
	}

	async function createThread() {
		if (!project) return;
		busy = true;
		const slug = project.slug;
		if (await attempt(() => api.createThread(slug, threadTitle.trim()))) {
			threadTitle = '';
			await refresh();
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

	/** A form's submit: `act`, in place of sending the form. */
	const submit = (act: () => Promise<void>) => (e: SubmitEvent) => {
		e.preventDefault();
		void act();
	};

	async function finish(thanks?: string) {
		if (!(await attempt(() => api.finishOnboarding()))) return;
		if (thanks) toast(thanks);
		await refresh();
	}

	async function hide() {
		const ok = await confirmDialog({
			title: 'Hide the setup guide?',
			message: 'What you have set up stays as it is. You can bring the guide back under Settings > Vault & external > Vault.',
			confirmLabel: 'Hide the guide'
		});
		if (ok) await finish();
	}
</script>

<!-- Which project the first thread and deliverable go in, when there is more than one. -->
{#snippet projectPicker()}
	{#if projects.length > 1}
		<div class="field">
			Project
			<Dropdown bind:value={chosenProject} options={projects.map((p) => ({ value: p.slug, label: p.name }))} label="Project" full />
		</div>
	{/if}
{/snippet}

{#if importing && settings}
	<section class="onb" aria-label="Import a vault">
		<OnboardingImport
			{settings}
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
			Everything you write lives in a folder, a vault, that your cloud storage (OneDrive, SharePoint, Nextcloud) keeps in sync and backs up. Do you
			have a vault already?
		</p>
		<div class="onb-choices">
			<button class="onb-choice" onclick={() => (importing = true)} disabled={!settings}>
				<span class="onb-choice-icon"><FolderInput size={20} /></span>
				<strong>Import an existing vault</strong>
				<span class="onb-choice-sub">From its folder, synced here by your cloud storage.</span>
			</button>
			<button class="onb-choice primary" onclick={() => (started = true)}>
				<span class="onb-choice-icon"><Rocket size={20} /></span>
				<strong>Set up a new vault</strong>
				<span class="onb-choice-sub">A few short steps: what it is for first, then a first project, thread and deliverable.</span>
			</button>
		</div>
	</section>
{:else}
	<section class="onb" aria-label="Set up your vault">
		<header class="onb-head">
			<div>
				<h2>Set up your vault</h2>
				<p class="sub">{current ? `${stepCount} short steps, and you are ready to work.` : 'Everything is set up.'}</p>
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
							{#if step.id === 'purpose' && step.done && open !== step.id}
								<button class="link accent" onclick={() => (revisiting = 'purpose')}>Change</button>
							{/if}
						</div>
						{#if step.done && step.summary && open !== step.id}<div class="onb-summary">{step.summary}</div>{/if}

						{#if open === step.id && (!step.done || revisiting === step.id)}
							<div class="onb-body">
								{#if step.id === 'purpose'}
									<div class="onb-choices compact">
										{#each purposes as p (p.id)}
											{@const Choice = p.icon}
											<button class="onb-choice" class:primary={purpose === p.id} onclick={() => choosePurpose(p.id)} disabled={busy}>
												<span class="onb-choice-icon"><Choice size={18} /></span>
												<strong>{p.title}</strong>
												<span class="onb-choice-sub">{p.sub}</span>
											</button>
										{/each}
									</div>
									{#if revisiting === 'purpose'}
										<div class="onb-actions">
											<button class="btn ghost" onclick={() => (revisiting = null)}>Keep {chosen?.title ?? 'it'}</button>
										</div>
									{/if}
								{:else if step.id === 'semester'}
									<p>
										A semester gives the dashboard its calendar: the weeks, the vacations and the sprints that your work is counted in.
										Start from a semester template, or lay out your own.
									</p>
									<div class="onb-actions">
										{#each semesterTemplates as t (t.id)}
											<button class="btn primary" onclick={() => (editor = { preset: draftOf(t) })}>Start from {t.name}</button>
										{/each}
										<button class="btn" onclick={() => (editor = {})}>Create my own</button>
										<button class="btn ghost" onclick={() => skip('semester')}>Skip for now</button>
									</div>
								{:else if step.id === 'project'}
									<p>
										A <strong>project</strong> holds the work on one subject: a course, a client, a team. It starts from its goal: what it must
										achieve, and for whom.
									</p>
									<div class="onb-actions">
										<button class="btn primary" onclick={() => (making = true)}><FolderPlus size={15} />{chosen ? `New ${chosen.title.toLowerCase()} project` : 'New project'}</button>
									</div>
								{:else if step.id === 'thread'}
									<p>A <strong>thread</strong> is where you think the work through, with posts, prompts and feedback.</p>
									<form class="onb-form" onsubmit={submit(createThread)}>
										{@render projectPicker()}
										<label class="field">
											Thread
											<input bind:value={threadTitle} placeholder="e.g. What should we charge?" autocomplete="off" />
										</label>
										<div class="onb-actions">
											<button class="btn primary" type="submit" disabled={busy || !project || !threadTitle.trim()}>
												Create thread
											</button>
										</div>
									</form>
								{:else}
									<p>
										A <strong>deliverable</strong> is a file in the project that your prompts and feedback are about: a brief, a report, a plan.
										Give it a name; you write it in Ledger.
									</p>
									<form class="onb-form" onsubmit={submit(createDeliverable)}>
										{@render projectPicker()}
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
{#if making}
	<ProjectModal kind={purpose ?? 'study'} oncreated={(slug) => (chosenProject = slug)} onclose={() => (making = false)} />
{/if}
