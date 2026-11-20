// Mirrors src-tauri/src/models.rs (camelCase on the wire).

export interface LinkedThread {
	slug: string;
	title: string;
}

export interface ThreadSummary {
	projectSlug: string;
	slug: string;
	title: string;
	createdAt: string;
	linkedThreadSlugs: string[];
	linkedCount: number;
	postCount: number;
	promptCount: number;
	feedbackCount: number;
	uploadCount: number;
	storyCount: number;
	lastActivity: string;
}

/** A project with its threads, as the sidebar shows it. */
export interface ProjectSummary {
	slug: string;
	name: string;
	createdAt: string;
	updatedAt: string;
	threads: ThreadSummary[];
	deliverableCount: number;
	storyCount: number;
	taskCount: number;
}

export interface ProjectView {
	slug: string;
	name: string;
	description: string;
	/** The reading guide (README.md), raw and rendered. */
	readme: string;
	readmeHtml: string;
	threadCount: number;
	deliverableCount: number;
	/** Files uploaded in its threads, screenshots included. */
	attachmentCount: number;
	storyCount: number;
	taskCount: number;
	openTaskCount: number;
}

export interface Post {
	id: string;
	parentId: string | null;
	guidingQuestion: boolean;
	versionPublished: boolean;
	deliverablePath: string | null;
	deliverableVersion: string | null;
	/** On a "version published" note: the prompt that caused the version, and its name and thread while it still exists. */
	promptId: string | null;
	promptTitle: string | null;
	promptThread: string | null;
	createdAt: string;
	updatedAt: string;
	body: string;
	html: string;
	/** What the post is; null until you say. */
	kind: PostKind | null;
	/** How sure you are of it; set by hand only. */
	labels: EpistemicLabel[];
}

export interface PromptDeliverable {
	/** The live path: a prompt belongs to the deliverable, not to one of its versions. */
	path: string;
	exists: boolean;
	/** The versions published so far, oldest first. */
	versions: string[];
}

export interface Prompt {
	id: string;
	threadSlug: string;
	title: string;
	templateId: string | null;
	templateName: string | null;
	/** What is filled in for the person, from a template or from feedback; they can edit it. */
	base: string;
	baseHtml: string;
	/** What the person adds, in their own words. */
	body: string;
	html: string;
	createdAt: string;
	updatedAt: string;
	deliverables: PromptDeliverable[];
	/** The pieces of feedback (ids) it was written to resolve. */
	feedback: string[];
	/** The times it was run with Claude Code, newest first. */
	runs: ClaudeRunRecord[];
}

/** What a new prompt is made of. It has no title: Claude Code names it from what it asks. */
export interface NewPromptInput {
	/** Filled in from the template or the feedback (`suggestPromptBase`), edited or not. */
	base: string;
	/** What the person adds. */
	body: string;
	templateId: string | null;
	/** An existing deliverable the prompt asks changes to. */
	deliverable: string | null;
	/** The name of a new deliverable to start for it; null leaves the naming to whatever produces it. */
	newDeliverable: string | null;
	/** The pieces of feedback (ids) it is meant to resolve. */
	feedback: string[];
	/** The template's guiding questions as answered in the dialog; a blank answer is a question skipped. */
	answers?: { question: string; answer: string }[];
}

/** What the New prompt dialog starts from when it is opened for something other than a blank prompt. */
export interface PromptDraft {
	title: string;
	body: string;
	/** An existing deliverable the prompt asks for changes to, by its live path; null leaves the choice at "a new deliverable". */
	deliverable: string | null;
	/** The pieces of feedback (ids) the prompt is written to resolve. */
	feedback?: string[];
	/** Said at the top of the dialog: where the draft came from. */
	note: string;
}

export interface Feedback {
	id: string;
	threadSlug: string;
	deliverablePath: string;
	deliverableVersion: string | null;
	status: 'open' | 'resolved';
	body: string;
	html: string;
	createdAt: string;
	updatedAt: string;
	/** The passage it is about, as plain text, with the words just before and after it (see anchors.ts); null when it is
	 *  about the whole document. */
	quote: string | null;
	quotePrefix: string | null;
	quoteSuffix: string | null;
	/** The version it was given on: the one it names, or for feedback on the live file the first version published after it
	 *  was written (what that version froze); null while none has been published since. */
	givenOn: string | null;
	/** Who gave it; with their Portflow id and role when they are someone in Portflow. */
	from: string | null;
	fromUser: number | null;
	fromRole: string | null;
	/** How it was given: "in person", "meeting", "portflow", "email", "phone" or "other". */
	given: string | null;
	/** Your own note (for yourself or for Claude), or feedback from someone else. */
	kind: FeedbackKind;
	/** The user story it is about, by id; the day ("2026-11-09") of the meeting note it was written down in. */
	story: string | null;
	daily: string | null;
	/** Set on feedback pulled from Portflow: it can't be edited in Ledger. */
	portflowComment: number | null;
	/** The feedback this answers (a reply in Portflow). */
	replyTo: string | null;
	commentType: string | null;
	/** The evidence item (its slug) and the version it is on. */
	evidence: string | null;
	evidenceVersion: number | null;
}

/** Who gave a piece of feedback, as typed or picked. */
export interface FeedbackFrom {
	name: string | null;
	portflowUser: number | null;
	role: string | null;
	given: string | null;
}

export interface Upload {
	id: string;
	threadSlug: string;
	/** Relative to the project folder, e.g. "attachments/diagram.png". */
	file: string;
	name: string;
	size: number;
	uploadedAt: string;
	description: string;
	descriptionHtml: string;
	image: boolean;
	/** Whether the file is still where the manifest says. */
	exists: boolean;
}

