import { invoke } from '@tauri-apps/api/core';
import type { Anchor } from './anchors';
import { capture, captureDebug, logError } from './log.svelte';
import type {
	ActivityEvent,
	Branding,
	BrandingSide,
	PdfLayoutScope,
	PdfLayoutView,
	Vaults,
	ProjectMeta,
	LabelConfirmation,
	DeliverableMeta,
	DeliverableDescription,
	EvidenceParts as DescriptionParts,
	PostKind,
	EpistemicLabel,
	Client,
	ClientDraft,
	ConflictView,
	CreatedVault,
	EncryptionView,
	Person,
	PersonDraft,
	ProjectPerson,
	Semester,
	SemesterDraft,
	SemesterTemplate,
	Board,
	Bump,
	Authorship,
	Comparison,
	LiveChanges,
	Column,
	Deliverable,
	DeliverableSummary,
	Feedback,
	LinkTarget,
	FeedbackFrom,
	FeedbackRecord,
	DailyNote,
	InboxFeedback,
	Post,
	ProjectSummary,
	ProjectView,
	NewPromptInput,
	OnboardingStatus,
	Prompt,
	Provenance,
	Purpose,
	PublishChoices,
	ReaderPreset,
	Region,
	ScreenshotTaken,
	SidebarFold,
	LayoutSettings,
	SearchHit,
	Settings,
	Story,
	StoryDraft,
	StoryUpdate,
	Task,
	TaskUpdate,
	Template,
	TemplateDraft,
	ThreadSummary,
	ThreadView,
	Upload,
	VersionUpdate
} from './types';

/** Commands reject with a plain message; surface it as an `Error` so callers can `catch (e)` uniformly. */
/** Runs a backend command; a failure is written to the app's log (unless `log` is false) and thrown. */
async function call<T>(command: string, args?: Record<string, unknown>, log = true): Promise<T> {
	if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
		throw new Error('Ledger needs its desktop shell - start it with `pnpm run tauri dev`.');
	}
	const started = performance.now();
	try {
		const result = await invoke<T>(command, args);
		// A debug capture records each action by name and time only: the arguments hold what you wrote.
		if (log && capture.active) captureDebug(command, `ok in ${Math.round(performance.now() - started)} ms`);
		return result;
	} catch (e) {
		const message = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e);
		if (log) logError(command, `${message}${capture.active ? ` (after ${Math.round(performance.now() - started)} ms)` : ''}`);
		throw new Error(message);
	}
}

/** Like `call`, for a command that takes bytes: `body` is sent as is, and `headers` (percent-encoded, as headers carry
 *  only ASCII) say what they are. */
async function callRaw<T>(command: string, body: Uint8Array, headers: Record<string, string>): Promise<T> {
	if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
		throw new Error('Ledger needs its desktop shell - start it with `pnpm run tauri dev`.');
	}
	const encoded = Object.fromEntries(Object.entries(headers).map(([k, v]) => [k, encodeURIComponent(v)]));
	try {
		return await invoke<T>(command, body, { headers: encoded });
	} catch (e) {
		const message = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e);
		logError(command, message);
		throw new Error(message);
	}
}

