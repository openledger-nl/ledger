// Sample data for Storybook: a student's vault with two study projects, made up from scratch (nothing here comes from
// a real vault). The stories pass these to components as props, and the fake backend (backend.ts) answers the
// components' own calls from them.

import type { Anchor, Mark } from '$lib/anchors';
import type {
	ActivityEvent,
	Board,
	DailyNote,
	Deliverable,
	DeliverableSummary,
	EpsilonView,
	Evidence,
	EvidenceMention,
	Feedback,
	OnboardingStatus,
	Post,
	PostMeta,
	ProjectMeta,
	ProjectPerson,
	ProjectPortflow,
	ProjectSummary,
	ProjectView,
	Prompt,
	Provenance,
	Semester,
	Settings,
	Story,
	Task,
	Template,
	ThreadSummary,
	ThreadView,
	Upload
} from '$lib/types';

export const PROJECT = 'smart-greenhouse';
export const THREAD = 'sensor-research';

const at = (day: number, time = '10:00') => `2026-11-${String(day).padStart(2, '0')}T${time}:00.000Z`;

// ---- projects and threads ------------------------------------------------------------------------------------

function thread(slug: string, title: string, day: number, counts: Partial<ThreadSummary> = {}): ThreadSummary {
	return {
		projectSlug: PROJECT,
		slug,
		title,
		createdAt: at(day - 7),
		linkedThreadSlugs: [],
		linkedCount: 0,
		postCount: 3,
		promptCount: 1,
		feedbackCount: 2,
		uploadCount: 1,
		storyCount: 0,
		lastActivity: at(day),
		...counts
	};
}

export const threads: ThreadSummary[] = [
	thread(THREAD, 'Sensor research', 14),
	thread('project-plan', 'Project plan', 12, { feedbackCount: 4 }),
	thread('dashboard-prototype', 'Dashboard prototype', 9, { postCount: 7, uploadCount: 3 }),
	thread('retrospective-sprint-2', 'Retrospective sprint 2', 6, { promptCount: 0, feedbackCount: 0 })
];

export const projects: ProjectSummary[] = [
	{
		slug: PROJECT,
		name: 'Smart Greenhouse',
		createdAt: at(1),
		updatedAt: at(14),
		threads,
		deliverableCount: 3,
		storyCount: 2,
		taskCount: 5
	},
	{
		slug: 'portfolio-website',
		name: 'Portfolio website',
		createdAt: at(2),
		updatedAt: at(10),
		threads: [{ ...thread('design-system', 'Design system', 10), projectSlug: 'portfolio-website' }],
		deliverableCount: 1,
		storyCount: 0,
		taskCount: 2
	}
];

export const projectView: ProjectView = {
	slug: PROJECT,
	name: 'Smart Greenhouse',
	description: 'A greenhouse that waters itself, for the S3 semester.',
	readme: '# Smart Greenhouse\n\nWhat this project is, and how to read it.',
	readmeHtml: '<h1>Smart Greenhouse</h1><p>What this project is, and how to read it.</p>',
	threadCount: threads.length,
	deliverableCount: 3,
	attachmentCount: 4,
	storyCount: 2,
	taskCount: 5,
	openTaskCount: 3
};

export const projectMeta: ProjectMeta = {
	slug: PROJECT,
	kind: 'study',
	goal: 'Show that I can research, design and build an IoT product for a real client.',
	end: 'semester',
	semester: 's3-2026',
	intendedMmm: 'cyborg',
	mainQuestion: 'How can a small greenhouse keep its plants watered without a person checking it every day?',
	client: 'Kwekerij De Groene Vingers',
	tag: PROJECT
};

// ---- people ------------------------------------------------------------------------------------------------

export const people: ProjectPerson[] = [
	{ project: PROJECT, name: 'Frank de Vries', role: 'coach', portflowUser: 101, tag: 'coach/frank' },
	{ project: PROJECT, name: 'Sanne Jansen', role: 'expert', portflowUser: 102, tag: 'expert/sanne' },
	{ project: PROJECT, name: 'Sam Bakker', role: 'teammate', portflowUser: null, tag: 'teammate/sam' },
	{ project: PROJECT, name: 'Ries van Dam', role: 'client', portflowUser: null, tag: 'client/ries' }
];

export const projectOptions = projects.map((p) => ({ value: p.slug, label: p.name }));
export const peopleOptions = people.map((p) => ({ value: p.tag, label: p.name, hint: p.tag }));

// ---- posts, feedback, prompts, uploads ----------------------------------------------------------------------