export interface ThreadView {
	projectSlug: string;
	slug: string;
	title: string;
	description: string;
	createdAt: string;
	linkedThreads: LinkedThread[];
	posts: Post[];
	prompts: Prompt[];
	feedback: Feedback[];
	uploads: Upload[];
	stories: Story[];
}

// ---- semesters and sprints --------------------------------------------------------------------------

/** A stretch of a semester. Dates are calendar days, "2026-09-01", inclusive at both ends. */
export interface Sprint {
	name: string;
	start: string;
	end: string;
	goal: string;
}

/** Days off within a semester: the sprint clock stops, and starts again after it. */
export interface Vacation {
	name: string;
	start: string;
	end: string;
}

export interface Semester {
	id: string;
	name: string;
	start: string;
	end: string;
	sprintWeeks: number;
	notes: string;
	sprints: Sprint[];
	vacations: Vacation[];
	createdAt: string;
	updatedAt: string;
}

/** What creating or changing a semester takes. Without sprints, they are cut from the start. */
export interface SemesterDraft {
	name: string;
	start: string;
	end: string;
	sprintWeeks: number;
	notes: string;
	sprints: Sprint[];
	vacations: Vacation[];
}

// ---- activity ---------------------------------------------------------------------------------------

export type ActivityKind = 'thread' | 'post' | 'prompt' | 'feedback' | 'upload' | 'version' | 'run' | 'story' | 'task';

/** One thing made in a project, at a moment: what the contribution graph counts. */
export interface ActivityEvent {
	projectSlug: string;
	kind: ActivityKind;
	at: string;
}

// ---- deliverables -----------------------------------------------------------------------------------

export type DeliverableKind = 'markdown' | 'image' | 'other';

export interface DeliverableSummary {
	/** The live file, relative to the deliverables folder. */
	path: string;
	name: string;
	kind: DeliverableKind;
	size: number;
	modified: string;
	/** Whether the live file exists (published versions can outlive it). */
	exists: boolean;
	/** Published versions, oldest first. */
	versions: string[];
	promptCount: number;
	openFeedback: number;
	/** The AI-use label of the newest version that has one. */
	mmm: MmmLevel | null;
	/** When the newest published version was made. */
	latestAt: string | null;
	/** The newest version of its evidence item that is in Portflow (Portflow on only), and when it was delivered there. */
	delivered: { number: number; at: string } | null;
	/** The Portflow evidence item this deliverable is (its name), when it is one: it is shown by that name. */
	evidence: string | null;
	/** The item's other files (base paths, in a folder beside it), folded into it rather than listed on their own. */
	attachments: string[];
}

export interface DeliverablePrompt {
	threadSlug: string;
	threadTitle: string;
	id: string;
	title: string;
}

export interface Deliverable {
	path: string;
	basePath: string;
	name: string;
	kind: DeliverableKind;
	/** Set when `path` is a published version, which is read-only. */
	version: string | null;
	versions: string[];
	exists: boolean;
	size: number;
	modified: string;
	body: string | null;
	html: string | null;
	editable: boolean;
	prompts: DeliverablePrompt[];
	feedback: Feedback[];
	/** The Portflow evidence item this deliverable is (its name), when it is one: it is shown by that name. */
	evidence: string | null;
	/** The item's other files (base paths, in a folder beside it), folded into it rather than listed on their own. */
	attachments: string[];
}

// ---- how versions came to be ------------------------------------------------------------------------

/** The five AI-use (MMM) levels, from the most AI to the least. */
export type MmmLevel = 'lazy-prompter' | 'piggybacker' | 'cyborg' | 'handyman' | 'man-made';

export interface PromptRef {
	id: string;
	title: string;
	threadSlug: string;
	/** Whether the prompt is still there. */
	exists: boolean;
}

export interface RunSummary {
	id: string;
	promptId: string;
	promptTitle: string;
	threadSlug: string;
	status: 'finished' | 'failed' | 'cancelled';
	model: string;
	permissionMode: string;
	startedAt: string;
	finishedAt: string;
	seconds: number;
	turns: number | null;
}

export interface ProvenanceVersion {
	kind: 'version';
	version: string;
	/** The snapshot, relative to the deliverables folder. */
	path: string;
	at: string;
	by: 'ai' | 'human' | 'unknown';
	prompt: PromptRef | null;
	run: RunSummary | null;
	mmm: MmmLevel | null;
	note: string;
	size: number;
	sizeChange: number | null;
	sha256: string | null;
	/** The id of the record file in the vault (`versions/<id>.md`), when there is one. */
	record: string | null;
	/** The "version published" note, in its thread. */
	post: { threadSlug: string; id: string } | null;
}

export type ProvenanceEntry =
	| { kind: 'prompt'; id: string; title: string; at: string }
	| ({ kind: 'run'; version: string | null; at: string } & RunSummary)
	| ProvenanceVersion
	| { kind: 'feedback'; id: string; status: 'open' | 'resolved'; version: string | null; excerpt: string; at: string };

export interface ProvenanceGroup {
	/** Null for what is filed under no thread. */
	thread: { slug: string; title: string; exists: boolean } | null;
	firstAt: string;
	lastAt: string;
	entries: ProvenanceEntry[];
}

/** A run that changed the live file and is in no version. */
export interface StoppedRun {
	id: string;
	promptId: string;
	promptTitle: string;
	threadSlug: string;
	status: string;
	finishedAt: string;
}

/** What the live file holds that no version does: the version it follows, when it last changed. */
export interface ProvenanceDraft {
	since: string | null;
	modified: string;
	stoppedRun: StoppedRun | null;
}

