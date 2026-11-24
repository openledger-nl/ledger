// Sample data for Storybook: a student's vault with two study projects, made up from scratch (nothing here comes from
// a real vault). The stories pass these to components as props, and the fake backend (backend.ts) answers the
// components' own calls from them.

import type { Anchor, Mark } from '$lib/anchors';
import type {
	Authorship,
	Comparison,
	LiveChanges,
	Branding,
	ActivityEvent,
	Vaults,
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
	Client,
	ConflictView,
	Person,
	ProjectPerson,
	ProjectPortflow,
	ProjectSummary,
	ProjectView,
	Prompt,
	Provenance,
	Semester,
	SemesterTemplate,
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
	{ project: PROJECT, person: 'frank-de-vries', name: 'Frank de Vries', role: 'coach', portflowUser: 101, tag: 'people/frank-de-vries' },
	{ project: PROJECT, person: 'sanne-jansen', name: 'Sanne Jansen', role: 'expert', portflowUser: 102, tag: 'people/sanne-jansen' },
	{ project: PROJECT, person: 'sam-bakker', name: 'Sam Bakker', role: 'teammate', portflowUser: null, tag: 'people/sam-bakker' },
	{ project: PROJECT, person: 'ries-van-dam', name: 'Ries van Dam', role: 'stakeholder', portflowUser: null, tag: 'people/ries-van-dam' }
];

export const projectOptions = projects.map((p) => ({ value: p.slug, label: p.name }));
/** Everyone the sample vault knows: the project's people, and someone who is in no project yet. */
export const vaultPeople: Person[] = [
	...people.map((p) => ({
		id: p.person,
		name: p.name,
		email: '',
		organisation: p.role === 'stakeholder' ? 'Kwekerij Van Dam' : p.role === 'teammate' ? '' : 'Fontys ICT',
		portflowUser: p.portflowUser,
		notes: '',
		projects: [{ project: PROJECT, projectName: 'Smart Greenhouse', role: p.role }],
		tag: p.tag,
		createdAt: at(1, '09:00'),
		updatedAt: at(1, '09:00')
	})),
	{
		id: 'lotte-smit',
		name: 'Lotte Smit',
		email: 'lotte@example.nl',
		organisation: 'Fontys ICT',
		portflowUser: null,
		notes: 'Assessor for the end of the semester.',
		projects: [],
		tag: 'people/lotte-smit',
		createdAt: at(2, '10:00'),
		updatedAt: at(2, '10:00')
	}
];

export const peopleOptions = people.map((p) => ({ value: p.tag, label: p.name, hint: p.tag }));

/** The branding of the sample project's PDFs: both sides named, your logo chosen, the client's not yet. */
export const branding: Branding = {
	author: 'Sam Student',
	accent: '#154273',
	ownName: 'Fontys ICT',
	ownLogo: 'branding/own.png',
	clientName: null,
	clientLogo: null,
	showLabel: null
};

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
			files: [{ name: 'Project plan.pdf', path: 'history/deliverables/Project plan/Project plan@v1.0.pdf' }],
			inLedger: true,
			portflowUrl: null
		},
		{
			number: 2,
			portflowVersion: 3_100_002,
			kind: 'file',
			createdAt: at(12),
			descriptionHtml: '',
			files: [{ name: 'Project plan.pdf', path: 'history/deliverables/Project plan/Project plan@v2.0.pdf' }],
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

/** NJ26, the semester template a new vault starts with. */
export const semesterTemplate: SemesterTemplate = {
	id: 'nj26',
	name: 'NJ26 - Semester X',
	start: '2026-08-31',
	end: '2027-01-24',
	sprintWeeks: 3,
	notes: '',
	sprints: [
		{ name: 'Kick-off', start: '2026-08-31', end: '2026-09-06', goal: '' },
		{ name: 'Sprint 0', start: '2026-09-07', end: '2026-09-20', goal: '' },
		{ name: 'Sprint 1', start: '2026-09-21', end: '2026-10-11', goal: '' },
		{ name: 'Sprint 2', start: '2026-10-12', end: '2026-11-08', goal: '' },
		{ name: 'Sprint 3', start: '2026-11-09', end: '2026-11-29', goal: '' },
		{ name: 'Sprint 4', start: '2026-11-30', end: '2026-12-20', goal: '' },
		{ name: 'Sprint 5', start: '2027-01-04', end: '2027-01-24', goal: '' }
	],
	vacations: [
		{ name: 'Autumn break', start: '2026-10-19', end: '2026-10-25' },
		{ name: 'Christmas break', start: '2026-12-21', end: '2027-01-03' }
	],
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
			people: ['frank-de-vries'],
			body: 'Discussed the sensor research. [[people/frank-de-vries]] wants the price per sensor in it.'
		},
		{ time: '14:30', title: 'Client call', projects: [PROJECT], people: ['ries-van-dam', 'sam-bakker'], body: 'Ries is happy with the dashboard.' }
	],
	updatedAt: at(16)
};