export const api = {
	renderMarkdown: (body: string) => call<string>('render_markdown', { body }),
	/** What a [[link]] in this project's markdown can point at: its threads, deliverables and feedback. */
	listLinks: (project: string) => call<LinkTarget[]>('list_links', { project }),
	/** What the meeting notes link to: every project's targets, each with its project. */
	listVaultLinks: () => call<LinkTarget[]>('list_vault_links'),
	search: (query: string) => call<SearchHit[]>('search', { query }),
	/** Everything made in the vault and when, oldest first: the contribution graph's events. */
	listActivity: () => call<ActivityEvent[]>('list_activity'),

	// semesters and their sprints
	listSemesters: () => call<Semester[]>('list_semesters'),
	createSemester: (draft: SemesterDraft) => call<Semester>('create_semester', { draft }),
	updateSemester: (id: string, draft: SemesterDraft) => call<Semester>('update_semester', { id, draft }),
	deleteSemester: (id: string) => call<void>('delete_semester', { id }),
	/** What a new semester can start from (shared/templates/semesters), the earliest first. */
	listSemesterTemplates: () => call<SemesterTemplate[]>('list_semester_templates'),
	/** A new template without an `id`; otherwise a change to that one. */
	saveSemesterTemplate: (id: string | null, draft: SemesterDraft) => call<SemesterTemplate>('save_semester_template', { id, draft }),
	deleteSemesterTemplate: (id: string) => call<void>('delete_semester_template', { id }),

	// projects
	listProjects: () => call<ProjectSummary[]>('list_projects'),
	createProject: (name: string) => call<ProjectSummary>('create_project', { name }),
	renameProject: (slug: string, name: string) => call<ProjectSummary>('rename_project', { slug, name }),
	getProject: (slug: string) => call<ProjectView>('get_project', { slug }),
	saveReadme: (slug: string, text: string) => call<ProjectView>('save_readme', { slug, text }),
	deleteProject: (slug: string) => call<void>('delete_project', { slug }),
	/** A project's kind, goal and end (in its project.md); null while it has none. */
	getProjectMeta: (slug: string) => call<ProjectMeta | null>('get_project_meta', { slug }),
	listProjectMeta: () => call<ProjectMeta[]>('list_project_meta'),
	saveProjectMeta: (meta: ProjectMeta) => call<ProjectMeta>('save_project_meta', { slug: meta.slug, meta }),
	/** A deliverable's number, goal, own label, question and template (in its project's project.md); null without. */
	getDeliverableMeta: (project: string, path: string) => call<DeliverableMeta | null>('get_deliverable_meta', { project, path }),
	listDeliverableMeta: (project: string) => call<DeliverableMeta[]>('list_deliverable_meta', { project }),
	saveDeliverableMeta: (meta: DeliverableMeta) => call<DeliverableMeta>('save_deliverable_meta', { meta }),
	/** Its evidence description, kept for hand-in; empty when it has none. */
	getDeliverableDescription: (project: string, path: string) => call<DeliverableDescription>('get_deliverable_description', { project, path }),
	/** Its three parts, or `text` as it is; empty takes it away. */
	saveDeliverableDescription: (project: string, path: string, parts: DescriptionParts | null, text: string | null = null) =>
		call<DeliverableDescription>('save_deliverable_description', { project, path, parts, text }),
	/** A version published with another label than the project's intended one: the intended label and why, kept with it. */
	confirmLabel: (c: LabelConfirmation) =>
		call<void>('confirm_label', { project: c.project, path: c.path, version: c.version, intended: c.intended, reason: c.reason }),
	/** The number the project's next deliverable gets: never one given before. */
	nextDeliverableNumber: (project: string) => call<number>('next_deliverable_number', { project }),
	/** The people of one project (coach, teammates, client…), or of every project with null. */
	listProjectPeople: (slug: string | null = null) => call<ProjectPerson[]>('list_project_people', { slug }),
	/** Every client the vault knows, with its projects. */
	listClients: () => call<Client[]>('list_clients'),
	createClient: (draft: ClientDraft) => call<Client>('create_client', { draft }),
	updateClient: (id: string, draft: ClientDraft) => call<Client>('update_client', { id, draft }),
	/** Takes it out of the vault; its projects keep going without a client. */
	deleteClient: (id: string) => call<void>('delete_client', { id }),
	/** Everyone the vault knows, with the projects each is part of. */
	listPeople: () => call<Person[]>('list_people'),
	getPerson: (id: string) => call<Person>('get_person', { id }),
	createPerson: (draft: PersonDraft) => call<Person>('create_person', { draft }),
	updatePerson: (id: string, draft: PersonDraft) => call<Person>('update_person', { id, draft }),
	/** Takes them out of the vault and out of every project they were part of. */
	deletePerson: (id: string) => call<void>('delete_person', { id }),
	/** How the project's exported PDFs look and who they name. */
	getProjectBranding: (slug: string) => call<Branding>('get_project_branding', { slug }),
	/** Sets the names and the colour (the logos are set with `setProjectLogo`). */
	saveProjectBranding: (slug: string, branding: Branding) => call<Branding>('save_project_branding', { slug, branding }),
	/** Copies the picture at `source` into the project as that side's logo; null removes it. */
	setProjectLogo: (slug: string, side: BrandingSide, source: string | null) => call<Branding>('set_project_logo', { slug, side, source }),
	/** Takes your name, your own name and logo, and the colour from another project. */
	copyProjectBranding: (slug: string, from: string) => call<Branding>('copy_project_branding', { slug, from }),
	/** Which layout the project's PDFs are set in, and where the layouts of one's own are. */
	pdfLayout: (project: string) => call<PdfLayoutView>('pdf_layout', { project }),
	/** Starts a layout of one's own (a copy of the one used now) for the project or the whole vault. */
	customisePdfLayout: (project: string, scope: PdfLayoutScope) => call<PdfLayoutView>('customise_pdf_layout', { project, scope }),
	/** Deletes a layout of one's own, so the one under it is used again. */
	resetPdfLayout: (project: string, scope: PdfLayoutScope) => call<PdfLayoutView>('reset_pdf_layout', { project, scope }),
	/** Opens a layout of one's own in its program, or shows it in its folder. */
	openPdfLayout: (project: string, scope: PdfLayoutScope, reveal: boolean) => call<null>('open_pdf_layout', { project, scope, reveal }),
	/** Replaces who is part of the project: people of the vault, by their id, each with their role in it. */
	saveProjectPeople: (slug: string, people: Pick<ProjectPerson, 'person' | 'role'>[]) =>
		call<ProjectPerson[]>('save_project_people', { slug, people: people.map((p) => ({ person: p.person, role: p.role, project: slug })) }),

	// threads
	createThread: (project: string, title: string) => call<ThreadSummary>('create_thread', { project, title }),
	renameThread: (project: string, slug: string, title: string) => call<ThreadSummary>('rename_thread', { project, slug, title }),
	getThread: (project: string, slug: string) => call<ThreadView>('get_thread', { project, slug }),
	deleteThread: (project: string, slug: string) => call<void>('delete_thread', { project, slug }),
	linkThreads: (project: string, slug: string, otherSlug: string) => call<void>('link_threads', { project, slug, otherSlug }),
	unlinkThreads: (project: string, slug: string, otherSlug: string) => call<void>('unlink_threads', { project, slug, otherSlug }),

	// posts
	/** What a post is and how sure you are of it, in its frontmatter. */
	setPostMeta: (project: string, thread: string, id: string, kind: PostKind | null, labels: EpistemicLabel[]) =>
		call<Post>('set_post_meta', { project, thread, id, kind, labels }),
	addPost: (project: string, thread: string, body: string, replyToId: string | null = null) =>
		call<Post>('add_post', { project, thread, body, replyToId }),
	updatePost: (project: string, thread: string, id: string, body: string) => call<Post>('update_post', { project, thread, id, body }),
	deletePost: (project: string, thread: string, id: string) => call<void>('delete_post', { project, thread, id }),

	// prompts
	createPrompt: (project: string, thread: string, draft: NewPromptInput) => call<Prompt>('create_prompt', { project, thread, draft }),
	/** The prompts about a deliverable (any of its paths), oldest first. */
	listDeliverablePrompts: (project: string, path: string) => call<Prompt[]>('list_deliverable_prompts', { project, path }),
	/** Adds pieces of feedback to a prompt: they are linked, and put in its base text with the version each was given on. */
	attachPromptFeedback: (project: string, thread: string, id: string, feedback: string[]) =>
		call<Prompt>('attach_prompt_feedback', { project, thread, id, feedback }),
	detachPromptFeedback: (project: string, thread: string, id: string, feedback: string) =>
		call<Prompt>('detach_prompt_feedback', { project, thread, id, feedback }),
	updatePrompt: (project: string, thread: string, id: string, base: string, body: string) =>
		call<Prompt>('update_prompt', { project, thread, id, base, body }),
	/** What a new prompt's base text starts as, from the template or the feedback it is made from. */
	suggestPromptBase: (project: string, templateId: string | null, feedback: string[]) =>
		call<string>('suggest_prompt_base', { project, spec: { templateId, feedback } }),
	deletePrompt: (project: string, thread: string, id: string) => call<void>('delete_prompt', { project, thread, id }),
	linkPromptDeliverable: (project: string, thread: string, id: string, path: string) =>
		call<Prompt>('link_prompt_deliverable', { project, thread, id, path }),
	unlinkPromptDeliverable: (project: string, thread: string, id: string, path: string) =>
		call<Prompt>('unlink_prompt_deliverable', { project, thread, id, path }),
	createPromptDeliverable: (project: string, thread: string, id: string, name: string) =>
		call<Prompt>('create_prompt_deliverable', { project, thread, id, name }),

	// meeting notes: one daily note per day, meetings/<year>/<month>/<date>.md
	/** Newest first; with a project or a person tag, only the days with a meeting of theirs. */
	listDailyNotes: (filter: { project?: string; person?: string } = {}) =>
		call<DailyNote[]>('list_daily_notes', { project: filter.project ?? null, person: filter.person ?? null }),
	getDailyNote: (date: string) => call<DailyNote | null>('get_daily_note', { date }),
	/** Replaces the day's meetings; none (and nothing else in the note) removes the note, and null comes back. */
	saveDailyNote: (note: Pick<DailyNote, 'date' | 'meetings'>) => call<DailyNote | null>('save_daily_note', { note }),
	deleteDailyNote: (date: string) => call<void>('delete_daily_note', { date }),

	// feedback
	/** Every piece of feedback in the vault, newest first. */
	listAllFeedback: () => call<InboxFeedback[]>('list_all_feedback'),
	/** Your own note or someone else's, from the inbox; kept in a thread (see FeedbackRecord). */
	recordFeedback: (draft: FeedbackRecord) => call<Feedback>('record_feedback', { draft }),
	/** On the whole document, or on the passage `quote` when given; `from` says who gave it. */
	addFeedback: (project: string, thread: string, deliverablePath: string, body: string, quote: Anchor | null = null, from: FeedbackFrom | null = null) =>
		call<Feedback>('add_feedback', { project, thread, deliverablePath, body, quote, from }),
	/** Who gave the feedback; not for feedback from Portflow, which says so itself. */
	setFeedbackFrom: (project: string, thread: string, id: string, from: FeedbackFrom) =>
		call<Feedback>('set_feedback_from', { project, thread, id, from }),
	/** One piece of feedback; with `all`, it and every reply to it. */
	setFeedbackStatus: (project: string, thread: string, id: string, status: 'open' | 'resolved', all = false) =>
		call<Feedback>('set_feedback_status', { project, thread, id, status, all }),
	updateFeedback: (project: string, thread: string, id: string, body: string) =>
		call<Feedback>('update_feedback', { project, thread, id, body }),
	deleteFeedback: (project: string, thread: string, id: string) => call<void>('delete_feedback', { project, thread, id }),

	// uploads and project files
	uploadFiles: (project: string, thread: string, paths: string[], description: string) =>
		call<Upload[]>('upload_files', { project, thread, paths, description }),
	/** A file dropped on the window, saved in the thread: its bytes go as the raw request body, the rest in headers. */
	uploadDropped: async (project: string, thread: string, file: File) =>
		callRaw<Upload>('upload_dropped', new Uint8Array(await file.arrayBuffer()), { project, thread, name: file.name }),
	/** Every file uploaded in the project, from all its threads, newest first. */
	listProjectUploads: (project: string) => call<Upload[]>('list_project_uploads', { project }),
	updateUpload: (project: string, thread: string, id: string, description: string) =>
		call<Upload>('update_upload', { project, thread, id, description }),
	deleteUpload: (project: string, thread: string, id: string) => call<void>('delete_upload', { project, thread, id }),
	/** A file in the project's folder ("attachments/…", "deliverables/…"), as raw bytes. */
	readProjectFile: (project: string, path: string) => call<ArrayBuffer>('read_project_file', { project, path }),
	/** A picture in a project's folder as the window can show it: a JPEG XL one as PNG (see `shownMimeOf`). */
	readProjectImage: (project: string, path: string) => call<ArrayBuffer>('read_project_image', { project, path }),
	/** The preview of a picture being dropped onto the window, read from where it is on this machine. */
	previewDroppedImage: (path: string) => call<ArrayBuffer>('preview_dropped_image', { path }),
	openProjectFile: (project: string, path: string, reveal = false) => call<void>('open_project_file', { project, path, reveal }),

	// deliverables
	listDeliverables: (project: string) => call<DeliverableSummary[]>('list_deliverables', { project }),
	getDeliverable: (project: string, path: string) => call<Deliverable>('get_deliverable', { project, path }),
	createDeliverable: (project: string, name: string, body = '') => call<Deliverable>('create_deliverable', { project, name, body }),
	saveDeliverable: (project: string, path: string, body: string) => call<Deliverable>('save_deliverable', { project, path, body }),
	publishDeliverable: (project: string, path: string, bump: Bump, version: string | null = null, choices: PublishChoices | null = null) =>
		call<Deliverable>('publish_deliverable', { project, path, bump, version, choices }),
	/**
	 * Makes a PDF of a markdown deliverable (the live file or a published version) in the project's `exports/` folder, and
	 * opens it when `open` is set. Returns its path relative to the project's folder.
	 */
	exportDeliverablePdf: (project: string, path: string, open: boolean) => call<string>('export_deliverable_pdf', { project, path, open }),
	/** How the versions of a deliverable came to be, by thread. `path` is the live file or any of its versions. */
	getProvenance: (project: string, path: string) => call<Provenance>('get_provenance', { project, path }),
	/** What changed in a markdown deliverable from `from` to `to`: each a version ("0.3") or "live". */
	deliverableChanges: (project: string, path: string, from: string, to: string) =>
		call<Comparison>('deliverable_changes', { project, path, from, to }),
	/** The changes in the text being typed since a version (the newest when null): where, by whom, and how to undo each. */
	deliverableLiveChanges: (project: string, path: string, text: string, against: string | null = null) =>
		call<LiveChanges>('deliverable_live_changes', { project, path, text, against }),
	/** Who wrote what is in it at `at` (a version or "live"); with `focus`, only what that version wrote. */
	deliverableAuthors: (project: string, path: string, at: string, focus: string | null = null) =>
		call<Authorship>('deliverable_authors', { project, path, at, focus }),
	/** Says who made a version, or sets its AI-use label or change note. */
	updateVersionRecord: (project: string, path: string, version: string, update: VersionUpdate) =>
		call<Provenance>('update_version_record', { project, path, version, update }),
	deleteDeliverable: (project: string, path: string) => call<void>('delete_deliverable', { project, path }),
	/** Gives a deliverable another name (its extension stays); the answer is its new path. */
	renameDeliverable: (project: string, path: string, name: string) => call<string>('rename_deliverable', { project, path, name }),

	// templates
	listTemplates: (scope: string) => call<Template[]>('list_templates', { scope }),
	projectTemplates: (project: string) => call<Template[]>('project_templates', { project }),
	effectiveTemplates: (project: string) => call<Template[]>('effective_templates', { project }),
	createTemplate: (scope: string, draft: TemplateDraft) => call<Template>('create_template', { scope, draft }),
	/** Copies a vault-wide template into the project under the same id, so the project can change its own. */
	overrideTemplate: (project: string, id: string) => call<Template>('override_template', { project, id }),
	updateTemplate: (scope: string, id: string, draft: TemplateDraft) => call<Template>('update_template', { scope, id, draft }),
	/** The readers a template can name by a word. */
	templateReaderPresets: () => call<ReaderPreset[]>('template_reader_presets'),
	/** What a new deliverable in the project starts with from this template: its skeleton, with its readers noted. */
	templateSkeleton: (project: string, id: string) => call<string>('template_skeleton', { project, id }),
	deleteTemplate: (scope: string, id: string) => call<void>('delete_template', { scope, id }),

	// user stories
	listStories: (project: string) => call<Story[]>('list_stories', { project }),
	createStory: (project: string, draft: StoryDraft) => call<Story>('create_story', { project, draft }),
	updateStory: (project: string, id: string, updates: StoryUpdate) => call<Story>('update_story', { project, id, updates }),
	deleteStory: (project: string, id: string) => call<void>('delete_story', { project, id }),
	linkStoryThread: (project: string, id: string, threadSlug: string) => call<Story>('link_story_thread', { project, id, threadSlug }),
	unlinkStoryThread: (project: string, id: string, threadSlug: string) => call<Story>('unlink_story_thread', { project, id, threadSlug }),

	// the board
	getBoard: (project: string) => call<Board>('get_board', { project }),
	createTask: (project: string, title: string, body: string, storyId: string | null, threadSlug: string | null) =>
		call<Task>('create_task', { project, title, body, storyId, threadSlug }),
	updateTask: (project: string, id: string, updates: TaskUpdate) => call<Task>('update_task', { project, id, updates }),
	deleteTask: (project: string, id: string) => call<void>('delete_task', { project, id }),
	addColumn: (project: string, name: string) => call<Column[]>('add_column', { project, name }),
	renameColumn: (project: string, id: string, name: string) => call<Column[]>('rename_column', { project, id, name }),
	deleteColumn: (project: string, id: string) => call<Column[]>('delete_column', { project, id }),

	// the vault
	getSettings: () => call<Settings>('get_settings'),
	setVaultPath: (path: string, copy: boolean) => call<Settings>('set_vault_path', { path, copy }),
	// the vault picker
	listVaults: () => call<Vaults>('list_vaults'),
	/** A new vault: a folder called `name` in `parent`, which must not be there yet or be empty. With a passphrase it is encrypted. */
	createVault: (parent: string, name: string, passphrase: string | null = null) => call<CreatedVault>('create_vault', { parent, name, passphrase }),
	/** Opens an encrypted vault with its passphrase or recovery key; `keep` keeps its key in this computer's keychain. */
	unlockVault: (path: string, secret: string, keep: boolean) => call<Vaults>('unlock_vault', { path, secret, keep }),
	// the open vault's encryption
	vaultEncryption: () => call<EncryptionView>('vault_encryption'),
	changePassphrase: (current: string, passphrase: string) => call<EncryptionView>('change_passphrase', { current, passphrase }),
	/** The recovery key, for the passphrase: the one call that hands out the key. */
	recoveryKey: (passphrase: string) => call<string>('recovery_key', { passphrase }),
	/** Takes the key out of this computer's keychain: the next start asks for the passphrase. */
	forgetVaultKey: () => call<EncryptionView>('forget_vault_key'),
	/** An encrypted copy of the open vault in a new folder, opened at once; the vault of plain files stays as it is. */
	encryptedCopy: (parent: string, name: string, passphrase: string) => call<CreatedVault>('encrypted_copy', { parent, name, passphrase }),
	openVault: (path: string) => call<Vaults>('open_vault', { path }),
	/** Off the list; the folder is left alone. */
	forgetVault: (path: string) => call<Vaults>('forget_vault', { path }),
	// the guide for a new client
	onboardingStatus: () => call<OnboardingStatus>('onboarding_status'),
	skipOnboardingStep: (step: 'semester') => call<OnboardingStatus>('skip_onboarding_step', { step }),
	setOnboardingPurpose: (purpose: Purpose) => call<OnboardingStatus>('set_onboarding_purpose', { purpose }),
	finishOnboarding: () => call<OnboardingStatus>('finish_onboarding'),
	restartOnboarding: () => call<OnboardingStatus>('restart_onboarding'),
	reloadVault: () => call<void>('reload_vault'),
	/** The conflict copies in the vault: two machines changed the same file, and both versions were kept. */
	listConflicts: () => call<ConflictView[]>('list_conflicts'),
	/** Keeps one of the two: the copy (it replaces the original) or the original (the copy goes). */
	resolveConflict: (copy: string, keep: 'copy' | 'original') => call<ConflictView[]>('resolve_conflict', { copy, keep }),
	checkVaultChanges: () => call<boolean>('check_vault_changes'),

	// screenshots
	saveScreenshotHotkey: (hotkey: string) => call<Settings>('save_screenshot_hotkey', { hotkey }),
	saveScreenshotTool: (useCommand: boolean, command: string) => call<Settings>('save_screenshot_tool', { useCommand, command }),
	// the shell's layout, kept in this machine's settings for the vault
	layoutSettings: () => call<LayoutSettings>('layout_settings'),
	saveSidebarFold: (fold: SidebarFold) => call<LayoutSettings>('save_sidebar_fold', { fold }),
	pendingScreenshot: () => call<ScreenshotTaken | null>('pending_screenshot'),
	takeScreenshot: () => call<void>('take_screenshot'),
	/** Lets go of the hotkey while it is being re-recorded, and takes it back afterwards. */
	pauseScreenshotHotkey: (paused: boolean) => call<void>('pause_screenshot_hotkey', { paused }),
	screenshotImage: () => call<ArrayBuffer>('screenshot_image'),
	screenshotOverlayReady: () => call<void>('screenshot_overlay_ready'),
	screenshotFinish: (region: Region) => call<void>('screenshot_finish', { region }),
	screenshotCancel: () => call<void>('screenshot_cancel'),
	pendingScreenshotImage: () => call<ArrayBuffer>('pending_screenshot_image'),
	attachScreenshot: (project: string, thread: string, description: string) =>
		call<Upload>('attach_screenshot', { project, thread, description }),
	discardScreenshot: () => call<void>('discard_screenshot')
};