export interface Provenance {
	groups: ProvenanceGroup[];
	summary: {
		versions: number;
		ai: number;
		human: number;
		unrecorded: number;
		/** The label of the newest version that has one. */
		label: { level: MmmLevel; version: string } | null;
	};
	/** The live file when it holds something no version does. */
	draft: ProvenanceDraft | null;
	/** Where a publish goes when the person does not say. */
	defaultThread: string | null;
	stoppedRun: StoppedRun | null;
	historyHasAi: boolean;
	/** The label of the newest labelled version: where the next one starts from. */
	suggestedLabel: MmmLevel | null;
}

/** What the person says about a version when publishing it. `thread`: null picks the default, "" files it in no thread. */
export interface PublishChoices {
	thread: string | null;
	ai: boolean;
	mmm: MmmLevel | null;
	prompt: string | null;
	run: string | null;
	note: string | null;
}

/** Only what is present changes. `mmm`: "" removes the label. */
export interface VersionUpdate {
	by?: 'ai' | 'human';
	mmm?: MmmLevel | '';
	note?: string;
}

// ---- templates --------------------------------------------------------------------------------------

export interface Template {
	/** "" for a vault-wide template, else the project's slug. */
	scope: string;
	id: string;
	name: string;
	/** The "use when…" hint. */
	trigger: string;
	questions: string[];
	/** The skeleton of the deliverable. */
	body: string;
	/** The kinds of project it is offered in; empty for every kind. */
	kinds: ProjectKind[];
	/** Who its deliverables are written for: told to Claude, and noted in the deliverable. */
	readers: TemplateReader[];
	/** The readers are a visible line in a new deliverable, not only a `%%` note. */
	readersShown: boolean;
	/** One Ledger ships, as shipped: not a file in the vault until it is changed, and it can't be deleted. */
	builtin: boolean;
	/** The vault's own version of one Ledger ships: deleting it brings the shipped one back. */
	customisedBuiltin: boolean;
	createdAt: string;
	updatedAt: string;
	/** A vault-wide template the project has replaced with its own. */
	overridden: boolean;
}

/** Someone a template's deliverable is written for: a preset's id (`client`, `me`, `people`, …) or your own words. */
export interface TemplateReader {
	who: string;
	/** What they need from it, beyond what the preset says. */
	needs: string;
	/** What must be kept from them. */
	keepFrom: string;
}

/** A reader a template can name by a word, with what Claude is told about them. */
export interface ReaderPreset {
	id: string;
	name: string;
	hint: string;
}

/** What a template says, as the editor saves it. */
export type TemplateDraft = Pick<Template, 'name' | 'trigger' | 'questions' | 'body' | 'kinds' | 'readers' | 'readersShown'>;

// ---- stories, tasks, the board ----------------------------------------------------------------------

export interface Criterion {
	text: string;
	done: boolean;
}

export interface Story {
	projectSlug: string;
	id: string;
	summary: string;
	role: string;
	want: string;
	reason: string;
	notes: string;
	criteria: Criterion[];
	threads: LinkedThread[];
	taskCount: number;
	createdAt: string;
	updatedAt: string;
}

export interface Column {
	id: string;
	name: string;
}

export interface Task {
	id: string;
	title: string;
	body: string;
	columnId: string;
	storyId: string | null;
	storySummary: string | null;
	threadSlug: string | null;
	threadTitle: string | null;
	createdAt: string;
	updatedAt: string;
}

export interface Board {
	columns: Column[];
	backlogColumnId: string;
	tasks: Task[];
	stories: { id: string; summary: string }[];
}

// ---- search -----------------------------------------------------------------------------------------

export interface SearchHit {
	kind: 'post' | 'prompt' | 'feedback' | 'deliverable';
	/** The post, prompt or feedback id; the path for a deliverable. */
	id: string;
	projectSlug: string;
	projectName: string;
	title: string;
	threadSlug: string | null;
	createdAt: string;
	snippet: string;
}

// ---- settings, sync, screenshots --------------------------------------------------------------------

export interface SyncStatus {
	configured: boolean;
	repo: string | null;
	branch: string | null;
	state: 'disabled' | 'idle' | 'syncing' | 'error';
	/** Changes made here that have not been pushed yet. */
	pending: boolean;
	/** Whether syncs encrypt what they upload (the setting as it stood when the last sync started). */
	encrypted: boolean;
	lastSyncAt: string | null;
	lastError: string | null;
	summary: string | null;
}

/** What GitHub holds of the vault as of the last sync that finished, counted without asking GitHub. */
export interface SyncOverview {
	/** The commit both sides last agreed on. */
	head: string | null;
	files: number;
	deliverables: number;
	attachments: number;
	/** Everything else: threads, posts, stories, templates, semesters … */
	notes: number;
	/** The key the files on GitHub are encrypted with (two hex digits), once they all are. */
	key: string | null;
}

export interface SyncSettings {
	repo: string;
	branch: string;
	/** The token itself never reaches the screen. */
	hasToken: boolean;
	tokenHint: string;
	configured: boolean;
}

export interface EncryptionStatus {
	/** Whether what is synced is encrypted. */
	enabled: boolean;
	/** The key lives in the vault's .ledger/.env; the screen only learns whether it is there. */
	hasKey: boolean;
	/** The key's one-byte label as two hex digits, to tell keys apart between machines. */
	keyId: string | null;
	/** Why the key can't be read, if it can't (a damaged line in .ledger/.env, an unreadable file). */
	problem: string | null;
	/** This vault already synced the configured repository and branch without encryption: that branch can't become an encrypted one. */
	needsFreshTarget: boolean;
}