// ---- the rest -------------------------------------------------------------------------------------------------

export const activity: ActivityEvent[] = Array.from({ length: 60 }, (_, i) => ({
	projectSlug: i % 3 ? PROJECT : 'portfolio-website',
	kind: (['post', 'feedback', 'version', 'prompt'] as const)[i % 4],
	at: new Date(Date.UTC(2026, 8, 1 + Math.floor(i * 1.3))).toISOString()
}));

export const onboarding: OnboardingStatus = { active: true, skipped: [], purpose: 'study' };

export const settings: Settings = {
	vaultPath: '/home/student/Documents/Ledger',
	vaultLocked: false,
	settingsFile: '/home/student/.config/nl.openledger.ledger/vaults/3f9a0c1d2e4b5a6978c0d1e2f3a4b5c6/settings.json',
	version: '0.8.0',
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
	}
};

export const anchor: Anchor = { text: 'corrode far less than resistive ones', prefix: 'Capacitive sensors ', suffix: '. The datasheet' };
export const marks: Mark[] = [{ id: 'f-3', anchor, tone: 'open' }];

export const noop = () => {};

// ---- clients ----------------------------------------------------------------------------------------------

/** The clients of the sample vault. */
export const clients: Client[] = [
	{
		id: 'kwekerij-van-dam',
		name: 'Kwekerij Van Dam',
		website: 'https://kwekerijvandam.nl',
		notes: 'Family business, three greenhouses.',
		projects: [],
		createdAt: at(1, '09:00'),
		updatedAt: at(1, '09:00')
	}
];

// ---- conflict copies ------------------------------------------------------------------------------------

/** A post both machines changed: OneDrive kept the laptop's beside it. */
export const conflicts: ConflictView[] = [
	{
		copy: `projects/${PROJECT}/threads/${THREAD}/posts/n-2-LAPTOP.md`,
		original: `projects/${PROJECT}/threads/${THREAD}/posts/n-2.md`,
		text: true,
		originalText: '---\nid: n-2\n---\n\nCapacitive sensors corrode far less than resistive ones.\n',
		copyText: '---\nid: n-2\n---\n\nCapacitive sensors corrode far less than resistive ones. Ask the supplier for a price per sensor.\n',
		originalModified: at(16, '09:10'),
		copyModified: at(16, '09:12')
	}
];

// ---- the vault picker -------------------------------------------------------------------------------

/** The machine as it opens on the vault picker for the first time: no vault yet, none opened before. */
export const firstStart: Vaults = { open: false, locked: false, vaults: [], suggestedParent: '/home/student/Documents' };

/** A start after updating Ledger: the vault opened last is of an older format, so the picker asks to upgrade it. */
export const upgradeWaiting: Vaults = {
	open: false,
	locked: false,
	suggestedParent: '/home/student/Documents',
	vaults: [{ path: '/home/student/OneDrive/Thesis', name: 'Thesis', exists: true, open: false, refused: null, encrypted: false, needsKey: false, upgradeFrom: 1 }]
};

