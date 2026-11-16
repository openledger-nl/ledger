<script lang="ts">
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import Dropdown, { type DropdownOption } from '$lib/components/Dropdown.svelte';
	import Switch from '$lib/components/Switch.svelte';
	import { trelloApi as api } from '$lib/api';
	import { ago, dateTime } from '$lib/format';
	import { attempt, confirmDialog, toast } from '$lib/ui.svelte';
	import type { TrelloBoard, TrelloMapping, TrelloMember, TrelloStatus, TrelloView } from '$lib/types';

	const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

	let view = $state<TrelloView | null>(null);
	/** The newest status the backend announced; until one arrives, the one that came with the view. */
	let live = $state<TrelloStatus | null>(null);
	let loadError = $state<string | null>(null);
	const status = $derived(live ?? view?.status ?? null);

	let apiKey = $state('');
	let keyInput = $state<HTMLInputElement>();
	let token = $state('');
	let busy = $state<string | null>(null);
	let result = $state<{ ok: boolean; message: string } | null>(null);

	/** The account's boards, loaded once there is a key and token. */
	let boards = $state<TrelloBoard[] | null>(null);
	let boardsError = $state<string | null>(null);

	let changingPerson = $state(false);
	let pickBoard = $state('');
	let people = $state<TrelloMember[]>([]);
	let pickPerson = $state('');

	const boardOptions = $derived<DropdownOption[]>((boards ?? []).map((b) => ({ value: b.id, label: b.name })));
	const personOptions = $derived<DropdownOption[]>([
		{ value: '', label: pickBoard ? 'Choose a person…' : 'Choose a board first' },
		...people.map((m) => ({ value: m.id, label: `${m.fullName || m.username}${m.fullName && m.username ? ` (@${m.username})` : ''}` }))
	]);

	/** Per linked project: its columns and the lists they use. */
	let mappings = $state<Record<string, TrelloMapping>>({});

	// ---- loading -------------------------------------------------------------------------------

	async function load() {
		try {
			view = await api.trelloGet();
			loadError = null;
		} catch (e) {
			loadError = message(e);
			return;
		}
		if (view.hasKey && view.hasToken && boards === null) await loadBoards();
		await loadMappings();
	}

	async function loadBoards() {
		boardsError = null;
		try {
			boards = await api.trelloBoards();
		} catch (e) {
			boards = null;
			boardsError = message(e);
		}
	}

	async function loadMapping(slug: string) {
		try {
			mappings[slug] = await api.trelloProjectMapping(slug);
		} catch {
			delete mappings[slug];
		}
	}

	async function loadMappings() {
		if (!view) return;
		const linked = view.projects.filter((p) => p.board);
		for (const slug of Object.keys(mappings)) {
			if (!linked.some((p) => p.slug === slug)) delete mappings[slug];
		}
		for (const p of linked) await loadMapping(p.slug);
	}

	onMount(() => {
		void load();
		let alive = true;
		let unlisten: (() => void) | undefined;
		let lastSeen: string | null = null;
		listen<TrelloStatus>('trello-status', (event) => {
			live = event.payload;
			// A round just finished: what it set aside or detached changes what is listed here.
			if (event.payload.lastSyncAt !== lastSeen && event.payload.state !== 'syncing') {
				lastSeen = event.payload.lastSyncAt;
				void load();
			}
		})
			.then((fn) => (alive ? (unlisten = fn) : fn()))
			.catch(() => {});
		return () => {
			alive = false;
			unlisten?.();
		};
	});

	// ---- the key and token ---------------------------------------------------------------------

	async function test() {
		busy = 'test';
		result = null;
		try {
			result = { ok: true, message: await api.trelloTest(apiKey.trim() || null, token.trim() || null) };
		} catch (e) {
			result = { ok: false, message: message(e) };
		}
		busy = null;
	}

	async function saveCredentials() {
		busy = 'save';
		const saved = await attempt(() => api.trelloSaveCredentials(apiKey.trim() || null, token.trim() || null));
		busy = null;
		if (!saved) return;
		view = saved;
		apiKey = '';
		token = '';
		result = null;
		toast('Trello key and token saved');
		await loadBoards();
	}

	async function disconnect() {
		const ok = await confirmDialog({
			title: 'Remove the Trello key and token?',
			message:
				'The key and token are removed from this machine and syncing stops. Your tasks and stories stay as they are, here and on Trello; nothing is deleted on Trello.',
			confirmLabel: 'Remove',
			danger: true
		});
		if (!ok) return;
		const saved = await attempt(() => api.trelloDisconnect());
		if (!saved) return;
		view = saved;
		boards = null;
		result = null;
	}

	/** Off removes the key and token (after asking); on points at the fields to fill in. */
	function toggleTrello(on: boolean) {
		if (!on) return void disconnect();
		toast('Fill in the API key and token below, then save.');
		keyInput?.focus();
	}

	// ---- the person ----------------------------------------------------------------------------

	async function chooseBoard(id: string) {
		pickBoard = id;
		pickPerson = '';
		people = [];
		if (!id) return;
		const members = await attempt(() => api.trelloBoardMembers(id));
		if (members) people = members;
	}

	function startChanging() {
		changingPerson = true;
		pickBoard = '';
		pickPerson = '';
		people = [];
	}

	async function savePerson() {
		if (view?.member && view.member.id !== pickPerson) {
			const ok = await confirmDialog({
				title: 'Sync for someone else?',
				message: `Items already synced for ${view.member.fullName || view.member.username} are not assigned to the new person on Trello, so they stop syncing (they stay here, marked as no longer linked). Nothing is deleted.`,
				confirmLabel: 'Change person',
				danger: true
			});
			if (!ok) return;
		}
		busy = 'person';
		const saved = await attempt(() => api.trelloSaveMember(pickBoard, pickPerson));
		busy = null;
		if (!saved) return;
		view = saved;
		changingPerson = false;
		toast('Syncing for ' + (saved.member?.fullName || saved.member?.username || 'that person'));
		await loadMappings();
	}

	// ---- projects and columns ------------------------------------------------------------------

	// The dropdown shows the board the project has, so a cancelled or failed change needs no undoing here.
	async function linkProject(slug: string, board: string) {
		const project = view?.projects.find((p) => p.slug === slug);
		const before = project?.board ?? '';
		if (before && before !== board) {
			const ok = await confirmDialog({
				title: board ? 'Use another board?' : 'Stop syncing this project?',
				message:
					'Items already synced with the old board stop syncing: their cards aren’t on the new board. They stay here, marked as no longer linked, and nothing is deleted on Trello.',
				confirmLabel: board ? 'Use this board' : 'Stop syncing',
				danger: true
			});
			if (!ok) return;
		}
		busy = 'link:' + slug;
		const saved = await attempt(() => api.trelloLinkProject(slug, board || null));
		busy = null;
		if (!saved) return;
		view = saved;
		await loadMappings();
	}

	async function chooseList(slug: string, column: string, choice: string) {
		if ((await attempt(() => api.trelloSetColumnList(slug, column, choice))) !== undefined) await loadMapping(slug);
	}

	// ---- syncing -------------------------------------------------------------------------------

	async function syncNow() {
		await attempt(() => api.trelloSyncNow());
	}

	async function relink(kind: 'task' | 'story', project: string, id: string) {
		const ok = await confirmDialog({
			title: 'Give it a new card?',
			message:
				'The old card is left alone on Trello. The next sync makes a new card for this item, assigned to the person, and syncs it from then on.',
			confirmLabel: 'Make a new card'
		});
		if (!ok) return;
		const saved = await attempt(() => api.trelloRelinkItem(project, kind, id));
		if (saved) view = saved;
	}

	const stateText = $derived.by(() => {
		if (!status?.configured) return 'Not syncing';
		if (status.state === 'syncing') return 'Syncing…';
		if (status.state === 'error') return 'Last sync failed';
		return status.pending ? 'Changes waiting to be looked at' : 'Up to date';
	});

	const ready = $derived(!!view && view.hasKey && view.hasToken);