/** What a folded sidebar becomes: an icon rail (the default) or nothing. */
export type SidebarFold = 'rail' | 'hide';

/** The shell's layout, from the vault's `.ledger/settings.json` (never synced: each machine has its own). */
export interface LayoutSettings {
	sidebarFold: SidebarFold;
}

export interface ScreenshotSettings {
	hotkey: string;
	defaultHotkey: string;
	/** Why the saved hotkey isn't working, if it isn't. */
	problem: string | null;
	/** Something about this platform worth knowing (a missing permission, Wayland). */
	note: string | null;
	/** Whether the user's own tool takes the screenshots, and its command (kept while the built-in one is used). */
	useCommand: boolean;
	command: string;
	/** Ready-made commands for this platform. */
	presets: ScreenshotPreset[];
	/** What the built-in capture does on this platform. */
	builtin: string;
	platform: 'linux' | 'macos' | 'windows';
}

export interface ScreenshotPreset {
	name: string;
	command: string;
	/** The program is installed where the command expects it. */
	found: boolean;
}

export interface Settings {
	vaultPath: string;
	/** Fixed by the LEDGER_VAULT environment variable. */
	vaultLocked: boolean;
	settingsFile: string;
	secretsFile: string;
	version: string;
	sync: SyncSettings;
	status: SyncStatus;
	screenshot: ScreenshotSettings;
	encryption: EncryptionStatus;
}

/** The guide on the home page for a new client (see onboarding.rs). */
export interface VaultEntry {
	path: string;
	/** The folder's name, which is what the picker calls the vault. */
	name: string;
	/** The folder is still there. */
	exists: boolean;
	/** It is the vault that is open. */
	open: boolean;
}

export interface Vaults {
	/** A vault is open; before the first one the app shows the vault picker. */
	open: boolean;
	/** Fixed by the LEDGER_VAULT environment variable. */
	locked: boolean;
	/** The open vault first, then the others opened on this machine, the latest first. */
	vaults: VaultEntry[];
	/** Where a new vault goes unless another folder is picked. */
	suggestedParent: string | null;
}

export interface OnboardingStatus {
	/** The guide is on the home page. */
	active: boolean;
	/** The steps that were put off: only "semester" and "sync" can be. */
	skipped: string[];
}

/** A dragged rectangle, in the capture's own pixels. */
export interface Region {
	x: number;
	y: number;
	width: number;
	height: number;
}

/** The payload of the `screenshot-ready` event: a screenshot is waiting to be attached to a thread. */
export interface ScreenshotTaken {
	width: number;
	height: number;
}

export interface StoryDraft {
	id: string;
	summary: string;
	role: string;
	want: string;
	reason: string;
	notes: string;
	criteria: Criterion[];
	threadSlugs: string[];
}

export interface StoryUpdate {
	summary?: string;
	role?: string;
	want?: string;
	reason?: string;
	notes?: string;
	criteria?: Criterion[];
}

/** For `storyId` and `threadSlug`, an empty string clears the link. */
export interface TaskUpdate {
	title?: string;
	body?: string;
	columnId?: string;
	storyId?: string;
	threadSlug?: string;
}

export type Bump = 'minor' | 'major';

// ---- Run a prompt with Claude Code (a block of its own; see src-tauri/src/claude_code) ---------------

/** What Claude Code may do on its own. There is deliberately no mode that skips the permission checks. */
export type ClaudePermissionMode = 'acceptEdits' | 'plan' | 'dontAsk';

export interface ClaudeCodeSettings {
	/** Empty means "find claude on the PATH". */
	cliPath: string;
	permissionMode: ClaudePermissionMode;
	/** Empty means the CLI's own default. */
	model: string;
	/** Extra guard rails: ignore the project's own Claude settings, and refuse reads and writes outside the project. */
	confine: boolean;
	/** A prompt that is created starts running at once, without the confirmation. */
	autoRun: boolean;
	/** A new prompt is named by Claude Code from what it asks. */
	namePrompts: boolean;
	/** The program that would be run, when it can be found. */
	resolvedPath: string | null;
	/** Why it can't be found. */
	problem: string | null;
	settingsFile: string;
	maxConcurrent: number;
}

export interface ClaudeCodeCheck {
	ok: boolean;
	version: string;
	path: string | null;
	message: string;
}

export interface ClaudeRunPreview {
	/** Where it runs. */
	folder: string;
	/** For reading only; it is never run through a shell. */
	command: string;
	/** The text sent on standard input, exactly. */
	prompt: string;
	/** The settings file given with --settings, when the guard rails are on. */
	guard: string | null;
	permissionMode: ClaudePermissionMode;
	model: string;
	/** Set when it can't be run. */
	problem: string | null;
}

export type ClaudeRunStatus = 'running' | 'finished' | 'failed' | 'cancelled';

export interface ClaudeRun {
	id: string;
	project: string;
	thread: string;
	promptId: string;
	promptTitle: string;
	status: ClaudeRunStatus;
	startedAt: string;
	finishedAt: string | null;
	exitCode: number | null;
	/** Why it failed or stopped. */
	message: string | null;
	/** Cancel was pressed and the process is being stopped. */
	cancelling: boolean;
	/** How many pieces of output there have been so far. */
	chunks: number;
	/** The versions the app published for what the run wrote and linked to the prompt, like "plan@v0.2.md". */
	linked: string[];
	vaultReloaded: boolean;
}