/** The vault the sample opens, and the other vaults this machine has opened. */
export const vaults: Vaults = {
	open: true,
	locked: false,
	suggestedParent: '/home/student/Documents',
	vaults: [
		{ path: '/home/student/OneDrive/Ledger', name: 'Ledger', exists: true, open: true, refused: null, encrypted: false, needsKey: false, upgradeFrom: null },
		{ path: '/home/student/OneDrive/Thesis', name: 'Thesis', exists: true, open: false, refused: null, encrypted: false, needsKey: false, upgradeFrom: 1 },
		{ path: '/home/student/Nextcloud/School 2025', name: 'School 2025', exists: true, open: false, refused: null, encrypted: true, needsKey: true, upgradeFrom: null },
		{ path: '/home/student/Documents/Hobby', name: 'Hobby', exists: true, open: false, refused: "/home/student/Documents/Hobby holds a vault from Ledger 0.x. Ledger 1.0 can't open it and doesn't change it: create a new vault, in a folder your cloud storage keeps in sync.", encrypted: false, needsKey: false, upgradeFrom: null },
		{ path: '/media/usb/Old vault', name: 'Old vault', exists: false, open: false, refused: null, encrypted: false, needsKey: false, upgradeFrom: null }
	]
};

// ---- what changed, and who wrote what (DocHistory) ---------------------------------------------------
// What the backend (history.rs) answers for sensor-research.md: v1.0 by you, v1.1 by Claude Code (a price table and two
// notes on the sensors), and a live file in which you moved the advice up, changed it and took a note out.

export const historyVersions = [
	{ version: '1.0', by: 'human' as const, run: null },
	{ version: '1.1', by: 'ai' as const, run: { id: 'run-1' } as never }
];

