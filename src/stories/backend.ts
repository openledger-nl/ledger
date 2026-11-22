// The backend as Storybook stands in for it: Ledger's front end asks the Rust side for everything through Tauri's
// `invoke`, and in Storybook there is no Rust side. .storybook/preview.ts hands every call (and the plugins' calls:
// the opener, the dialog) to `answer` below through Tauri's own mock (`mockIPC`), so components that load their own data
// show the sample vault from fixtures.ts. A command that isn't listed answers null, and says so in the console.

import type { LayoutSettings, PortflowStatus, PortflowView, UpdateView } from '$lib/types';
import * as fx from './fixtures';

const portflowStatus: PortflowStatus = {
	state: 'idle',
	signedIn: true,
	expiresAt: null,
	lastSyncAt: '2026-11-16T09:10:00.000Z',
	lastError: null,
	summary: 'Up to date',
	report: null
};

const portflow: PortflowView = {
	enabled: true,
	launchUrl: 'https://portflow.fontys.nl',
	defaultLaunchUrl: 'https://portflow.fontys.nl',
	write: false,
	writes: [
		{ id: 'guide-description', label: 'Reading guide to collection description', on: false, available: true },
		{ id: 'evidence-description', label: 'Evidence description to evidence item', on: false, available: true }
	],
	writesAvailable: true,
	portfolio: { id: 9_001, userId: 7_001, userName: 'Student, Sam' },
	status: portflowStatus,
	links: [],
	outcomes: 12,
	people: 3,
	unplaced: 0,
	savedLogin: null
};

const updates: UpdateView = {
	currentVersion: '0.8.0',
	automatic: true,
	unavailable: null,
	devBuild: false,
	following: null,
	available: null,
	checkedAt: '2026-11-16T08:00:00.000Z',
	configFile: '/home/student/.config/nl.openledger.ledger/config.json'
};

const layout: LayoutSettings = { sidebarFold: 'rail' };