/** The note kept in the thread (`runs/<id>.md`) about a run of a prompt that is over. */
export interface ClaudeRunRecord {
	/** The id of the run, as it was while it ran. */
	id: string;
	promptId: string;
	status: Exclude<ClaudeRunStatus, 'running'>;
	model: string;
	permissionMode: string;
	startedAt: string;
	finishedAt: string;
	seconds: number;
	turns: number | null;
	/** Deliverables it wrote or changed, relative to the project's deliverables folder. */
	files: string[];
	/** How it ended and what it did, rendered. */
	html: string;
}

export interface ClaudeRunSnapshot {
	run: ClaudeRun;
	output: string;
}

/** The payload of the `claude-run` event: the run as it now is, and the output just added, if any. */
export interface ClaudeRunEvent {
	run: ClaudeRun;
	chunk: { seq: number; text: string } | null;
}

// ---- Trello sync (feat/trello-sync) -----------------------------------------------------------------

export interface TrelloMember {
	id: string;
	username: string;
	fullName: string;
}

export interface TrelloBoard {
	id: string;
	name: string;
}

export interface TrelloList {
	id: string;
	name: string;
}

/** What the last Trello round did, and what it wants a person to know. */
export interface TrelloReport {
	cardsCreated: number;
	cardsUpdated: number;
	itemsCreated: number;
	itemsUpdated: number;
	converted: number;
	detached: number;
	conflicts: number;
	/** Tasks without a list and cards without a column: left out. */
	unmatched: number;
	skipped: number;
	/** Items that are not syncing because their card is gone or no longer the person's. */
	unlinked: number;
	vaultWrites: number;
	notes: string[];
}

export interface TrelloStatus {
	/** There are a key and token, a person and at least one linked project. */
	configured: boolean;
	state: 'disabled' | 'idle' | 'syncing' | 'error';
	/** Changes made here that the sync hasn't looked at yet. */
	pending: boolean;
	lastSyncAt: string | null;
	lastError: string | null;
	summary: string | null;
	report: TrelloReport | null;
}

export interface TrelloProject {
	slug: string;
	name: string;
	/** The Trello board the project is linked to; a project with none isn't synced. */
	board: string | null;
	boardName: string | null;
}

/** An item that stopped syncing because its card was archived, deleted, moved or is no longer the person's. */
export interface TrelloUnlinkedItem {
	project: string;
	projectName: string;
	kind: 'task' | 'story';
	id: string;
	title: string;
	reason: string;
}

export interface TrelloView {
	/** The key and token themselves never reach the screen. */
	hasKey: boolean;
	keyHint: string;
	hasToken: boolean;
	tokenHint: string;
	/** The one person whose cards are synced. */
	member: TrelloMember | null;
	configured: boolean;
	projects: TrelloProject[];
	unlinked: TrelloUnlinkedItem[];
	status: TrelloStatus;
	settingsFile: string;
	secretsFile: string;
}

export interface TrelloColumnChoice {
	id: string;
	name: string;
	/** "auto" (the list with the same name), "none" (not synced) or the id of a list. */
	choice: string;
	listId: string | null;
	listName: string | null;
}

export interface TrelloMapping {
	lists: TrelloList[];
	columns: TrelloColumnChoice[];
	/** Lists no column uses: their cards aren't added. */
	unusedLists: string[];
}

// ---- Updating Ledger from inside the app (see src-tauri/src/updates.rs) -----------------------------

/** A release newer than this one, as the notice shows it. */
export interface UpdateInfo {
	kind: 'release';
	version: string;
	currentVersion: string;
	/** When it was published (ISO), if the release says. */
	date: string | null;
	/** The first number goes up: something may work differently afterwards. */
	major: boolean;
	/** The release notes, rendered, without the breaking changes. */
	notesHtml: string;
	/** The "Breaking changes" section, rendered, shown apart. */
	breakingHtml: string | null;
}

/** One of the commits a development build's branch gained since the running build. */
export interface BranchCommit {
	hash: string;
	subject: string;
	/** The first line marks it as breaking (`feat!:`). */
	breaking: boolean;
}

/** What a development build's runner (`pnpm run dev:app`) holds back, and the commits that came with it (see devupdates.rs). */
export interface BranchUpdate {
	kind: 'branch';
	/** The branch the checkout is on, or `HEAD` when it is on a commit. */
	branch: string;
	/** The commit the running build was made from (short), and the one the checkout is at now (full). */
	head: string;
	target: string;
	/** Commits the branch gained since; the newest are listed, `more` is how many are left out. */
	behind: number;
	commits: BranchCommit[];
	more: number;
	/** Which change this is: the same one is announced once, and *Not now* holds until another comes. */
	generation: number;
	/** Updating restarts the app (the Rust side or the packages changed); otherwise only the window reloads. */
	restart: boolean;
	/** The new Rust build: none needed, being built, ready, or failing. */
	build: 'none' | 'building' | 'ready' | 'failed';
	/** The first error of a build that failed. */
	error: string | null;
	/** `package.json` or `pnpm-lock.yaml` changed: the packages are installed when you update. */
	dependencies: boolean;
}

/** What there is to install: a release, or new commits on a development build's branch. */
export type Announced = UpdateInfo | BranchUpdate;

/** Settings → About. */
export interface UpdateView {
	currentVersion: string;
	/** Look by itself shortly after starting and every few hours. */
	automatic: boolean;
	/** Why this build can't look for anything newer (a debug build `pnpm run dev:app` didn't start, or no updater key yet). */
	unavailable: string | null;
	/** A development build, which hears of the changes in its checkout instead of the releases. */
	devBuild: boolean;
	/** The branch its checkout is on, once a look has found it. */
	following: string | null;
	available: Announced | null;
	/** When a check last worked (ISO). */
	checkedAt: string | null;
	/** Where the choice is kept: the app's config, on this machine only. */
	configFile: string;
}