// ---- Run a prompt with Claude Code (a block of its own; see src-tauri/src/claude_code) ---------------

import type {
	ClaudeCodeCheck,
	ClaudeCodeSettings,
	ClaudeRun,
	ClaudeRunPreview,
	ClaudeRunSnapshot
} from './types';

export const claudeCodeApi = {
	settings: () => call<ClaudeCodeSettings>('claude_code_settings'),
	saveSettings: (cliPath: string, permissionMode: string, model: string, confine: boolean, autoRun: boolean, namePrompts: boolean) =>
		call<ClaudeCodeSettings>('save_claude_code_settings', { cliPath, permissionMode, model, confine, autoRun, namePrompts }),
	/** Runs `claude --version` with the path as typed (blank: look on the PATH). Doesn't save anything. */
	check: (cliPath: string) => call<ClaudeCodeCheck>('check_claude_code', { cliPath }),
	/** Everything a run would send and where, for the confirmation. `model`: one chosen for this run ("" for Claude Code's
	 *  default), in place of the one in Settings; left out, the one in Settings. */
	preview: (project: string, thread: string, id: string, model?: string) =>
		call<ClaudeRunPreview>('claude_code_preview', { project, thread, id, model: model ?? null }),
	/** `confirmed` is the prompt text from the preview: if the prompt changed since, nothing starts. `model` as for the preview. */
	start: (project: string, thread: string, id: string, confirmed: string, model?: string) =>
		call<ClaudeRun>('claude_code_start', { project, thread, id, confirmed, model: model ?? null }),
	cancel: (runId: string) => call<ClaudeRun>('claude_code_cancel', { runId }),
	runs: () => call<ClaudeRunSnapshot[]>('claude_code_runs'),
	run: (runId: string) => call<ClaudeRunSnapshot>('claude_code_run', { runId })
};