export const changes: Comparison = {
 "addedBy": [
  {
   "source": 1,
   "words": 36
  }
 ],
 "blocks": [
  {
   "by": null,
   "html": "<h1>Sensor research</h1>\n",
   "state": "same"
  },
  {
   "by": 1,
   "html": "<p>Which soil moisture sensor holds up in wet soil for a whole season<ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">, and what does it cost</ins>?</p>\n",
   "state": "changed"
  },
  {
   "by": null,
   "html": "<h2>Findings</h2>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<p>Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.</p>\n",
   "state": "same"
  },
  {
   "by": 1,
   "html": "<table><thead><tr><th><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Sensor</ins></th><th><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Price</ins></th><th><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Lifetime</ins></th></tr></thead><tbody>\n<tr><td><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Capacitive v2</ins></td><td><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">4 euro</ins></td><td><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">2 years</ins></td></tr>\n<tr><td><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Resistive</ins></td><td><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">1 euro</ins></td><td><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">3 months</ins></td></tr>\n</tbody></table>\n",
   "state": "added"
  },
  {
   "by": 1,
   "html": "<ul>\n<li><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Capacitive sensors need 3.3 V, which the board gives.</ins></li>\n</ul>\n",
   "state": "added"
  },
  {
   "by": 1,
   "html": "<ul>\n<li><ins class=\"dv-ins\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Resistive sensors drift as the probe corrodes.</ins></li>\n</ul>\n",
   "state": "added"
  },
  {
   "by": null,
   "html": "<h2>Advice</h2>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<p>Order three capacitive sensors and test them for two sprints.</p>\n",
   "state": "same"
  }
 ],
 "changed": 1,
 "from": "1.0",
 "hunks": [
  {
   "lines": [
    {
     "by": null,
     "new": 1,
     "old": 1,
     "parts": [
      {
       "emph": false,
       "text": "# Sensor research"
      }
     ],
     "tag": "same"
    },
    {
     "by": null,
     "new": 2,
     "old": 2,
     "parts": [],
     "tag": "same"
    },
    {
     "by": null,
     "new": null,
     "old": 3,
     "parts": [
      {
       "emph": false,
       "text": "Which soil moisture sensor holds up in wet soil for a whole "
      },
      {
       "emph": true,
       "text": "season?"
      }
     ],
     "tag": "del"
    },
    {
     "by": 0,
     "new": 3,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "Which soil moisture sensor holds up in wet soil for a whole "
      },
      {
       "emph": true,
       "text": "season, and what does it cost?"
      }
     ],
     "tag": "add"
    },
    {
     "by": null,
     "new": 4,
     "old": 4,
     "parts": [],
     "tag": "same"
    },
    {
     "by": null,
     "new": 5,
     "old": 5,
     "parts": [
      {
       "emph": false,
       "text": "## Findings"
      }
     ],
     "tag": "same"
    },
    {
     "by": null,
     "new": 6,
     "old": 6,
     "parts": [],
     "tag": "same"
    },
    {
     "by": null,
     "new": 7,
     "old": 7,
     "parts": [
      {
       "emph": false,
       "text": "Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors."
      }
     ],
     "tag": "same"
    },
    {
     "by": null,
     "new": 8,
     "old": 8,
     "parts": [],
     "tag": "same"
    },
    {
     "by": 1,
     "new": 9,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "| Sensor | Price | Lifetime |"
      }
     ],
     "tag": "add"
    },
    {
     "by": null,
     "new": 10,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "|---|---|---|"
      }
     ],
     "tag": "add"
    },
    {
     "by": 1,
     "new": 11,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "| Capacitive v2 | 4 euro | 2 years |"
      }
     ],
     "tag": "add"
    },
    {
     "by": 1,
     "new": 12,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "| Resistive | 1 euro | 3 months |"
      }
     ],
     "tag": "add"
    },
    {
     "by": null,
     "new": 13,
     "old": null,
     "parts": [],
     "tag": "add"
    },
    {
     "by": 1,
     "new": 14,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "- Capacitive sensors need 3.3 V, which the board gives."
      }
     ],
     "tag": "add"
    },
    {
     "by": 1,
     "new": 15,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "- Resistive sensors drift as the probe corrodes."
      }
     ],
     "tag": "add"
    },
    {
     "by": null,
     "new": 16,
     "old": null,
     "parts": [],
     "tag": "add"
    },
    {
     "by": null,
     "new": 17,
     "old": 9,
     "parts": [
      {
       "emph": false,
       "text": "## Advice"
      }
     ],
     "tag": "same"
    },
    {
     "by": null,
     "new": 18,
     "old": 10,
     "parts": [],
     "tag": "same"
    },
    {
     "by": null,
     "new": 19,
     "old": 11,
     "parts": [
      {
       "emph": false,
       "text": "Order three capacitive sensors and test them for two sprints."
      }
     ],
     "tag": "same"
    }
   ],
   "newStart": 1,
   "oldStart": 1
  }
 ],
 "moved": 0,
 "sources": [
  {
   "at": "2026-11-20T13:51:56.463Z",
   "by": "human",
   "prompt": null,
   "version": "1.0",
   "who": "You"
  },
  {
   "at": "2026-11-20T13:51:56.466Z",
   "by": "ai",
   "prompt": "Compare soil moisture sensors",
   "version": "1.1",
   "who": "Claude Code"
  },
  {
   "at": "2026-11-20T13:51:56.468Z",
   "by": "human",
   "prompt": null,
   "version": null,
   "who": "You"
  }
 ],
 "to": "1.1",
 "wordsAdded": 36,
 "wordsRemoved": 0
};