export interface UpdateCheck {
	available: Announced | null;
	message: string;
}

/** What to do about Claude Code runs that a restart would stop: ask, wait for them, or stop them. */
export type RunsChoice = 'ask' | 'wait' | 'stop';

/**
 * Where installing got to when it couldn't restart (yet). A release that installs ends in a restart; a development build
 * ends in `applied`: it restarts into the new build when `restart` says so, or the window reloads.
 */
export type InstallStep =
	| { state: 'wait'; waitingFor: string[] }
	| { state: 'ask'; runs: string[] }
	| { state: 'applied'; commits: number; head: string; restart: boolean };

/** The `update-progress` event. */
export interface UpdateProgress {
	downloaded: number;
	total: number | null;
}

// ---- the app's log ----------------------------------------------------------------------------------

/** `debug` entries are only kept in a debug capture. */
export type LogLevel = 'error' | 'warn' | 'info' | 'debug';

/** A line of the app's log: an error, or something worth knowing when one happens. */
export interface LogEntry {
	at: string;
	level: LogLevel;
	/** What it came from: sync, trello, app, panic, ui, or the command that failed. */
	source: string;
	message: string;
}

/** This build, this machine and where things are kept, for the About screen and for reports. */
export interface AboutInfo {
	version: string;
	tauriVersion: string;
	os: string;
	arch: string;
	logFile: string;
	/** The most the log may take on disk, in bytes. */
	logMaxBytes: number;
	configDir: string;
	homeDir: string;
}

/** Whether a debug capture runs, since when, and how much it has recorded. */
export interface CaptureStatus {
	active: boolean;
	startedAt: string | null;
	events: number;
}

/** Something a `[[link]]` in a project's markdown can point at (see links.ts for how a typed target is matched to one). */
export interface LinkTarget {
	kind: 'thread' | 'deliverable' | 'feedback';
	/** The thread's slug, the deliverable's live path, or the feedback's id. */
	id: string;
	/** A thread's title, a deliverable's file name, the start of what a piece of feedback says. */
	title: string;
	/** Where it is: a deliverable's folder, the file a piece of feedback is on. */
	detail: string;
	/** For a deliverable and for feedback, the path that opens it (the live file or a version); empty for a thread. */
	open: string;
}

// ---- Portflow (feat/portflow-integration) ---------------------------------------------------------------

export interface PortflowReport {
	outcomes: number;
	people: number;
	collections: number;
	evidence: number;
	newVersions: number;
	/** Files of those versions that are only in Portflow (uploaded there; not copied). */
	inPortflowOnly: number;
	newFeedback: number;
	unplaced: number;
	filesTouched: number;
	notes: string[];
}

export interface PortflowStatus {
	state: 'off' | 'signed-out' | 'idle' | 'syncing' | 'error';
	signedIn: boolean;
	expiresAt: number | null;
	lastSyncAt: string | null;
	lastError: string | null;
	summary: string | null;
	report: PortflowReport | null;
}

export interface PortflowLink {
	project: string;
	projectName: string;
	semester: string;
	semesterName: string | null;
	collection: number;
	collectionName: string | null;
}

export interface PortflowView {
	enabled: boolean;
	launchUrl: string;
	defaultLaunchUrl: string;
	write: boolean;
	/** Each kind of write and whether it is on; `available` says whether this version can make it at all. */
	writes: { id: string; label: string; on: boolean; available: boolean }[];
	writesAvailable: boolean;
	portfolio: { id: number; userId: number; userName: string } | null;
	status: PortflowStatus;
	links: PortflowLink[];
	outcomes: number;
	people: number;
	unplaced: number;
}

export interface PortflowCollectionChoice {
	id: number;
	name: string;
	subtitle: string;
	section: string;
	evidenceCount: number;
	linkedProject: string | null;
	linkedSemester: string | null;
}

export interface EvidenceRef {
	project: string;
	projectName: string;
	slug: string;
	name: string;
	thread: string | null;
}

export interface PortflowPerson {
	id: number;
	name: string;
	role: string;
	shared: string;
	until: string | null;
	active: boolean;
}

export interface OutcomeRef {
	id: number;
	name: string;
}

export interface PortflowCollection {
	semester: string;
	semesterName: string | null;
	collection: number;
	name: string;
	subtitle: string;
	section: string;
	color: string;
	outcomes: OutcomeRef[];
}

export interface EvidenceVersion {
	number: number;
	portflowVersion: number | null;
	kind: string;
	createdAt: string;
	descriptionHtml: string;
	/** `path`: the Ledger deliverable, when the version came from Ledger; otherwise it is opened in Portflow. */
	files: { name: string; path: string | null }[];
	inLedger: boolean;
	portflowUrl: string | null;
}

/** An evidence description in the shape teachers expect: What? / Goal? / Why it contributes? (markdown each). */
export interface EvidenceParts {
	what: string;
	goal: string;
	why: string;
}

export interface Evidence {
	project: string;
	slug: string;
	name: string;
	thread: string | null;
	threadTitle: string | null;
	portflowId: number | null;
	kind: string;
	/** The description as Ledger keeps it (markdown), and its three parts when it has the shape teachers expect. */
	description: string;
	descriptionParts: EvidenceParts | null;
	descriptionHtml: string;
	files: { path: string; name: string; exists: boolean }[];
	outcomes: OutcomeRef[];
	versions: EvidenceVersion[];
	feedbackCount: number;
	openFeedback: number;
	/** Its page in Portflow (the latest version). */
	portflowUrl: string | null;
}

