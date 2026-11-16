// Example data for the v1 preview screens, from the projects in the requirements: a study project (Thrym Player), a
// company project (Bakery planning tool) and a hobby project (Synth build). Every value is checked against the types in
// $lib/types with `satisfies`, so a fixture that no longer matches the contract fails `npm run check`.

import type { DailyNote, DeliverableMeta, FeedbackItem, LabelConfirmation, PostMeta, ProjectMeta } from '$lib/types';

export const projectMeta = [
	{
		slug: 'thrym-player',
		kind: 'study',
		goal: 'A music player that shows which interaction patterns make offline listening easy, tested with five students.',
		end: 'semester',
		semester: 'NJ26',
		intendedMmm: 'handyman',
		mainQuestion: 'How can a music player make offline listening as easy as streaming?',
		client: null,
		tag: 'thrym-player'
	},
	{
		slug: 'bakery-planning-tool',
		kind: 'company',
		goal: 'Give the bakery a rota tool their staff can use on a phone, and the owner a week view.',
		end: '2027-01-29',
		semester: null,
		intendedMmm: 'cyborg',
		mainQuestion: null,
		client: 'Bakkerij De Korenaar',
		tag: 'bakery-planning-tool'
	},
	{
		slug: 'synth-build',
		kind: 'hobby',
		goal: 'Build a four-voice analogue synth from a kit and document every step.',
		end: null,
		semester: null,
		intendedMmm: 'man-made',
		mainQuestion: null,
		client: null,
		tag: 'synth-build'
	}
] satisfies ProjectMeta[];

export const daily = [
	{
		date: '2026-11-04',
		id: 'daily-2026-11-04',
		updatedAt: '2026-11-04T17:30',
		meetings: [
			{
				time: '10:00',
				title: 'Coach meeting',
				projects: ['thrym-player', 'bakery-planning-tool'],
				people: ['coach/frank'],
				body: `- Thrym Player: the offline sync idea is too big for this semester. **Decision:** drop sync between devices.
- Bakery: the client wants the week view first.
- [ ] Rewrite the project plan's scope section`
			},
			{
				time: '14:00',
				title: 'Kickoff at the bakery',
				projects: ['bakery-planning-tool'],
				people: ['client/korenaar/marieke', 'client/korenaar/joost'],
				body: `Staff swap shifts over WhatsApp today; nobody knows the latest rota.
- First version: a rota per week, readable on a phone.
- [ ] Draft the requirements
- Who may change the rota: only the owner?`
			}
		]
	},
	{
		date: '2026-10-28',
		id: 'daily-2026-10-28',
		updatedAt: '2026-10-28T16:10',
		meetings: [
			{
				time: '15:00',
				title: 'Interview with Sander',
				projects: ['thrym-player'],
				people: ['expert/sander'],
				body: `On Teams, about offline downloads.
- Downloads expire after 30 days offline: show the expiry in the library.
- [ ] Research post on download expiry
- What happens to a playlist when one song expires?`
			}
		]
	}
] satisfies DailyNote[];

export const feedback = [
	{
		id: 'f-1',
		kind: 'someone',
		source: 'portflow',
		scope: 'personal',
		from: 'coach/frank',
		role: 'coach',
		given: 'portflow',
		daily: null,
		target: { on: 'deliverable', project: 'thrym-player', deliverable: '01 Project plan.md', version: '1.0' },
		body: 'Your main question is clear, but the sub-questions overlap. Make SQ2 about the interaction, not the technology.',
		at: '2026-11-06T09:12',
		status: 'open',
		resolvedAt: null,
		followUps: []
	},
	{
		id: 'f-2',
		kind: 'someone',
		source: 'ledger',
		scope: 'personal',
		from: 'expert/sander',
		role: 'expert',
		given: 'in an interview',
		daily: '2026-10-28',
		target: { on: 'deliverable', project: 'thrym-player', deliverable: '02 Analysis.md', version: null },
		body: 'Explain why phase 2 cannot run in parallel; a reader will ask.',
		at: '2026-10-28T15:40',
		status: 'open',
		resolvedAt: null,
		followUps: ['Post “Why phase 2 waits” in Interaction design']
	},
	{
		id: 'f-3',
		kind: 'someone',
		source: 'ledger',
		scope: 'personal',
		from: 'client/korenaar/marieke',
		role: 'client',
		given: 'by mail',
		daily: null,
		target: { on: 'deliverable', project: 'bakery-planning-tool', deliverable: '01 Requirements.md', version: '1.0' },
		body: 'The week view must show who is on holiday. Otherwise this is what we need.',
		at: '2026-11-07T11:05',
		status: 'open',
		resolvedAt: null,
		followUps: []
	},
	{
		id: 'f-4',
		kind: 'someone',
		source: 'feedpulse',
		scope: 'group',
		from: 'teammate/lisa',
		role: 'teammate',
		given: 'feedpulse',
		daily: null,
		target: { on: 'project', project: 'thrym-player' },
		body: 'Good sprint review. Next time, show the test results before the demo.',
		at: '2026-10-31T16:00',
		status: 'resolved',
		resolvedAt: '2026-11-02T10:00',
		followUps: ['Reflection post, sprint 2']
	},
	{
		id: 'f-5',
		kind: 'own',
		source: 'ledger',
		scope: 'personal',
		from: null,
		role: null,
		given: null,
		daily: null,
		target: { on: 'deliverable', project: 'synth-build', deliverable: '01 Build log.md', version: null },
		body: 'Note for myself: add photos of the VCO calibration before the next version.',
		at: '2026-11-08T21:30',
		status: 'open',
		resolvedAt: null,
		followUps: []
	}
] satisfies FeedbackItem[];

// Posts and deliverables are the vault's own, so there is nothing to show for them until you set something.
export const postMeta = [] satisfies PostMeta[];
export const deliverableMeta = [] satisfies DeliverableMeta[];
export const confirmations = [] satisfies LabelConfirmation[];