export const changesLive: Comparison = {
 "addedBy": [
  {
   "source": 2,
   "words": 6
  }
 ],
 "blocks": [
  {
   "by": null,
   "html": "<h1>Sensor research</h1>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<p>Which soil moisture sensor holds up in wet soil for a whole season, and what does it cost?</p>\n",
   "state": "same"
  },
  {
   "by": 0,
   "html": "<h2>Advice</h2>\n",
   "state": "moved"
  },
  {
   "by": 2,
   "html": "<p>Order three capacitive sensors and test them for <del class=\"dv-del\">two</del><ins class=\"dv-ins\" data-by=\"human\" data-o=\"2\" title=\"You, not published yet\">one sprint in the wettest</ins> <del class=\"dv-del\">sprints</del><ins class=\"dv-ins\" data-by=\"human\" data-o=\"2\" title=\"You, not published yet\">bed</ins>.</p>\n",
   "state": "moved"
  },
  {
   "by": null,
   "html": "<h2>Findings</h2>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<p>Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.</p>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<table><thead><tr><th>Sensor</th><th>Price</th><th>Lifetime</th></tr></thead><tbody>\n<tr><td>Capacitive v2</td><td>4 euro</td><td>2 years</td></tr>\n<tr><td>Resistive</td><td>1 euro</td><td>3 months</td></tr>\n</tbody></table>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<ul>\n<li>Capacitive sensors need 3.3 V, which the board gives.</li>\n</ul>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<ul>\n<li>Resistive sensors drift as the probe corrodes.</li>\n</ul>\n",
   "state": "removed"
  },
  {
   "by": null,
   "html": "",
   "state": "movedAway"
  },
  {
   "by": null,
   "html": "",
   "state": "movedAway"
  }
 ],
 "changed": 0,
 "from": "1.1",
 "hunks": [
  {
   "lines": [
    {
     "by": null,
     "new": 2,
     "old": 2,
     "parts": [],
     "tag": "same"
    },
    {
     "by": null,
     "new": 3,
     "old": 3,
     "parts": [
      {
       "emph": false,
       "text": "Which soil moisture sensor holds up in wet soil for a whole season, and what does it cost?"
      }
     ],
     "tag": "same"
    },
    {
     "by": null,
     "new": 4,
     "old": 4,
     "parts": [],
     "tag": "same"
    },
    {
     "by": 0,
     "new": 5,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "## Advice"
      }
     ],
     "tag": "add"
    },
    {
     "by": null,
     "new": 6,
     "old": null,
     "parts": [],
     "tag": "add"
    },
    {
     "by": 0,
     "new": 7,
     "old": null,
     "parts": [
      {
       "emph": false,
       "text": "Order three capacitive sensors and test them for one sprint in the wettest bed."
      }
     ],
     "tag": "add"
    },
    {
     "by": null,
     "new": 8,
     "old": null,
     "parts": [],
     "tag": "add"
    },
    {
     "by": null,
     "new": 9,
     "old": 5,
     "parts": [
      {
       "emph": false,
       "text": "## Findings"
      }
     ],
     "tag": "same"
    },
    {
     "by": null,
     "new": 10,
     "old": 6,
     "parts": [],
     "tag": "same"
    },
    {
     "by": null,
     "new": 11,
     "old": 7,
     "parts": [
      {
       "emph": false,
       "text": "Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors."
      }
     ],
     "tag": "same"
    }
   ],
   "newStart": 2,
   "oldStart": 2
  },
  {
   "lines": [
    {
     "by": null,
     "new": 16,
     "old": 12,
     "parts": [
      {
       "emph": false,
       "text": "| Resistive | 1 euro | 3 months |"
      }
     ],
     "tag": "same"
    },
    {
     "by": null,
     "new": 17,
     "old": 13,
     "parts": [],
     "tag": "same"
    },
    {
     "by": null,
     "new": 18,
     "old": 14,
     "parts": [
      {
       "emph": false,
       "text": "- Capacitive sensors need 3.3 V, which the board gives."
      }
     ],
     "tag": "same"
    },
    {
     "by": null,
     "new": null,
     "old": 15,
     "parts": [
      {
       "emph": false,
       "text": "- Resistive sensors drift as the probe corrodes."
      }
     ],
     "tag": "del"
    },
    {
     "by": null,
     "new": null,
     "old": 16,
     "parts": [],
     "tag": "del"
    },
    {
     "by": null,
     "new": null,
     "old": 17,
     "parts": [
      {
       "emph": false,
       "text": "## Advice"
      }
     ],
     "tag": "del"
    },
    {
     "by": null,
     "new": null,
     "old": 18,
     "parts": [],
     "tag": "del"
    },
    {
     "by": null,
     "new": null,
     "old": 19,
     "parts": [
      {
       "emph": false,
       "text": "Order three capacitive sensors and test them for two sprints."
      }
     ],
     "tag": "del"
    }
   ],
   "newStart": 16,
   "oldStart": 12
  }
 ],
 "moved": 2,
 "sources": [
  {
   "at": "2026-11-20T13:51:56.463Z",
   "by": "human",
   "prompt": null,
   "version": "1.0",
   "who": "You"
  },
  {
   "at": "2026-11-20T13:51:56.466Z",
   "by": "ai",
   "prompt": "Compare soil moisture sensors",
   "version": "1.1",
   "who": "Claude Code"
  },
  {
   "at": "2026-11-20T13:51:56.468Z",
   "by": "human",
   "prompt": null,
   "version": null,
   "who": "You"
  }
 ],
 "to": null,
 "wordsAdded": 6,
 "wordsRemoved": 9
};