export interface ProjectPortflow {
	collections: PortflowCollection[];
	evidence: Evidence[];
	/** The collection the reading guide goes to (the one for the current semester); null when none is linked. */
	guideCollection: number | null;
	/** Whether the guide is written to that collection's description (Settings → Portflow, off by default). */
	guideWrite: boolean;
	/** The project's evidence items: what the reading guide can mention (`@Analysis`), and where a mention leads. */
	mentions: EvidenceMention[];
}

/** An evidence item as a reading guide mentions it: written `[@Name](portflow:evidence-<portflowId>)`. */
export interface EvidenceMention {
	portflowId: number;
	name: string;
	/** The Ledger thread it is placed in, if any. */
	thread: string | null;
	threadTitle: string | null;
	/** The deliverable it is in Ledger (its live file), once its files are downloaded: a mention leads there first. */
	deliverable: string | null;
}

export interface PortflowFeedback {
	project: string;
	projectName: string;
	thread: string;
	id: string;
	from: string | null;
	fromRole: string | null;
	evidence: string | null;
	evidenceName: string | null;
	createdAt: string;
	excerpt: string;
}

export interface PortflowImported {
	project: string;
	projectName: string;
	evidence: number;
	threads: number;
}

// ---- Portflow: semesters, sections and Epsilon+, the competence matrix (see portflow_epsilon.rs) ------

/** A section of the Portflow portfolio: one semester. */
export interface PortflowSection {
	id: number;
	name: string;
	template: string;
	importedAt: string | null;
	/** The Ledger semester it is. */
	semester: string | null;
	semesterName: string | null;
}

export interface EpsilonSemester {
	id: string;
	name: string;
	start: string;
	end: string;
	section: number | null;
	sectionName: string | null;
	current: boolean;
}

/** One sub-goal: one level of an activity in a layer ("Analysis-U2"), or of a goal like PS ("PS-2"). */
export interface EpsilonLevel {
	id: number;
	name: string;
	level: number;
	evidenceCount: number;
	evidence: EvidenceRef[];
	feedbackCount: number;
	descriptionHtml: string;
	/** The same, as markdown: what a thread or deliverable started from this level quotes. */
	description: string;
}

export interface EpsilonCell {
	/** U, S, O, H or I. */
	layer: string;
	/** The goal that is this cell's activity. */
	outcome: number;
	evidenceCount: number;
	feedbackCount: number;
	/** The highest level that has evidence. */
	level: number | null;
	levels: EpsilonLevel[];
	/** The level aimed for (its row's), and whether evidence reaches it. */
	target: number | null;
	reached: boolean;
}

export interface EpsilonSkill {
	outcome: number;
	/** What intended development calls it: its nickname, or its name ("PS"). */
	key: string;
	name: string;
	nickname: string;
	evidenceCount: number;
	level: number | null;
	levels: EpsilonLevel[];
	target: number | null;
	reached: boolean;
}

/** A semester's intended development: a level per layer (a whole row) and per level-only goal, and its overview. */
export interface EpsilonGoals {
	layers: Record<string, number>;
	goals: Record<string, number>;
	/** The learning plan, an evidence item: "<project>/<evidence slug>". */
	learningPlan: string | null;
	studyGuide: string | null;
}

export interface EpsilonPlan {
	project: string;
	projectName: string;
	evidence: string;
	name: string;
	thread: string | null;
	version: number | null;
	deliveredAt: string | null;
}

export interface EvidenceChoice {
	id: string;
	name: string;
	projectName: string;
}

export interface EpsilonView {
	semesters: EpsilonSemester[];
	semester: string | null;
	section: number | null;
	sectionName: string | null;
	goalSet: string;
	/** False when the semester has no section yet: then the newest goal set is shown. */
	scoped: boolean;
	columns: { outcome: number; name: string }[];
	rows: { layer: string; name: string; target: number | null }[];
	cells: EpsilonCell[];
	skills: EpsilonSkill[];
	maxLevel: number;
	evidenceCount: number;
	cellsWithEvidence: number;
	cellsTotal: number;
	goals: EpsilonGoals;
	targetsTotal: number;
	targetsReached: number;
	plan: EpsilonPlan | null;
	/** The projects linked to a collection in this semester. */
	linkedProjects: string[];
}

// ---- v1 ---------------------------------------------------------------------------------------------
// What the v1 vault adds (Requirements 0.27; the LoFi wireframes in Penpot): a project's kind, goal, end and people,
// a post's kind and labels, meeting notes as daily notes, feedback from the inbox, a deliverable's number and goal.
// camelCase on the wire like the rest.

/** Every project is one of three kinds, in the same vault and the same layout; the study parts are optional extras. */
export type ProjectKind = 'study' | 'hobby' | 'company';

/** The facts in a project's `00 Overview.md` frontmatter that 0.8 doesn't have. */
export interface ProjectMeta {
	slug: string;
	kind: ProjectKind;
	/** The most important thing a project has: asked first, shown on every project screen, fed to every deliverable. */
	goal: string;
	/** "semester" ends with the semester it runs in; otherwise a day of your own (ISO), or null for no end. */
	end: 'semester' | string | null;
	/** The semester a study project runs in, by name (NJ26). */
	semester: string | null;
	/** The label you mean the work to get; offered when publishing, and compared with what you publish. */
	intendedMmm: MmmLevel | null;
	/** Study projects have one; optional for the others. */
	mainQuestion: string | null;
	/** A company project's client (a company file in Companies/ is roadmap). */
	client: string | null;
	/** Shown in the overview's tag, `project/<tag>`, which daily notes and feedback use. */
	tag: string;
}