// ---- Trello sync (feat/trello-sync) -----------------------------------------------------------------

import type {
	TrelloBoard,
	TrelloMapping,
	TrelloMember,
	TrelloStatus,
	TrelloView
} from './types';

export const trelloApi = {
	trelloGet: () => call<TrelloView>('trello_get'),
	trelloStatus: () => call<TrelloStatus>('trello_status'),
	/** Tries the key and token on screen (a blank one means the saved one) without saving them. Resolves to a message naming the account. */
	trelloTest: (apiKey: string | null, token: string | null) => call<string>('trello_test', { apiKey, token }),
	/** A blank key or token keeps the saved one. */
	trelloSaveCredentials: (apiKey: string | null, token: string | null) =>
		call<TrelloView>('trello_save_credentials', { apiKey, token }),
	trelloDisconnect: () => call<TrelloView>('trello_disconnect'),
	trelloBoards: () => call<TrelloBoard[]>('trello_boards'),
	trelloBoardMembers: (board: string) => call<TrelloMember[]>('trello_board_members', { board }),
	/** Chooses the one person to sync for, from the members of `board`. */
	trelloSaveMember: (board: string, member: string) => call<TrelloView>('trello_save_member', { board, member }),
	/** Links a project to a board; `null` stops syncing the project. */
	trelloLinkProject: (project: string, board: string | null) => call<TrelloView>('trello_link_project', { project, board }),
	trelloProjectMapping: (project: string) => call<TrelloMapping>('trello_project_mapping', { project }),
	/** `choice` is "auto", "none" or a list id. */
	trelloSetColumnList: (project: string, column: string, choice: string) =>
		call<void>('trello_set_column_list', { project, column, choice }),
	trelloSyncNow: () => call<void>('trello_sync_now'),
	/** Gives an item that stopped syncing a new card on the next sync (the old card is left alone). */
	trelloRelinkItem: (project: string, kind: 'task' | 'story', id: string) =>
		call<TrelloView>('trello_relink_item', { project, kind, id })
};

