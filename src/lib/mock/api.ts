// A stand-in for the v1 backend commands, so the preview screens work end to end. Each function has the signature the
// real command in $lib/api will get: swapping a screen to the backend is changing `mockApi.x` to `api.x`. What is
// written here lives in this machine's browser storage (starting from the fixtures), never in the vault; Settings ›
// Features › Preview screens can reset it.

import * as fixtures from './fixtures';
import type {
	DeliverableMeta,
	FeedbackDraft,
	FeedbackItem,
	LabelConfirmation,
	DailyNote,
	PostMeta,
	ProjectMeta
} from '$lib/types';

const KEY = 'ledger-preview-data';

interface Store {
	projects: ProjectMeta[];
	daily: DailyNote[];
	feedback: FeedbackItem[];
	posts: PostMeta[];
	deliverables: DeliverableMeta[];
	confirmations: LabelConfirmation[];
}

const seed = (): Store => structuredClone({
	projects: fixtures.projectMeta,
	daily: fixtures.daily,
	feedback: fixtures.feedback,
	posts: fixtures.postMeta,
	deliverables: fixtures.deliverableMeta,
	confirmations: fixtures.confirmations
});

function load(): Store {
	try {
		const saved = JSON.parse(localStorage.getItem(KEY) ?? 'null');
		if (saved && typeof saved === 'object') {
			// Data from before meetings moved into daily notes: the old meetings and notes are dropped, the rest kept.
			delete saved.meetings;
			if (!Array.isArray(saved.daily) || saved.daily.some((d: { meetings?: unknown }) => !Array.isArray(d?.meetings))) delete saved.daily;
			return { ...seed(), ...saved };
		}
	} catch {
		// Unreadable: start again from the fixtures.
	}
	return seed();
}

let store = load();

function save() {
	try {
		localStorage.setItem(KEY, JSON.stringify(store));
	} catch {
		// Kept for this session only.
	}
}

/** Answers like a command does: a little later, and a copy, so a screen can't change the store by accident. */
const answer = <T>(value: T): Promise<T> => new Promise((resolve) => setTimeout(() => resolve(structuredClone(value)), 60));

const id = (prefix: string) => `${prefix}-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;

/** `Thrym Player` → `thrym-player`, as project tags are written. */
export const tagOf = (name: string) =>
	name
		.toLowerCase()
		.normalize('NFKD')
		.replace(/[^\p{L}\p{N}]+/gu, '-')
		.replace(/^-|-$/g, '');

export const mockApi = {
	// ---- projects ---------------------------------------------------------------------------------
	getProjectMeta: (slug: string) => answer(store.projects.find((p) => p.slug === slug) ?? null),
	listProjectMeta: () => answer(store.projects),
	saveProjectMeta: (meta: ProjectMeta) => {
		store.projects = [...store.projects.filter((p) => p.slug !== meta.slug), meta];
		save();
		return answer(meta);
	},

	// ---- posts ------------------------------------------------------------------------------------
	listPostMeta: (postIds: string[]) => answer(store.posts.filter((p) => postIds.includes(p.postId))),
	setPostMeta: (meta: PostMeta) => {
		store.posts = [...store.posts.filter((p) => p.postId !== meta.postId), meta];
		save();
		return answer(meta);
	},

	// ---- daily notes -----------------------------------------------------------------------------
	listDailyNotes: (filter: { project?: string; person?: string } = {}) =>
		answer(
			store.daily
				.filter((d) => !filter.project || d.meetings.some((m) => m.projects.includes(filter.project ?? '')))
				.filter((d) => !filter.person || d.meetings.some((m) => m.people.includes(filter.person ?? '')))
				.sort((a, b) => b.date.localeCompare(a.date))
		),
	getDailyNote: (date: string) => answer(store.daily.find((d) => d.date === date) ?? null),
	/** Writes the note of its day: a day has one note, so saving another for the same day replaces it. */
	saveDailyNote: (note: Omit<DailyNote, 'updatedAt'>) => {
		const saved: DailyNote = { ...note, updatedAt: new Date().toISOString() };
		store.daily = [...store.daily.filter((d) => d.date !== note.date), saved];
		save();
		return answer(saved);
	},
	deleteDailyNote: (date: string) => {
		store.daily = store.daily.filter((d) => d.date !== date);
		save();
		return answer(null);
	},

	// ---- feedback ---------------------------------------------------------------------------------
	listFeedback: () => answer([...store.feedback].sort((a, b) => b.at.localeCompare(a.at))),
	recordFeedback: (draft: FeedbackDraft) => {
		const item: FeedbackItem = { ...draft, id: id('f'), source: 'ledger', scope: 'personal', status: 'open', resolvedAt: null, followUps: [] };
		store.feedback = [...store.feedback, item];
		save();
		return answer(item);
	},
	/** Only you resolve feedback: this is called from your click, never by Ledger itself. */
	setFeedbackStatus: (feedbackId: string, status: 'open' | 'resolved') => {
		store.feedback = store.feedback.map((f) =>
			f.id === feedbackId ? { ...f, status, resolvedAt: status === 'resolved' ? new Date().toISOString() : null } : f
		);
		save();
		return answer(store.feedback.find((f) => f.id === feedbackId) ?? null);
	},

	// ---- deliverables -----------------------------------------------------------------------------
	listDeliverableMeta: (project: string) => answer(store.deliverables.filter((d) => d.project === project)),
	getDeliverableMeta: (project: string, path: string) =>
		answer(store.deliverables.find((d) => d.project === project && d.path === path) ?? null),
	/** The number the next deliverable of a project gets: one past the highest ever given, never reused. */
	nextDeliverableNumber: (project: string, existing: string[]) => {
		const numbers = [
			...store.deliverables.filter((d) => d.project === project).map((d) => d.number),
			...existing.map((name) => Number(/^(\d{2,})\s/.exec(name.split('/').pop() ?? '')?.[1] ?? 0))
		];
		return answer(Math.max(0, ...numbers) + 1);
	},
	saveDeliverableMeta: (meta: DeliverableMeta) => {
		store.deliverables = [...store.deliverables.filter((d) => !(d.project === meta.project && d.path === meta.path)), meta];
		save();
		return answer(meta);
	},
	confirmLabel: (confirmation: LabelConfirmation) => {
		store.confirmations = [...store.confirmations, confirmation];
		save();
		return answer(confirmation);
	},

	/** Back to the fixtures: everything written in the preview screens is forgotten. */
	reset: () => {
		store = seed();
		save();
		return answer(null);
	}
};