/** What a post is; set by you, shown on it, filtered on. Prompts and runs are posts too, but 0.8 keeps them apart. */
export type PostKind = 'thought' | 'devlog' | 'research' | 'reflection';

/** How sure you are of a post; set by hand only, never by Ledger. */
export type EpistemicLabel = 'fact' | 'to-check' | 'assumption';

/** A post's kind and labels, by post id. */
export interface PostMeta {
	postId: string;
	kind: PostKind;
	labels: EpistemicLabel[];
}

/** A person is a tag for now (`expert/sander`, `client/police/ries`): role first, then who. */
export type PersonTag = string;

/** One meeting in a daily note: a `## 10:00 Title #project/x` heading, a `With:` line of people, then your notes. */
export interface MeetingSection {
	/** "10:00", or null when the heading has no time. */
	time: string | null;
	title: string;
	/** Project tags (`thrym-player`), on the heading: the meeting belongs to each of these projects. */
	projects: string[];
	/** Person tags (`coach/frank`), on the `With:` line; the same as in feedback. */
	people: PersonTag[];
	/** Your notes, as you wrote them: no template, no required parts. */
	body: string;
}

/**
 * A daily note, `Meetings/<day>.md`: one per day, made in Ledger or Obsidian, holding that day's meetings, each under its
 * own heading. Its frontmatter (`type: daily`, `id`, `date`, `tags`) gathers the tags of its meetings, so the day is
 * found by project and by person in Ledger and in Obsidian.
 */
export interface DailyNote {
	/** ISO day; also the file's name, so a day has one note. */
	date: string;
	id: string;
	meetings: MeetingSection[];
	updatedAt: string;
}

/** Your own note (for yourself or for Claude), or feedback from someone else. */
export type FeedbackKind = 'own' | 'someone';

export type FeedbackRole = 'coach' | 'expert' | 'teammate' | 'client' | 'colleague' | 'other';

/** Someone who is part of a project (its coach, a teammate, the client), listed in its project.md. */
/** How a project's exported PDFs look and who they name (branding.rs), kept in its project.md. */
export interface Branding {
	author: string | null;
	/** `#rrggbb`; null for the PDF layout's own colour. */
	accent: string | null;
	ownName: string | null;
	/** The logo's path in the project's folder (`branding/own.png`). */
	ownLogo: string | null;
	/** null: the project's client. */
	clientName: string | null;
	clientLogo: string | null;
	/** Whether the cover shows the version's AI-use (MMM) label; null for the default: only in a study project. */
	showLabel: boolean | null;
}

export type BrandingSide = 'own' | 'client';

/** Whose PDF layout: a project's own, the vault's, or Ledger's (pdf/mod.rs). */
export type PdfLayoutScope = 'project' | 'vault' | 'ledger';

/** Which layout a project's PDFs are set in, and where the layouts of one's own are (vault-relative paths). */
export interface PdfLayoutView {
	used: PdfLayoutScope;
	project: string | null;
	vault: string | null;
}

export interface ProjectPerson {
	project: string;
	name: string;
	role: FeedbackRole;
	/** Their Portflow id, when they are someone in Portflow. */
	portflowUser: number | null;
	/** `role/name` (`coach/frank-de-vries`), as meetings and feedback tag them; Ledger fills it in. */
	tag: PersonTag;
}

/** A piece of feedback in the inbox: with its project, the title of the thread it is kept in, and its story's summary. */
export interface InboxFeedback extends Feedback {
	project: string;
	projectName: string;
	threadTitle: string;
	storyTitle: string | null;
}

/** Feedback written down from the inbox. It lands on a deliverable (or a version), a user story, or the project as a
 *  whole, and is kept in a thread: the one named, or else the project's most recent one (or a new one, "Feedback"). */
export interface FeedbackRecord {
	project: string;
	thread: string | null;
	/** A live path, or a published version ("plan@v1.0.md"). */
	deliverable: string | null;
	story: string | null;
	kind: FeedbackKind;
	from: FeedbackFrom;
	daily: string | null;
	body: string;
	/** When it was given, ISO 8601. */
	at: string | null;
}

/** A deliverable's v1 facts: its number, its goal and its own label (Man-Made switches AI writing off for it). */
/** A deliverable's evidence description (What? / Goal? / Why it contributes?), kept in Ledger for when it is handed in.
 *  Empty when it has none; a Portflow evidence item has its own (`Evidence.description`). */
export interface DeliverableDescription {
	project: string;
	path: string;
	description: string;
	descriptionHtml: string;
	descriptionParts: EvidenceParts | null;
}

export interface DeliverableMeta {
	project: string;
	/** The live path, as 0.8 names it. */
	path: string;
	/** `01`, `02`: given in the order they are made, kept on rename, never renumbered. */
	number: number;
	goal: string;
	mmm: MmmLevel | null;
	/** The question it answers: "main" or a sub-question's text. */
	answers: string | null;
	template: string | null;
}

export interface DeliverableDraft {
	name: string;
	goal: string;
	template: string | null;
	answers: string | null;
	mmm: MmmLevel | null;
	/** Who writes the first draft: you, or Claude Code from a prompt (off by default; never for Man-Made). */
	firstDraft: 'me' | 'claude';
}

/** Publishing a version with another label than the project's intended one: your answer is kept with the version. */
export interface LabelConfirmation {
	project: string;
	path: string;
	version: string;
	intended: MmmLevel;
	chosen: MmmLevel;
	reason: string;
	at: string;
}