function post(id: string, body: string, html: string, day: number, more: Partial<Post> = {}): Post {
	return {
		id,
		parentId: null,
		guidingQuestion: false,
		versionPublished: false,
		deliverablePath: null,
		deliverableVersion: null,
		promptId: null,
		promptTitle: null,
		promptThread: null,
		createdAt: at(day),
		updatedAt: at(day),
		body,
		html,
		kind: null,
		labels: [],
		...more
	};
}

export const posts: Post[] = [
	post('p-1', 'Which soil moisture sensor holds up in wet soil for a whole season?', '<p>Which soil moisture sensor holds up in wet soil for a whole season?</p>', 10, {
		guidingQuestion: true
	}),
	post(
		'p-2',
		'Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.',
		'<p>Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.</p>',
		11,
		{ kind: 'research', labels: ['to-check'] }
	),
	post('p-3', 'Agreed, let\'s order three to test.', '<p>Agreed, let\'s order three to test.</p>', 11, { parentId: 'p-2', kind: 'thought' }),
	post('p-4', 'Version 1.0 of sensor-research.md was published.', '<p>Version 1.0 of <strong>sensor-research.md</strong> was published.</p>', 13, {
		versionPublished: true,
		deliverablePath: 'sensor-research.md',
		deliverableVersion: '1.0'
	})
];

export const postById: Record<string, Post> = Object.fromEntries(posts.map((p) => [p.id, p]));

export const postMeta: PostMeta = { postId: 'p-2', kind: 'research', labels: ['to-check'] };

function feedback(id: string, body: string, day: number, more: Partial<Feedback> = {}): Feedback {
	return {
		id,
		threadSlug: THREAD,
		deliverablePath: 'sensor-research.md',
		deliverableVersion: '1.0',
		status: 'open',
		body,
		html: `<p>${body}</p>`,
		createdAt: at(day),
		updatedAt: at(day),
		quote: null,
		quotePrefix: null,
		quoteSuffix: null,
		givenOn: null,
		from: 'Frank de Vries',
		fromUser: 101,
		fromRole: 'coach',
		given: null,
		kind: 'someone',
		story: null,
		daily: null,
		portflowComment: null,
		replyTo: null,
		commentType: null,
		evidence: null,
		evidenceVersion: null,
		...more
	};
}

export const feedbackOpen = feedback('f-1', 'Good comparison. Add the price per sensor, the client will ask.', 13);
export const feedbackResolved = feedback('f-2', 'The conclusion needs a source.', 12, {
	status: 'resolved',
	from: 'Sanne Jansen',
	fromUser: 102,
	fromRole: 'expert'
});
export const feedbackQuoted = feedback('f-3', 'Is this still true for the newer model?', 14, {
	quote: 'corrode far less than resistive ones',
	quotePrefix: 'Capacitive sensors ',
	quoteSuffix: '. The datasheet',
	from: null,
	fromUser: null,
	fromRole: null,
	kind: 'own'
});
export const feedbackList: Feedback[] = [feedbackOpen, feedbackResolved, feedbackQuoted];

export const prompt: Prompt = {
	id: 'pr-1',
	threadSlug: THREAD,
	title: 'Compare soil moisture sensors',
	templateId: null,
	templateName: null,
	base: 'You help a student with an ICT research project.',
	baseHtml: '<p>You help a student with an ICT research project.</p>',
	body: 'Compare capacitive and resistive soil moisture sensors on lifetime, price and accuracy. Use sources.',
	html: '<p>Compare capacitive and resistive soil moisture sensors on lifetime, price and accuracy. Use sources.</p>',
	createdAt: at(11),
	updatedAt: at(11),
	deliverables: [{ path: 'sensor-research.md', exists: true, versions: ['1.0'] }],
	feedback: ['f-1'],
	runs: []
};

const SKETCH = '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200"><rect width="320" height="200" fill="#dfe8d8"/><path d="M40 160 L160 50 L280 160 Z" fill="#7aa36b"/><text x="160" y="185" text-anchor="middle" font-family="sans-serif" font-size="16" fill="#2d4a25">Greenhouse sketch</text></svg>';
export const sketchBytes = (): ArrayBuffer => new TextEncoder().encode(SKETCH).buffer as ArrayBuffer;

export const upload: Upload = {
	id: 'u-1',
	threadSlug: THREAD,
	file: 'attachments/greenhouse-sketch.svg',
	name: 'greenhouse-sketch.svg',
	size: SKETCH.length,
	uploadedAt: at(12),
	description: 'First sketch of the layout.',
	descriptionHtml: '<p>First sketch of the layout.</p>',
	image: true,
	exists: true
};