// ---- Portflow (feat/portflow-integration) ------------------------------------------------------------
// Everything but `get`, `status` and `setEnabled` answers "Portflow is switched off" while it is off.

import type {
	Evidence,
	EvidenceParts,
	EvidenceChoice,
	EpsilonView,
	EpsilonGoals,
	PortflowSection,
	PortflowCollectionChoice,
	PortflowFeedback,
	PortflowImported,
	PortflowPerson,
	PortflowStatus,
	PortflowView,
	ProjectPortflow
} from './types';

export const portflowApi = {
	get: () => call<PortflowView>('portflow_get'),
	status: () => call<PortflowStatus>('portflow_status'),
	/** Off stops everything Portflow does; with `forget`, the sign-in window's cookies go too. */
	setEnabled: (enabled: boolean, forget = false) => call<PortflowView>('portflow_set_enabled', { enabled, forget }),
	/** Blank means the default (Fontys ICT's Canvas). */
	setLaunchUrl: (url: string) => call<PortflowView>('portflow_set_launch_url', { url }),
	/** Read only, or read and write with these writes on (only the ones this version can make are kept). */
	setAccess: (write: boolean, writes: string[]) => call<PortflowView>('portflow_set_access', { write, writes }),
	/** Opens the sign-in window and resolves once Portflow handed over its session. */
	signIn: () => call<PortflowView>('portflow_sign_in'),
	signOut: (forget = false) => call<PortflowView>('portflow_sign_out', { forget }),
	/** Signs in by itself from now on: the password goes in the system keychain, never in the vault. */
	saveLogin: (username: string, password: string) => call<PortflowView>('portflow_save_login', { username, password }),
	forgetLogin: () => call<PortflowView>('portflow_forget_login'),
	syncNow: () => call<void>('portflow_sync_now'),
	/** The portfolio's collections, asked from Portflow. */
	collections: () => call<PortflowCollectionChoice[]>('portflow_collections'),
	/** `collection` null unlinks the project for that semester. */
	linkCollection: (project: string, semester: string, collection: number | null) =>
		call<PortflowView>('portflow_link_collection', { project, semester, collection }),
	/** A new project from a collection, linked for `semester`; `threads`: a thread per evidence item. */
	importCollection: (collection: number, semester: string, name: string | null, threads: boolean) =>
		call<PortflowImported>('portflow_import_collection', { collection, semester, name, threads }),
	people: () => call<PortflowPerson[]>('portflow_people'),
	project: (project: string) => call<ProjectPortflow>('portflow_project', { project }),
	threadEvidence: (project: string, thread: string) => call<Evidence[]>('portflow_thread_evidence', { project, thread }),
	/** Into an existing thread, a new one (`newThread`, its title), or out of any (both null). */
	placeEvidence: (project: string, evidence: string, thread: string | null, newThread: string | null = null) =>
		call<Evidence[]>('portflow_place_evidence', { project, evidence, thread, newThread }),
	/** Saves an evidence item's description: its three parts, or (`parts` null) `text` as it is. Answers with the project's items. */
	saveEvidenceDescription: (project: string, evidence: string, parts: EvidenceParts | null, text: string | null = null) =>
		call<Evidence[]>('portflow_save_evidence_description', { project, evidence, parts, text }),
	/** Opens Portflow in the default browser, through Canvas (Portflow can't be opened at one item from outside). */
	open: () => call<void>('portflow_open'),
	recentFeedback: (limit = 8) => call<PortflowFeedback[]>('portflow_recent_feedback', { limit }),
	/** The HBO-i matrix of a semester (null: the current one), from the vault's mirror: no request goes to Portflow. */
	epsilon: (semester: string | null = null) => call<EpsilonView>('portflow_epsilon', { semester }),
	sections: () => call<PortflowSection[]>('portflow_sections'),
	/** `section` null: the semester is no section. */
	linkSection: (semester: string, section: number | null) => call<PortflowSection[]>('portflow_link_section', { semester, section }),
	/** Replaces a semester's intended development; answers with its matrix. */
	setIntended: (semester: string, goals: EpsilonGoals) => call<EpsilonView>('portflow_set_intended', { semester, goals }),
	evidenceChoices: () => call<EvidenceChoice[]>('portflow_evidence_choices')
};