</script>

<div class="panel">
	<div class="panel-head">
		<Switch checked={ready} label="Sync with Trello" disabled={!view || busy !== null} onchange={toggleTrello} />
		<h2>Sync with Trello</h2>
	</div>
	<p class="sub" style="margin:0">
		Keep tasks and user stories in step with the Trello cards of <strong>one person</strong>. Cards that person is a member of come in as tasks (or as
		user stories, when they carry the “User story” label), and a task or story made here becomes a card on Trello with that person assigned. Cards of
		anyone else are never read or changed. Only projects you link to a board are synced.
	</p>

	{#if loadError}
		<div class="notice bad">{loadError}</div>
	{/if}

	{#if view}
		<!-- 1. the key and token -->
		<div class="form-row">
			<label class="field grow">
				API key
				<input
					bind:this={keyInput}
					type="password"
					bind:value={apiKey}
					placeholder={view.hasKey ? `Saved ${view.keyHint} — leave blank to keep it` : 'Your Trello API key'}
					spellcheck="false"
					autocomplete="off"
				/>
			</label>
			<label class="field grow">
				Token
				<input
					type="password"
					bind:value={token}
					placeholder={view.hasToken ? `Saved ${view.tokenHint} — leave blank to keep it` : 'Your Trello token'}
					spellcheck="false"
					autocomplete="off"
				/>
			</label>
		</div>
		<div class="hint">
			Get both from
			<a
				href="https://trello.com/power-ups/admin"
				onclick={(e) => {
					e.preventDefault();
					openUrl(e.currentTarget.href).catch(() => {});
				}}>Trello’s Power-Ups admin</a
			>: create a Power-Up (any name), open its API key page and copy the key, then choose “Generate a Token” there and allow read and write access. Ledger
			sends them in an <code>Authorization</code> header, never in a web address, and keeps them in this vault’s <code>.ledger/.env</code>: on this machine
			only, never uploaded, and never shown here again beyond their last four characters.
		</div>

		{#if result}
			<div class="notice" class:ok={result.ok} class:bad={!result.ok}>{result.message}</div>
		{/if}

		<div class="row-end" style="justify-content:flex-start">
			<button class="btn" onclick={test} disabled={busy !== null || ((!apiKey.trim() && !view.hasKey) || (!token.trim() && !view.hasToken))}>
				Test connection
			</button>
			<button class="btn primary" onclick={saveCredentials} disabled={busy !== null || (!apiKey.trim() && !token.trim()) || (!view.hasKey && !apiKey.trim()) || (!view.hasToken && !token.trim())}>
				Save key and token
			</button>
			{#if view.hasKey || view.hasToken}
				<button class="btn ghost danger" onclick={disconnect} disabled={busy !== null}>Remove key and token</button>
			{/if}
		</div>

		{#if ready}
			<!-- 2. the person -->
			<div class="sync-detail">
				<h3 style="font-size:14px">Whose cards</h3>
				{#if view.member && !changingPerson}
					<div class="kv">
						<dt>Person</dt>
						<dd>
							{view.member.fullName || view.member.username}
							{#if view.member.fullName && view.member.username}<span class="hint">@{view.member.username}</span>{/if}
						</dd>
					</div>
					<div class="row-end" style="justify-content:flex-start">
						<button class="btn small" onclick={startChanging} disabled={busy !== null}>Choose someone else…</button>
					</div>
				{:else}
					<div class="form-row">
						<div class="field grow">
							Board to pick from
							<Dropdown
								bind:value={() => pickBoard, (v) => v !== pickBoard && chooseBoard(v)}
								options={[{ value: '', label: boards === null ? 'Loading boards…' : 'Choose a board…' }, ...boardOptions]}
								disabled={boards === null}
								label="Board to pick from"
								full
							/>
						</div>
						<div class="field grow">
							Person
							<Dropdown bind:value={pickPerson} options={personOptions} disabled={people.length === 0} label="Person" full />
						</div>
					</div>
					{#if boardsError}
						<div class="notice bad">{boardsError}</div>
					{/if}
					<div class="row-end" style="justify-content:flex-start">
						<button class="btn primary" onclick={savePerson} disabled={busy !== null || !pickPerson}>Sync for this person</button>
						{#if view.member}
							<button class="btn ghost" onclick={() => (changingPerson = false)}>Cancel</button>
						{/if}
						{#if boardsError}
							<button class="btn" onclick={loadBoards}>Try again</button>
						{/if}
					</div>
					<div class="hint">
						One person only: pick them from the members of any board you share. Their cards, on the boards linked below, are the only ones Ledger reads or
						touches.
					</div>
				{/if}
			</div>

			<!-- 3. projects, boards and columns -->
			{#if view.member}
				<div class="sync-detail">
					<h3 style="font-size:14px">Projects</h3>
					{#if view.projects.length === 0}
						<div class="hint">There are no projects yet.</div>
					{/if}
					{#each view.projects as p (p.slug)}
						<div class="field">
							{p.name}
							<Dropdown
								bind:value={() => p.board ?? '', (v) => v !== (p.board ?? '') && linkProject(p.slug, v)}
								options={[
									{ value: '', label: 'Not synced' },
									...boardOptions,
									...(p.board && !(boards ?? []).some((b) => b.id === p.board) ? [{ value: p.board, label: p.boardName ?? p.board }] : [])
								]}
								disabled={busy !== null || boards === null}
								label="Trello board for {p.name}"
								full
							/>
						</div>
						{#if p.board && mappings[p.slug]}
							{@const mapping = mappings[p.slug]}
							<div class="kv" style="grid-template-columns: minmax(90px, 160px) 1fr; padding-left:12px">
								{#each mapping.columns as c (c.id)}
									<dt>{c.name}</dt>
									<dd>
										<Dropdown
											bind:value={() => c.choice, (v) => v !== c.choice && chooseList(p.slug, c.id, v)}
											options={[
												{ value: 'auto', label: c.choice === 'auto' && c.listName ? `List “${c.listName}” (same name)` : 'The list with the same name' },
												{ value: 'none', label: 'Don’t sync this column' },
												...mapping.lists.map((l) => ({ value: l.id, label: `List “${l.name}”` }))
											]}
											disabled={busy !== null}
											label="Trello list for {c.name}"
										/>
										{#if !c.listId}<span class="hint">no list: its tasks aren’t synced</span>{/if}
									</dd>
								{/each}
							</div>
							{#if mapping.unusedLists.length > 0}
								<div class="hint" style="padding-left:12px">
									Cards in {mapping.unusedLists.map((n) => `“${n}”`).join(', ')} aren’t added, because no column uses that list.
								</div>
							{/if}
						{/if}
					{/each}
					<div class="hint">
						A column takes the Trello list with the same name (ignoring capitals) unless you choose another. A task whose column has no list isn’t put on
						Trello, and a card whose list has no column isn’t added here; both are counted below. A project’s user stories go in the list of its backlog
						column when they are made. One board serves one project.
					</div>
				</div>
			{/if}

			<!-- 4. status -->
			<div class="sync-detail">
				<div class="kv">
					<dt>Status</dt>
					<dd>
						<span class="dot" class:ok={status?.state === 'idle' && !status.pending} class:warn={status?.state === 'syncing' || !!status?.pending} class:bad={status?.state === 'error'}></span>
						{stateText}
					</dd>
					{#if status?.lastSyncAt}
						<dt>Last sync</dt>
						<dd title={dateTime(status.lastSyncAt)}>{ago(status.lastSyncAt)}{status.summary ? ` — ${status.summary}` : ''}</dd>
					{/if}
					{#if status?.report}
						<dt>Counts</dt>
						<dd>
							cards created {status.report.cardsCreated}, updated {status.report.cardsUpdated} · items added {status.report.itemsCreated}, updated
							{status.report.itemsUpdated}{#if status.report.conflicts > 0}
								· conflicts {status.report.conflicts}{/if}{#if status.report.unmatched > 0}
								· left out {status.report.unmatched}{/if}
						</dd>
					{/if}
					{#if status?.lastError}
						<dt>Problem</dt>
						<dd class="error-text">{status.lastError}</dd>
					{/if}
					<dt>Settings</dt>
					<dd class="mono">{view.settingsFile}</dd>
					<dt>Secrets</dt>
					<dd class="mono">{view.secretsFile}</dd>
				</div>
				{#if status?.report?.notes.length}
					<div class="notice">
						{#each status.report.notes as note, i (i)}<div>{note}</div>{/each}
					</div>
				{/if}
				{#if view.configured}
					<div class="row-end" style="justify-content:flex-start">
						<button class="btn small" onclick={syncNow} disabled={status?.state === 'syncing'}>Sync now</button>
					</div>
				{:else}
					<div class="hint">Syncing starts once there is a key and token, a person, and at least one project linked to a board.</div>
				{/if}
			</div>

			{#if view.unlinked.length > 0}
				<div class="sync-detail">
					<h3 style="font-size:14px">No longer synced</h3>
					<div class="hint">
						These items stopped syncing because their card was archived, deleted, moved to another board, or is no longer the person’s. They are still here, untouched.
					</div>
					{#each view.unlinked as item (item.project + '/' + item.kind + '/' + item.id)}
						<div class="kv" style="grid-template-columns: 1fr auto">
							<div>
								<strong>{item.title}</strong>
								<span class="hint">· {item.kind === 'story' ? 'user story' : 'task'} in {item.projectName}</span>
								<div class="hint">{item.reason}</div>
							</div>
							<button class="btn small" onclick={() => relink(item.kind, item.project, item.id)}>Make a new card</button>
						</div>
					{/each}
				</div>
			{/if}

			<div class="hint">
				Nothing is ever deleted on Trello, and deleting or archiving on one side never deletes on the other: an item whose card is gone stays here and stops
				syncing, and a card whose item you delete stays on Trello. When both sides change the same text, the newer edit wins and the other is kept in the
				vault’s <code>.ledger/conflicts</code> folder. Ledger looks at Trello every few minutes and shortly after you make changes. Trello allows about 100
				requests per 10 seconds per token; Ledger stays well under that and waits when Trello asks it to.
			</div>
			<div class="hint">
				Every card Ledger writes ends with a short “Synced with Ledger” note. A card with that note is never added from Trello (it reaches your other machines
				through the vault), so an item you deleted stays deleted and two machines never add the same item twice. Take the note off a card’s description to
				bring it in anyway. A user story’s description on Trello reads <code>**As a** … / **I want to** … / **Such that** …</code>, then
				<code>**Acceptance criteria**</code> as a checklist, then <code>**Notes**</code>; you can edit it there.
			</div>
		{/if}
	{:else if !loadError}
		<div class="hint">Loading…</div>
	{/if}
</div>