export const uploadFile: Upload = {
	...upload,
	id: 'u-2',
	file: 'attachments/datasheet.pdf',
	name: 'datasheet.pdf',
	size: 482_133,
	description: '',
	descriptionHtml: '',
	image: false
};

export const threadView: ThreadView = {
	projectSlug: PROJECT,
	slug: THREAD,
	title: 'Sensor research',
	description: '',
	createdAt: at(7),
	linkedThreads: [],
	posts,
	prompts: [prompt],
	feedback: feedbackList,
	uploads: [upload, uploadFile],
	stories: []
};

// ---- deliverables -------------------------------------------------------------------------------------------

const RESEARCH_MD =
	'# Sensor research\n\nWhich soil moisture sensor holds up in wet soil for a whole season?\n\n## Findings\n\nCapacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.\n\n## Advice\n\nOrder three capacitive sensors and test them for two sprints.\n';

export const deliverable: Deliverable = {
	path: 'sensor-research.md',
	basePath: 'sensor-research.md',
	name: 'sensor-research.md',
	kind: 'markdown',
	version: null,
	versions: ['1.0'],
	exists: true,
	size: RESEARCH_MD.length,
	modified: at(14),
	body: RESEARCH_MD,
	html: '<h1>Sensor research</h1><p>Which soil moisture sensor holds up in wet soil for a whole season?</p><h2>Findings</h2><p>Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.</p><h2>Advice</h2><p>Order three capacitive sensors and test them for two sprints.</p>',
	editable: true,
	prompts: [{ threadSlug: THREAD, threadTitle: 'Sensor research', id: 'pr-1', title: 'Compare soil moisture sensors' }],
	feedback: feedbackList,
	evidence: null,
	attachments: []
};

export const deliverables: DeliverableSummary[] = [
	{
		path: 'sensor-research.md',
		name: 'sensor-research.md',
		kind: 'markdown',
		size: RESEARCH_MD.length,
		modified: at(14),
		exists: true,
		versions: ['1.0'],
		promptCount: 1,
		openFeedback: 2,
		mmm: 'cyborg',
		latestAt: at(14),
		delivered: null,
		evidence: null,
		attachments: []
	},
	{
		path: 'Project plan.pdf',
		name: 'Project plan',
		kind: 'other',
		size: 284_221,
		modified: at(12),
		exists: true,
		versions: ['1.0', '2.0'],
		promptCount: 0,
		openFeedback: 1,
		mmm: 'man-made',
		latestAt: at(12),
		delivered: { number: 1, at: at(12) },
		evidence: 'project-plan',
		attachments: []
	}
];

export const provenance: Provenance = {
	groups: [
		{
			thread: { slug: THREAD, title: 'Sensor research', exists: true },
			firstAt: at(11),
			lastAt: at(13),
			entries: [
				{ kind: 'prompt', id: 'pr-1', title: 'Compare soil moisture sensors', at: at(11) },
				{
					kind: 'version',
					version: '1.0',
					path: 'sensor-research@v1.0.md',
					at: at(13),
					by: 'human',
					prompt: null,
					run: null,
					mmm: 'cyborg',
					note: 'First version for the coach.',
					size: RESEARCH_MD.length,
					sizeChange: null,
					sha256: null,
					record: null,
					post: { threadSlug: THREAD, id: 'p-4' }
				},
				{ kind: 'feedback', id: 'f-1', status: 'open', version: '1.0', excerpt: 'Good comparison. Add the price...', at: at(13) }
			]
		}
	],
	summary: { versions: 1, ai: 0, human: 1, unrecorded: 0, label: { level: 'cyborg', version: '1.0' } },
	draft: null,
	defaultThread: THREAD,
	stoppedRun: null,
	historyHasAi: false,
	suggestedLabel: 'cyborg'
};

// ---- Portflow -----------------------------------------------------------------------------------------------