// ---- Updating Ledger from inside the app (see src-tauri/src/updates.rs) -----------------------------

import type { InstallStep, RunsChoice, UpdateCheck, UpdateInfo, UpdateView } from './types';

export const updatesApi = {
	status: () => call<UpdateView>('update_status'),
	/** Whether to look for updates by itself; kept for this machine, not in the vault. */
	saveSettings: (automatic: boolean) => call<UpdateView>('save_update_settings', { automatic }),
	/** Looks now, whether or not automatic checks are on. */
	check: () => call<UpdateCheck>('check_for_updates'),
	/** Downloads (once), waits for syncs and runs, installs and restarts. Call again while the answer is to wait. */
	install: (choice: RunsChoice) => call<InstallStep>('install_update', { choice })
};

import type { AboutInfo, LogEntry } from './types';

export const logApi = {
	/** The last entries of the log, oldest first. */
	// Not logged: reading the log must not write to it, or a screen that follows the log could go round in circles.
	read: (limit?: number) => call<LogEntry[]>('log_read', { limit }, false),
	clear: () => call<void>('log_clear', undefined, false),
	/** Sets the most the log may take on disk; resolves to the limit as set. */
	setMaxBytes: (bytes: number) => call<number>('log_set_max_bytes', { bytes }),
	about: () => call<AboutInfo>('about_info', undefined, false)
};
