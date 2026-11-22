// People of the vault, as projects attach them: one person in every project they are part of (people.rs). Free of
// Svelte, like v1.ts.

import { api } from './api';
import type { FeedbackRole, Person, ProjectPerson } from './types';

/** The person of the vault with this name (not minding case or the spaces around it), if there is one. */
export const findByName = (people: Person[], name: string) => {
	const key = name.trim().toLowerCase();
	return key ? people.find((p) => p.name.trim().toLowerCase() === key) : undefined;
};

/** The person of the vault called `name`: the one there is, or a new one made now. */
export async function personNamed(people: Person[], name: string): Promise<Person> {
	return findByName(people, name) ?? api.createPerson({ name: name.trim(), email: '', organisation: '', portflowUser: null, notes: '' });
}

/** Who is part of a project, as it is saved: each person once, by their id, with their role in it. */
export type Attached = Pick<ProjectPerson, 'person' | 'role'>;

export const attachedOf = (list: ProjectPerson[]): Attached[] => list.map(({ person, role }) => ({ person, role }));

/** Adds someone to a project's list, or gives them this role when they are on it already. */
export function withPerson(list: Attached[], person: string, role: FeedbackRole): Attached[] {
	return list.some((p) => p.person === person) ? list.map((p) => (p.person === person ? { ...p, role } : p)) : [...list, { person, role }];
}