export const evidence: Evidence = {
	project: PROJECT,
	slug: 'project-plan',
	name: 'Project plan',
	thread: 'project-plan',
	threadTitle: 'Project plan',
	portflowId: 2_100_001,
	kind: 'file',
	description:
		'**Evidence description**\n\n**What?** A written plan with the main question, the research approach and the planning.\n\n**Goal?** To give my coach and the client a clear picture of what I deliver this semester.\n\n**Why it contributes?** It shows that I analyse the problem before I build.',
	descriptionParts: {
		what: 'A written plan with the main question, the research approach and the planning.',
		goal: 'To give my coach and the client a clear picture of what I deliver this semester.',
		why: 'It shows that I analyse the problem before I build.'
	},
	descriptionHtml:
		'<p><strong>Evidence description</strong></p><p><strong>What?</strong> A written plan with the main question, the research approach and the planning.</p><p><strong>Goal?</strong> To give my coach and the client a clear picture of what I deliver this semester.</p><p><strong>Why it contributes?</strong> It shows that I analyse the problem before I build.</p>',
	files: [{ path: 'Project plan.pdf', name: 'Project plan.pdf', exists: true }],
	outcomes: [{ id: 1, name: 'Analysis' }],
	versions: [
		{
			number: 1,
			portflowVersion: 3_100_001,
			kind: 'file',
			createdAt: at(5),
			descriptionHtml: '',
			files: [{ name: 'Project plan.pdf', path: 'versions/Project plan@v1.0.pdf' }],
			inLedger: true,
			portflowUrl: null
		},
		{
			number: 2,
			portflowVersion: 3_100_002,
			kind: 'file',
			createdAt: at(12),
			descriptionHtml: '',
			files: [{ name: 'Project plan.pdf', path: 'versions/Project plan@v2.0.pdf' }],
			inLedger: true,
			portflowUrl: null
		}
	],
	feedbackCount: 2,
	openFeedback: 1,
	portflowUrl: null
};

export const mentions: EvidenceMention[] = [
	{ portflowId: 2_100_001, name: 'Project plan', thread: 'project-plan', threadTitle: 'Project plan', deliverable: 'Project plan.pdf' }
];

export const projectPortflow: ProjectPortflow = {
	collections: [
		{
			semester: 's3-2026',
			semesterName: 'S3 - najaar 2026',
			collection: 280_001,
			name: 'Smart Greenhouse',
			subtitle: 'IoT project',
			section: 'Projects',
			color: '#4f7a43',
			outcomes: [{ id: 1, name: 'Analysis' }]
		}
	],
	evidence: [evidence],
	guideCollection: 280_001,
	guideWrite: false,
	mentions
};

const LAYERS = [
	{ layer: 'user-interaction', name: 'User Interaction' },
	{ layer: 'software', name: 'Software' },
	{ layer: 'infrastructure', name: 'Infrastructure' }
];
const OUTCOMES = [
	{ outcome: 1, name: 'Analysis' },
	{ outcome: 2, name: 'Advise' },
	{ outcome: 3, name: 'Design' },
	{ outcome: 4, name: 'Realisation' }
];

export const epsilon: EpsilonView = {
	semesters: [{ id: 's3-2026', name: 'S3 - najaar 2026', start: '2026-09-01', end: '2027-01-31', section: 1, sectionName: 'ICT & Software', current: true }],
	semester: 's3-2026',
	section: 1,
	sectionName: 'ICT & Software',
	goalSet: 'S3',
	scoped: false,
	columns: OUTCOMES,
	rows: LAYERS.map((l) => ({ ...l, target: 2 })),
	cells: LAYERS.flatMap((l, i) =>
		OUTCOMES.map((o, j) => ({
			layer: l.layer,
			outcome: o.outcome,
			evidenceCount: (i + j) % 3,
			feedbackCount: (i + j) % 2,
			level: (i + j) % 3 === 0 ? null : ((i + j) % 3) + 0,
			levels: [],
			target: 2,
			reached: (i + j) % 3 === 2
		}))
	),
	skills: [],
	maxLevel: 4,
	evidenceCount: 3,
	cellsWithEvidence: 8,
	cellsTotal: 12,
	goals: { layers: { software: 2 }, goals: {}, learningPlan: null, studyGuide: null },
	targetsTotal: 12,
	targetsReached: 4,
	plan: null,
	linkedProjects: [PROJECT]
};

// ---- planning -----------------------------------------------------------------------------------------------

export const semester: Semester = {
	id: 's3-2026',
	name: 'S3 - najaar 2026',
	start: '2026-08-31',
	end: '2027-01-29',
	sprintWeeks: 3,
	notes: '',
	sprints: [
		{ name: 'Sprint 1', start: '2026-08-31', end: '2026-09-18', goal: 'Research the client\'s greenhouse.' },
		{ name: 'Sprint 2', start: '2026-09-21', end: '2026-10-09', goal: 'Prototype the sensor board.' },
		{ name: 'Sprint 3', start: '2026-10-12', end: '2026-10-30', goal: 'Dashboard.' },
		{ name: 'Sprint 4', start: '2026-11-09', end: '2026-11-27', goal: 'Test with the client.' }
	],
	vacations: [{ name: 'Herfstvakantie', start: '2026-10-31', end: '2026-11-08' }],
	createdAt: at(1),
	updatedAt: at(1)
};