export const authorship: Authorship = {
 "at": null,
 "blocks": [
  {
   "by": 0,
   "html": "<h1><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">Sensor research</span></h1>\n",
   "state": "same"
  },
  {
   "by": 0,
   "html": "<p><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">Which soil moisture sensor holds up in wet soil for a whole season</span><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">, and what does it cost</span><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">?</span></p>\n",
   "state": "same"
  },
  {
   "by": 0,
   "html": "<h2><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">Advice</span></h2>\n",
   "state": "same"
  },
  {
   "by": 0,
   "html": "<p><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">Order three capacitive sensors and test them for</span> <span class=\"dv-by\" data-by=\"human\" data-o=\"2\" title=\"You, not published yet\">one sprint in the wettest</span> <span class=\"dv-by\" data-by=\"human\" data-o=\"2\" title=\"You, not published yet\">bed</span><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">.</span></p>\n",
   "state": "same"
  },
  {
   "by": 0,
   "html": "<h2><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">Findings</span></h2>\n",
   "state": "same"
  },
  {
   "by": 0,
   "html": "<p><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.</span></p>\n",
   "state": "same"
  },
  {
   "by": 1,
   "html": "<table><thead><tr><th><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Sensor</span></th><th><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Price</span></th><th><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Lifetime</span></th></tr></thead><tbody>\n<tr><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Capacitive v2</span></td><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">4 euro</span></td><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">2 years</span></td></tr>\n<tr><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Resistive</span></td><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">1 euro</span></td><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">3 months</span></td></tr>\n</tbody></table>\n",
   "state": "same"
  },
  {
   "by": 1,
   "html": "<ul>\n<li><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Capacitive sensors need 3.3 V, which the board gives</span><span class=\"dv-by\" data-by=\"human\" data-o=\"0\" title=\"You, v1.0\">.</span></li>\n</ul>\n",
   "state": "same"
  }
 ],
 "sources": [
  {
   "at": "2026-11-20T13:51:56.463Z",
   "by": "human",
   "prompt": null,
   "version": "1.0",
   "who": "You"
  },
  {
   "at": "2026-11-20T13:51:56.466Z",
   "by": "ai",
   "prompt": "Compare soil moisture sensors",
   "version": "1.1",
   "who": "Claude Code"
  },
  {
   "at": "2026-11-20T13:51:56.468Z",
   "by": "human",
   "prompt": null,
   "version": null,
   "who": "You"
  }
 ],
 "trace": null,
 "words": [
  {
   "source": 0,
   "words": 39
  },
  {
   "source": 1,
   "words": 29
  },
  {
   "source": 2,
   "words": 6
  }
 ]
};

export const authorshipTrace: Authorship = {
 "at": null,
 "blocks": [
  {
   "by": null,
   "html": "<h1>Sensor research</h1>\n",
   "state": "same"
  },
  {
   "by": 1,
   "html": "<p>Which soil moisture sensor holds up in wet soil for a whole season<span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">, and what does it cost</span>?</p>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<h2>Advice</h2>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<p>Order three capacitive sensors and test them for one sprint in the wettest bed.</p>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<h2>Findings</h2>\n",
   "state": "same"
  },
  {
   "by": null,
   "html": "<p>Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.</p>\n",
   "state": "same"
  },
  {
   "by": 1,
   "html": "<table><thead><tr><th><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Sensor</span></th><th><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Price</span></th><th><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Lifetime</span></th></tr></thead><tbody>\n<tr><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Capacitive v2</span></td><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">4 euro</span></td><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">2 years</span></td></tr>\n<tr><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Resistive</span></td><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">1 euro</span></td><td><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">3 months</span></td></tr>\n</tbody></table>\n",
   "state": "same"
  },
  {
   "by": 1,
   "html": "<ul>\n<li><span class=\"dv-by\" data-by=\"ai\" data-o=\"1\" title=\"Claude Code, v1.1\">Capacitive sensors need 3.3 V, which the board gives</span>.</li>\n</ul>\n",
   "state": "same"
  }
 ],
 "sources": [
  {
   "at": "2026-11-20T13:51:56.463Z",
   "by": "human",
   "prompt": null,
   "version": "1.0",
   "who": "You"
  },
  {
   "at": "2026-11-20T13:51:56.466Z",
   "by": "ai",
   "prompt": "Compare soil moisture sensors",
   "version": "1.1",
   "who": "Claude Code"
  },
  {
   "at": "2026-11-20T13:51:56.468Z",
   "by": "human",
   "prompt": null,
   "version": null,
   "who": "You"
  }
 ],
 "trace": {
  "kept": 29,
  "removedBy": [
   {
    "source": 2,
    "words": 7
   }
  ],
  "source": 1,
  "wrote": 36
 },
 "words": [
  {
   "source": 0,
   "words": 39
  },
  {
   "source": 1,
   "words": 29
  },
  {
   "source": 2,
   "words": 6
  }
 ]
};