/** What each command answers: a value, or a function of the call's arguments. */
const answers: Record<string, unknown | ((args: Record<string, unknown>) => unknown)> = {
	// the vault
	list_projects: fx.projects,
	get_project: fx.projectView,
	get_project_meta: fx.projectMeta,
	list_project_meta: [fx.projectMeta],
	list_project_people: fx.people,
	list_people: fx.vaultPeople,
	list_clients: fx.clients,
	create_client: fx.clients[0],
	update_client: fx.clients[0],
	delete_client: null,
	get_person: (a: Record<string, unknown>) => fx.vaultPeople.find((p) => p.id === a.id) ?? fx.vaultPeople[0],
	create_person: fx.vaultPeople[4],
	update_person: (a: Record<string, unknown>) => fx.vaultPeople.find((p) => p.id === a.id) ?? fx.vaultPeople[0],
	delete_person: null,
	get_project_branding: fx.branding,
	pdf_layout: { used: 'vault', project: null, vault: 'shared/templates/pdf/layout.typ' },
	get_thread: fx.threadView,
	list_semesters: [fx.semester],
	list_semester_templates: [fx.semesterTemplate],
	save_semester_template: fx.semesterTemplate,
	delete_semester_template: null,
	list_activity: fx.activity,
	list_deliverables: fx.deliverables,
	get_deliverable: fx.deliverable,
	get_provenance: fx.provenance,
	deliverable_changes: (a: Record<string, unknown>) => (a.from === '1.1' && a.to === 'live' ? fx.changesLive : fx.changes),
	deliverable_live_changes: (a: Record<string, unknown>) => (a.against === '1.0' ? fx.liveChangesReview : fx.liveChanges),
	deliverable_authors: (a: Record<string, unknown>) => (a.focus ? fx.authorshipTrace : fx.authorship),
	list_deliverable_prompts: [fx.prompt],
	list_deliverable_meta: [],
	get_deliverable_meta: null,
	next_deliverable_number: 4,
	list_daily_notes: [fx.dailyNote],
	get_daily_note: fx.dailyNote,
	list_all_feedback: fx.feedbackList.map((f) => ({
		...f,
		project: fx.PROJECT,
		projectName: 'Smart Greenhouse',
		threadTitle: 'Sensor research',
		storyTitle: null
	})),
	list_project_uploads: [fx.upload, fx.uploadFile],
	list_stories: [fx.story],
	get_board: fx.board,
	list_templates: [fx.template],
	project_templates: [fx.template],
	effective_templates: [fx.template],
	template_reader_presets: [
		{ id: 'me', name: 'Me, later', hint: 'knows the project; wants the decisions and why' },
		{ id: 'coach', name: 'Coach or assessor', hint: 'judges the work against the learning outcomes' }
	],
	template_skeleton: fx.template.body,
	list_links: [],
	list_vault_links: [],
	search: [
		{
			kind: 'post',
			id: 'p-2',
			projectSlug: fx.PROJECT,
			projectName: 'Smart Greenhouse',
			title: 'Sensor research',
			threadSlug: fx.THREAD,
			createdAt: '2026-11-11T10:00:00.000Z',
			snippet: 'Capacitive <mark>sensors</mark> corrode far less than resistive ones.'
		},
		{
			kind: 'deliverable',
			id: 'sensor-research.md',
			projectSlug: fx.PROJECT,
			projectName: 'Smart Greenhouse',
			title: 'sensor-research.md',
			threadSlug: null,
			createdAt: '2026-11-14T10:00:00.000Z',
			snippet: 'Which soil moisture <mark>sensor</mark> holds up in wet soil...'
		}
	],
	render_markdown: (args: Record<string, unknown>) => `<p>${String(args.markdown ?? args.text ?? '')}</p>`,
	suggest_prompt_base: 'You help a student with an ICT project.',
	read_project_file: () => fx.sketchBytes(),
	read_project_image: () => fx.sketchBytes(),
	preview_dropped_image: () => fx.sketchBytes(),
	check_vault_changes: false,

	// settings and the app
	get_settings: fx.settings,
	layout_settings: layout,
	onboarding_status: fx.onboarding,
	set_onboarding_purpose: (a: Record<string, unknown>) => ({ ...fx.onboarding, purpose: a.purpose }),
	list_vaults: fx.vaults,
	list_conflicts: fx.conflicts,
	vault_encryption: { encrypted: false, keyId: null, kept: false },
	unlock_vault: fx.vaults,
	resolve_conflict: [],
	create_vault: { vaults: fx.vaults, recovery: null },
	open_vault: fx.vaults,
	forget_vault: fx.vaults,
	update_status: updates,
	log_read: [
		{ at: '2026-11-16T09:12:00.000Z', level: 'info', source: 'vault', message: 'Reloaded changes made outside Ledger' },
		{ at: '2026-11-16T09:10:00.000Z', level: 'info', source: 'portflow', message: 'Took 2 evidence items from Portflow' }
	],
	log_write: null,
	capture_status: { active: false, startedAt: null, events: 0 },
	about_info: {
		version: '0.8.0',
		tauriVersion: '2.12.5',
		os: 'linux',
		arch: 'x86_64',
		logFile: '/home/student/.local/share/nl.openledger.ledger/logs/ledger.log',
		logMaxBytes: 1048576,
		configDir: '/home/student/.config/nl.openledger.ledger',
		homeDir: '/home/student'
	},
	trello_get: {
		hasKey: false,
		keyHint: '',
		hasToken: false,
		tokenHint: '',
		member: null,
		configured: false,
		projects: [],
		unlinked: [],
		status: { configured: false, state: 'disabled', pending: false, lastSyncAt: null, lastError: null, summary: null, report: null },
		settingsFile: '/home/student/.config/nl.openledger.ledger/vaults/3f9a0c1d2e4b5a6978c0d1e2f3a4b5c6/trello.json',
		secretsFile: 'the system keychain'
	},
	pending_screenshot: null,
	pending_screenshot_image: () => fx.sketchBytes(),
	claude_code_settings: {
		cliPath: 'claude',
		permissionMode: 'acceptEdits',
		model: '',
		confine: true,
		autoRun: false,
		namePrompts: true,
		resolvedPath: '/usr/local/bin/claude',
		problem: null,
		settingsFile: '/home/student/.config/nl.openledger.ledger/claude.json',
		maxConcurrent: 2
	},
	check_claude_code: { ok: true, version: '2.1.0', path: '/usr/local/bin/claude', message: 'Claude Code 2.1.0' },
	claude_code_runs: [],
	// The confirmation before a run: its command follows the model chosen for the run (null: the one in Settings, here none).
	claude_code_preview: (a: Record<string, unknown>) => {
		const model = typeof a.model === 'string' ? a.model : '';
		return {
			folder: '/home/student/Documents/Ledger/projects/smart-greenhouse',
			command: `claude -p --output-format stream-json --verbose --permission-mode acceptEdits${model ? ` --model ${model}` : ''}`,
			prompt: fx.prompt.body,
			guard: null,
			permissionMode: 'acceptEdits',
			model,
			problem: null
		};
	},

	// Portflow
	portflow_get: portflow,
	portflow_status: portflowStatus,
	portflow_project: fx.projectPortflow,
	portflow_thread_evidence: [fx.evidence],
	portflow_recent_feedback: [],
	portflow_epsilon: fx.epsilon,
	portflow_people: [],
	portflow_evidence_choices: [],
	portflow_sections: [{ id: 1, name: 'ICT & Software', template: 'S3', importedAt: '2026-09-02T10:00:00.000Z', semester: 's3-2026', semesterName: 'S3 - najaar 2026' }],
	portflow_collections: [],
	portflow_save_evidence_description: [fx.evidence],
	portflow_save_login: (args: Record<string, unknown>) => ({ ...portflow, savedLogin: String(args.username ?? '').trim() }),
	portflow_forget_login: portflow,

	// writes: answered with what they would have saved, near enough
	add_post: fx.posts[1],
	add_feedback: fx.feedbackOpen,
	update_feedback: fx.feedbackOpen,
	set_feedback_status: fx.feedbackResolved,
	save_daily_note: fx.dailyNote,
	save_deliverable: fx.deliverable,
	create_thread: fx.threads[0],
	create_prompt: fx.prompt,
	update_prompt: fx.prompt
};

export function answer(command: string, args: Record<string, unknown> = {}): unknown {
	if (command.startsWith('plugin:')) return null; // the opener and the dialog: nothing to open in Storybook
	if (!(command in answers)) {
		console.info(`[storybook backend] ${command} isn't faked; it answers null`, args);
		return null;
	}
	const value = answers[command];
	return typeof value === 'function' ? (value as (a: Record<string, unknown>) => unknown)(args) : value;
}