export const story: Story = {
	projectSlug: PROJECT,
	id: 'st-1',
	summary: 'Water plants automatically',
	role: 'grower',
	want: 'the greenhouse to water plants when the soil is dry',
	reason: 'I don\'t have to check every day',
	notes: '',
	criteria: [
		{ text: 'Waters below 30% moisture', done: true },
		{ text: 'Never waters twice within an hour', done: false }
	],
	threads: [{ slug: THREAD, title: 'Sensor research' }],
	taskCount: 2,
	createdAt: at(3),
	updatedAt: at(9)
};

export const board: Board = {
	columns: [
		{ id: 'backlog', name: 'Backlog' },
		{ id: 'doing', name: 'Doing' },
		{ id: 'done', name: 'Done' }
	],
	backlogColumnId: 'backlog',
	tasks: [],
	stories: [{ id: 'st-1', summary: 'Water plants automatically' }]
};

export const task: Task = {
	id: 't-1',
	title: 'Solder the sensor board',
	body: '',
	columnId: 'doing',
	storyId: 'st-1',
	storySummary: 'Water plants automatically',
	threadSlug: THREAD,
	threadTitle: 'Sensor research',
	createdAt: at(9),
	updatedAt: at(9)
};

export const template: Template = {
	scope: '',
	id: 'research-report',
	name: 'Research report',
	trigger: 'research',
	questions: ['What is the main question?', 'Which sources did you use?'],
	body: 'Write a research report that answers {{main question}} using {{sources}}.',
	kinds: ['study'],
	readers: [
		{ who: 'coach', needs: '', keepFrom: '' },
		{ who: 'me', needs: '', keepFrom: '' }
	],
	readersShown: false,
	builtin: false,
	customisedBuiltin: false,
	createdAt: at(1),
	updatedAt: at(1),
	overridden: false
};

// ---- meetings -----------------------------------------------------------------------------------------------

export const dailyNote: DailyNote = {
	date: '2026-11-16',
	id: 'daily-2026-11-16',
	meetings: [
		{
			time: '10:00',
			title: 'Coach meeting',
			projects: [PROJECT],
			people: ['coach/frank'],
			body: 'Discussed the sensor research. [@Frank de Vries](person:coach/frank) wants the price per sensor in it.'
		},
		{ time: '14:30', title: 'Client call', projects: [PROJECT], people: ['client/ries', 'teammate/sam'], body: 'Ries is happy with the dashboard.' }
	],
	updatedAt: at(16)
};

// ---- the rest -------------------------------------------------------------------------------------------------

export const activity: ActivityEvent[] = Array.from({ length: 60 }, (_, i) => ({
	projectSlug: i % 3 ? PROJECT : 'portfolio-website',
	kind: (['post', 'feedback', 'version', 'prompt'] as const)[i % 4],
	at: new Date(Date.UTC(2026, 8, 1 + Math.floor(i * 1.3))).toISOString()
}));

export const onboarding: OnboardingStatus = { active: true, skipped: [] };

export const settings: Settings = {
	vaultPath: '/home/student/Documents/Ledger',
	vaultLocked: false,
	settingsFile: '/home/student/Documents/Ledger/.ledger/settings.json',
	secretsFile: '/home/student/Documents/Ledger/.ledger/.env',
	version: '0.8.0',
	sync: { repo: 'student/ledger-vault', branch: 'main', hasToken: true, tokenHint: '...a1b2', configured: true },
	status: {
		configured: true,
		repo: 'student/ledger-vault',
		branch: 'main',
		state: 'idle',
		pending: false,
		encrypted: false,
		lastSyncAt: at(16, '09:12'),
		lastError: null,
		summary: 'Up to date'
	},
	screenshot: {
		hotkey: 'CommandOrControl+Shift+S',
		defaultHotkey: 'CommandOrControl+Shift+S',
		problem: null,
		note: null,
		useCommand: false,
		command: '',
		presets: [],
		builtin: 'Built in',
		platform: 'linux'
	},
	encryption: { enabled: false, hasKey: false, keyId: null, problem: null, needsFreshTarget: false }
};

export const anchor: Anchor = { text: 'corrode far less than resistive ones', prefix: 'Capacitive sensors ', suffix: '. The datasheet' };
export const marks: Mark[] = [{ id: 'f-3', anchor, tone: 'open' }];

export const noop = () => {};