// The live file of sensor-research.md being written, and its changes as the editor's margin shows them: since v1.1 (the
// newest), and since v1.0 (going through what Claude Code changed in v1.1).
export const liveText: string = "# Sensor research\n\nWhich soil moisture sensor holds up in wet soil for a whole season, and what does it cost?\n\n## Findings\n\nCapacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors, and our own test agrees.\n\n| Sensor | Price | Lifetime |\n|---|---|---|\n| Capacitive v2 | 4 euro | 2 years |\n| Resistive | 1 euro | 3 months |\n\n- Capacitive sensors need 3.3 V, which the board gives.\n\n## Advice\n\nOrder three capacitive sensors and test them for one sprint in the wettest bed.\n";

export const liveChanges: LiveChanges = {
 "against": "1.1",
 "changes": [
  {
   "by": 2,
   "deletes": [],
   "from": 124,
   "id": "b223fe832b87f77a",
   "inserts": [
    {
     "by": 2,
     "from": 216,
     "to": 241
    }
   ],
   "kind": "changed",
   "old": "Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.",
   "revert": [
    {
     "from": 124,
     "insert": "Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.",
     "to": 242
    }
   ],
   "to": 242,
   "wordsAdded": 5,
   "wordsRemoved": 0
  },
  {
   "by": null,
   "deletes": [
    {
     "at": 417,
     "block": true,
     "text": "- Resistive sensors drift as the probe corrodes."
    }
   ],
   "from": 417,
   "id": "1e3baef5cc333852",
   "inserts": [],
   "kind": "removed",
   "old": "- Resistive sensors drift as the probe corrodes.",
   "revert": [
    {
     "from": 417,
     "insert": "- Resistive sensors drift as the probe corrodes.\n\n",
     "to": 417
    }
   ],
   "to": 417,
   "wordsAdded": 0,
   "wordsRemoved": 7
  },
  {
   "by": 2,
   "deletes": [
    {
     "at": 477,
     "block": false,
     "text": "two"
    },
    {
     "at": 503,
     "block": false,
     "text": "sprints"
    }
   ],
   "from": 428,
   "id": "88a0136b9b75b4d0",
   "inserts": [
    {
     "by": 2,
     "from": 477,
     "to": 502
    },
    {
     "by": 2,
     "from": 503,
     "to": 506
    }
   ],
   "kind": "changed",
   "old": "Order three capacitive sensors and test them for two sprints.",
   "revert": [
    {
     "from": 428,
     "insert": "Order three capacitive sensors and test them for two sprints.",
     "to": 507
    }
   ],
   "to": 507,
   "wordsAdded": 6,
   "wordsRemoved": 2
  }
 ],
 "sources": [
  {
   "at": "2026-11-20T14:33:48.787Z",
   "by": "human",
   "prompt": null,
   "version": "1.0",
   "who": "You"
  },
  {
   "at": "2026-11-20T14:33:48.790Z",
   "by": "ai",
   "prompt": null,
   "version": "1.1",
   "who": "Claude Code"
  },
  {
   "at": "2026-11-20T14:33:48.789Z",
   "by": "human",
   "prompt": null,
   "version": null,
   "who": "You"
  }
 ]
};

