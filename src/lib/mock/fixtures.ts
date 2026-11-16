// Example data for the v1 preview screens, from the projects in the requirements: a study project (Thrym Player), a
// company project (Bakery planning tool) and a hobby project (Synth build). Every value is checked against the types in
// $lib/types with `satisfies`, so a fixture that no longer matches the contract fails `npm run check`.

import type { DailyNote, DeliverableMeta, LabelConfirmation, PostMeta, ProjectMeta } from '$lib/types';

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

// Posts and deliverables are the vault's own, so there is nothing to show for them until you set something.
export const postMeta = [] satisfies PostMeta[];
export const deliverableMeta = [] satisfies DeliverableMeta[];
export const confirmations = [] satisfies LabelConfirmation[];