export const liveChangesReview: LiveChanges = {
 "against": "1.0",
 "changes": [
  {
   "by": 1,
   "deletes": [],
   "from": 19,
   "id": "716d63bd197a4de8",
   "inserts": [
    {
     "by": 1,
     "from": 85,
     "to": 108
    }
   ],
   "kind": "changed",
   "old": "Which soil moisture sensor holds up in wet soil for a whole season?",
   "revert": [
    {
     "from": 19,
     "insert": "Which soil moisture sensor holds up in wet soil for a whole season?",
     "to": 109
    }
   ],
   "to": 109,
   "wordsAdded": 5,
   "wordsRemoved": 0
  },
  {
   "by": 2,
   "deletes": [],
   "from": 124,
   "id": "b223fe832b87f77a",
   "inserts": [
    {
     "by": 2,
     "from": 216,
     "to": 241
    }
   ],
   "kind": "changed",
   "old": "Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.",
   "revert": [
    {
     "from": 124,
     "insert": "Capacitive sensors corrode far less than resistive ones. The datasheet says 2 years outdoors.",
     "to": 242
    }
   ],
   "to": 242,
   "wordsAdded": 5,
   "wordsRemoved": 0
  },
  {
   "by": 1,
   "deletes": [],
   "from": 244,
   "id": "1a25e61982c33777",
   "inserts": [
    {
     "by": 1,
     "from": 246,
     "to": 252
    },
    {
     "by": 1,
     "from": 255,
     "to": 260
    },
    {
     "by": 1,
     "from": 263,
     "to": 271
    },
    {
     "by": 1,
     "from": 290,
     "to": 303
    },
    {
     "by": 1,
     "from": 306,
     "to": 312
    },
    {
     "by": 1,
     "from": 315,
     "to": 322
    },
    {
     "by": 1,
     "from": 327,
     "to": 336
    },
    {
     "by": 1,
     "from": 339,
     "to": 345
    },
    {
     "by": 1,
     "from": 348,
     "to": 356
    }
   ],
   "kind": "added",
   "old": "",
   "revert": [
    {
     "from": 244,
     "insert": "",
     "to": 360
    }
   ],
   "to": 358,
   "wordsAdded": 14,
   "wordsRemoved": 0
  },
  {
   "by": 1,
   "deletes": [],
   "from": 360,
   "id": "34c283b5ea06b2db",
   "inserts": [
    {
     "by": 1,
     "from": 362,
     "to": 415
    }
   ],
   "kind": "added",
   "old": "",
   "revert": [
    {
     "from": 360,
     "insert": "",
     "to": 417
    }
   ],
   "to": 415,
   "wordsAdded": 10,
   "wordsRemoved": 0
  },
  {
   "by": 2,
   "deletes": [
    {
     "at": 477,
     "block": false,
     "text": "two"
    },
    {
     "at": 503,
     "block": false,
     "text": "sprints"
    }
   ],
   "from": 428,
   "id": "88a0136b9b75b4d0",
   "inserts": [
    {
     "by": 2,
     "from": 477,
     "to": 502
    },
    {
     "by": 2,
     "from": 503,
     "to": 506
    }
   ],
   "kind": "changed",
   "old": "Order three capacitive sensors and test them for two sprints.",
   "revert": [
    {
     "from": 428,
     "insert": "Order three capacitive sensors and test them for two sprints.",
     "to": 507
    }
   ],
   "to": 507,
   "wordsAdded": 6,
   "wordsRemoved": 2
  }
 ],
 "sources": [
  {
   "at": "2026-11-20T14:33:48.787Z",
   "by": "human",
   "prompt": null,
   "version": "1.0",
   "who": "You"
  },
  {
   "at": "2026-11-20T14:33:48.790Z",
   "by": "ai",
   "prompt": null,
   "version": "1.1",
   "who": "Claude Code"
  },
  {
   "at": "2026-11-20T14:33:48.789Z",
   "by": "human",
   "prompt": null,
   "version": null,
   "who": "You"
  }
 ]
};
