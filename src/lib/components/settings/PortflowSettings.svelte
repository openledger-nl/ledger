<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { listen } from '@tauri-apps/api/event';
	import Dropdown, { type DropdownOption } from '$lib/components/Dropdown.svelte';
	import Switch from '$lib/components/Switch.svelte';
	import { api, portflowApi } from '$lib/api';
	import { projectUrl } from '$lib/actions';
	import { ago, dateTime, plural } from '$lib/format';
	import { loadPortflow } from '$lib/portflow.svelte';
	import { attempt, confirmDialog, refresh, toast } from '$lib/ui.svelte';
	import type { PortflowCollectionChoice, PortflowStatus, PortflowView, ProjectSummary, Semester } from '$lib/types';

	const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

	let view = $state<PortflowView | null>(null);
	let live = $state<PortflowStatus | null>(null);
	let loadError = $state<string | null>(null);
	const status = $derived(live ?? view?.status ?? null);
	let busy = $state<string | null>(null);

	let projects = $state<ProjectSummary[]>([]);
	let semesters = $state<Semester[]>([]);
	let collections = $state<PortflowCollectionChoice[] | null>(null);
	let collectionsError = $state<string | null>(null);
	let pickProject = $state('');
	let pickSemester = $state('');
	let pickCollection = $state('');

	let importCollection = $state('');
	let importSemester = $state('');
	let importName = $state('');
	let importThreads = $state(true);

	let launchUrl = $state('');
	let editingUrl = $state(false);

	// Remember me: the Fontys login, saved so that Ledger signs in by itself (portflow_login.rs).
	let remembering = $state(false);
	let loginName = $state('');
	let loginPassword = $state('');

	async function load() {
		try {
			view = await portflowApi.get();
			loadError = null;
		} catch (e) {
			loadError = message(e);
			return;
		}
		if (view.enabled) {
			projects = (await attempt(() => api.listProjects())) ?? [];
			semesters = (await attempt(() => api.listSemesters())) ?? [];
			if (!pickSemester) pickSemester = currentSemester()?.id ?? semesters[0]?.id ?? '';
			if (!importSemester) importSemester = pickSemester;
			if (view.status.signedIn && collections === null) await loadCollections();
		}
	}

	function currentSemester(): Semester | undefined {
		const today = new Date().toISOString().slice(0, 10);
		return semesters.find((s) => s.start <= today && today <= s.end);
	}

	async function loadCollections() {
		collectionsError = null;
		try {
			collections = await portflowApi.collections();
		} catch (e) {
			collections = null;
			collectionsError = message(e);
		}
	}

	onMount(() => {
		void load();
		let alive = true;
		let unlisten: (() => void) | undefined;
		let lastSeen: string | null = null;
		listen<PortflowStatus>('portflow-status', (event) => {
			live = event.payload;
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

	// ---- the switch ----------------------------------------------------------------------------

	async function toggle(on: boolean) {
		if (!on) {
			const ok = await confirmDialog({
				title: 'Switch Portflow off?',
				message:
					'Ledger stops syncing, forgets the session and hides everything Portflow: outcomes, evidence and the people to tag. The Portflow files in your vault stay as they are, and feedback keeps who gave it. Switch it on again to bring it all back.',
				confirmLabel: 'Switch off'
			});
			if (!ok) return;
		}
		busy = 'switch';
		const saved = await attempt(() => portflowApi.setEnabled(on));
		busy = null;
		if (!saved) return;
		view = saved;
		live = null;
		collections = null;
		await loadPortflow();
		await refresh();
		if (on) await load();
	}

	// ---- signing in ----------------------------------------------------------------------------

	async function signIn() {
		busy = 'sign-in';
		toast('Sign in with your Fontys account in the window that opens.');
		const saved = await attempt(() => portflowApi.signIn());
		busy = null;
		if (!saved) return;
		view = saved;
		live = null;
		toast(`Signed in to Portflow as ${saved.portfolio?.userName ?? 'you'}`);
		await loadCollections();
	}

	async function signOut(forget: boolean) {
		if (forget) {
			const ok = await confirmDialog({
				title: 'Sign out and forget?',
				message:
					'Ledger forgets the session and removes the sign-in window\'s cookies, so the next sign-in starts with a fresh Fontys login. Nothing in your vault or in Portflow changes.',
				confirmLabel: 'Sign out and forget'
			});
			if (!ok) return;
		}
		const saved = await attempt(() => portflowApi.signOut(forget));
		if (!saved) return;
		view = saved;
		live = null;
		collections = null;
	}

	// ---- remember me ---------------------------------------------------------------------------
	// The password goes to the system keychain and nowhere else; this screen keeps it only while it is being typed.

	async function setRemember(on: boolean) {
		if (on) {
			remembering = true;
			return;
		}
		remembering = false;
		loginPassword = '';
		if (!view?.savedLogin) return;
		busy = 'login';
		const saved = await attempt(() => portflowApi.forgetLogin());
		busy = null;
		if (!saved) return;
		view = saved;
		toast('Ledger forgot your Fontys password');
	}

	async function saveLogin() {
		busy = 'login';
		const saved = await attempt(() => portflowApi.saveLogin(loginName, loginPassword));
		busy = null;
		if (!saved) return;
		view = saved;
		remembering = false;
		loginPassword = '';
		toast('Remembered: Ledger signs in to Fontys by itself from now on');
	}

	// ---- access -------------------------------------------------------------------------------
	// Read only, or read and write with the kinds of write switched on one by one. Going to read and write asks first:
	// from then on Ledger can change the real portfolio.

	async function setAccess(write: boolean, writes: string[]) {
		if (!view) return;
		if (write && !view.write) {
			const ok = await confirmDialog({
				title: 'Let Ledger write to Portflow?',
				message: 'Ledger will then make the changes you switch on below in your real portfolio. Each kind of write has its own switch, and all start off.',
				confirmLabel: 'Allow writing',
				// Red: from here Ledger can change the real portfolio.
				danger: true
			});
			if (!ok) return;
		}
		busy = 'access';
		const saved = await attempt(() => portflowApi.setAccess(write, writes));
		busy = null;
		if (saved) {
			view = saved;
			// The evidence editors say whether a description goes to Portflow.
			await loadPortflow();
		}
	}

	const onWrites = () => (view?.writes ?? []).filter((w) => w.on).map((w) => w.id);
	const toggleWrite = (id: string, on: boolean) => setAccess(true, on ? [...onWrites(), id] : onWrites().filter((w) => w !== id));

	async function saveUrl() {
		const saved = await attempt(() => portflowApi.setLaunchUrl(launchUrl));
		if (!saved) return;
		view = saved;
		editingUrl = false;
		toast('Launch address saved');
	}

	// ---- collections ---------------------------------------------------------------------------

	const pickable = $derived((collections ?? []).filter((c) => !c.linkedProject));
	const bySection = $derived.by(() => {
		const groups: { section: string; items: PortflowCollectionChoice[] }[] = [];
		for (const c of pickable) {
			const group = groups.find((g) => g.section === c.section);
			if (group) group.items.push(c);
			else groups.push({ section: c.section, items: [c] });
		}
		return groups;
	});

	async function link() {
		const existing = view?.links.find((l) => l.project === pickProject && l.semester === pickSemester);
		if (existing) {
			const ok = await confirmDialog({
				title: 'Use another collection?',
				message: `${existing.projectName} is already linked to "${existing.collectionName ?? existing.collection}" for this semester. One project has one collection per semester; the old link is replaced. Its evidence items stay in your vault.`,
				confirmLabel: 'Replace'
			});
			if (!ok) return;
		}
		busy = 'link';
		const saved = await attempt(() => portflowApi.linkCollection(pickProject, pickSemester, Number(pickCollection)));
		busy = null;
		if (!saved) return;
		view = saved;
		pickCollection = '';
		toast('Linked. The next sync brings in its outcomes and evidence.');
		await loadCollections();
	}

	const importing = $derived(pickable.find((c) => String(c.id) === importCollection) ?? null);

	function chooseImport(id: string) {
		importCollection = id;
		importName = pickable.find((c) => String(c.id) === id)?.name ?? '';
	}

	const projectOptions = $derived<DropdownOption[]>([{ value: '', label: 'Choose a project' }, ...projects.map((p) => ({ value: p.slug, label: p.name }))]);
	const semesterOptions = $derived<DropdownOption[]>(semesters.map((s) => ({ value: s.id, label: s.name })));

	/** The collections not in Ledger yet, under their section, after a first "none" line saying `empty`. */
	function collectionOptions(empty: string): DropdownOption[] {
		// Until the collections have loaded there is nothing to choose.
		if (collections === null) return [{ value: '', label: 'Loading collections...', disabled: true }];
		return [
			{ value: '', label: empty },
			...bySection.flatMap((g) =>
				g.items.map((c) => ({
					value: String(c.id),
					label: `${c.name}${c.subtitle ? ` | ${c.subtitle}` : ''} (${plural(c.evidenceCount, 'item')})`,
					group: g.section || 'No section'
				}))
			)
		];
	}

	async function importProject() {
		if (!importing) return;
		busy = 'import';
		const done = await attempt(() => portflowApi.importCollection(importing.id, importSemester, importName.trim() || null, importThreads));
		busy = null;
		if (!done) return;
		toast(`Imported ${done.projectName}: ${plural(done.evidence, 'evidence item')}. The sync is bringing in its versions and feedback.`);
		importCollection = '';
		importName = '';
		await refresh();
		await goto(projectUrl(done.project));
	}

	async function unlink(project: string, semester: string, name: string) {
		const ok = await confirmDialog({
			title: 'Unlink this collection?',
			message: `${name} is no longer synced for this semester. Its evidence items, their versions and their feedback stay in your vault; nothing changes in Portflow.`,
			confirmLabel: 'Unlink'
		});
		if (!ok) return;
		const saved = await attempt(() => portflowApi.linkCollection(project, semester, null));
		if (!saved) return;
		view = saved;
		await loadCollections();
	}

	async function syncNow() {
		await attempt(() => portflowApi.syncNow());
	}

	const stateText = $derived.by(() => {
		switch (status?.state) {
			case 'syncing':
				return 'Syncing...';
			case 'error':
				return 'Last sync failed';
			case 'signed-out':
				return 'Signed out: sign in to sync';
			case 'idle':
				return 'Up to date';
			default:
				return 'Off';
		}
	});
</script>

<div class="panel">
	<div class="panel-head">
		<Switch checked={!!view?.enabled} label="Portflow" disabled={!view || busy !== null} onchange={toggle} />
		<h2>Portflow</h2>
	</div>
	<p class="sub" style="margin:0">
		Bring your Fontys portfolio into Ledger: the learning outcomes, the feedback your coaches and experts give in Portflow, and the versions you hand in. A
		project is linked to one collection per semester; each evidence item in it gets a place in one of your threads, where its feedback lands.
	</p>

	{#if loadError}
		<div class="notice bad">{loadError}</div>
	{/if}

	{#if view && !view.enabled}
		<div class="hint">
			Off. Ledger opens no window and sends nothing to Portflow or Canvas, and it doesn't read or change the Portflow files in your vault. This is kept per
			machine.
		</div>
	{:else if view}
		<!-- 1. signing in -->
		<div class="sync-detail">
			<h3 style="font-size:14px">Sign in</h3>
			{#if status?.signedIn}
				<div class="kv">
					<dt>Signed in as</dt>
					<dd>{view.portfolio?.userName ?? 'you'}</dd>
					{#if status.expiresAt}
						<dt>Session</dt>
						<dd>until {dateTime(new Date(status.expiresAt).toISOString())}; renewed on its own while Canvas remembers you</dd>
					{/if}
				</div>
				<div class="row-end" style="justify-content:flex-start">
					<button class="btn small" onclick={() => signOut(false)} disabled={busy !== null}>Sign out</button>
					<button class="btn small ghost danger" onclick={() => signOut(true)} disabled={busy !== null}>Sign out and forget</button>
				</div>
			{:else}
				<div class="row-end" style="justify-content:flex-start">
					<button class="btn primary" onclick={signIn} disabled={busy !== null}>{busy === 'sign-in' ? 'Waiting for the sign-in...' : 'Sign in with your Fontys account'}</button>
					{#if view.portfolio}
						<button class="btn small ghost danger" onclick={() => signOut(true)} disabled={busy !== null}>Forget the sign-in</button>
					{/if}
				</div>
			{/if}
			<div class="remember">
				<Switch checked={view.savedLogin !== null || remembering} label="Remember me" disabled={busy !== null} onchange={setRemember} />
				<div class="remember-text">
					<strong>Remember me</strong>
					{#if view.savedLogin}
						<span class="hint">Signs in by itself as <span class="mono">{view.savedLogin}</span>, also when Canvas has forgotten you.</span>
					{:else}
						<span class="hint">Ledger types your Fontys login in for you, also when Canvas has forgotten you.</span>
					{/if}
				</div>
			</div>
			{#if remembering && !view.savedLogin}
				<form
					class="form-row"
					onsubmit={(e) => {
						e.preventDefault();
						void saveLogin();
					}}
				>
					<label class="field grow">
						Fontys username
						<input bind:value={loginName} type="email" autocomplete="off" spellcheck="false" placeholder="i123456@fontysict.nl" />
					</label>
					<label class="field grow">
						Password
						<input bind:value={loginPassword} type="password" autocomplete="off" />
					</label>
					<button class="btn small primary" type="submit" disabled={busy !== null || !loginName.trim() || !loginPassword}>Remember</button>
				</form>
			{/if}
			{#if remembering || view.savedLogin}
				<div class="hint">
					Your password is kept in the system keychain on Linux and macOS (the Credential Manager on Windows), not in your vault. Ledger types it only into the
					Fontys login page, and stops when Fontys asks for anything else.
				</div>
			{/if}
			<div class="hint">
				A Ledger window opens Portflow the way Canvas does; you sign in there as usual. Unless you choose Remember me, Ledger never sees or stores your password: once Portflow has opened,
				it takes only the session Portflow's own page uses (valid for eight hours) and closes the window. The session is kept in the system's keychain, like the
				Trello key: on this machine only, never in the vault. Signing out or switching Portflow off removes it.
			</div>
			{#if editingUrl}
				<div class="form-row">
					<label class="field grow">
						Launch address
						<input bind:value={launchUrl} placeholder={view.defaultLaunchUrl} spellcheck="false" />
					</label>
				</div>
				<div class="row-end" style="justify-content:flex-start">
					<button class="btn small primary" onclick={saveUrl}>Save</button>
					<button class="btn small ghost" onclick={() => (editingUrl = false)}>Cancel</button>
				</div>
				<div class="hint">The Canvas page that opens Portflow. Leave it blank for Fontys ICT's.</div>
			{:else}
				<div class="hint">
					Opens <span class="mono">{view.launchUrl}</span> |
					<button
						class="link accent"
						onclick={() => {
							launchUrl = view?.launchUrl === view?.defaultLaunchUrl ? '' : (view?.launchUrl ?? '');
							editingUrl = true;
						}}>change</button
					>
				</div>
			{/if}
		</div>

		<!-- 2. access -->
		<div class="sync-detail">
			<h3 style="font-size:14px">Access</h3>
			<label class="choice">
				<input type="radio" name="portflow-access" checked={!view.write} disabled={busy !== null} onchange={() => setAccess(false, onWrites())} />
				<span><strong>Read only</strong> | Ledger reads your portfolio and never changes anything in it.</span>
			</label>
			<label class="choice">
				<input type="radio" name="portflow-access" checked={view.write} disabled={!view.writesAvailable || busy !== null} onchange={() => setAccess(true, onWrites())} />
				<span><strong>Read and write</strong> | also make the changes you switch on, one kind at a time.</span>
			</label>
			<div class="writes">
				{#each view.writes as w (w.id)}
					<label class="choice small" class:off={!w.available} title={w.available ? '' : 'Not built yet: Ledger can\'t make this change in this version'}>
						<input
							type="checkbox"
							checked={w.on && w.available}
							disabled={!view.write || !w.available || busy !== null}
							onchange={(e) => toggleWrite(w.id, e.currentTarget.checked)}
						/>{w.label}{#if !w.available}<span class="hint"> | not built yet</span>{/if}
					</label>
				{/each}
			</div>
			{#if !view.writesAvailable}
				<div class="hint">
					Writing isn't in this version yet: uploading to your real portfolio comes once it has been tried out with your go-ahead. Until then Ledger only reads,
					and that is enforced in the app's core, not just here.
				</div>
			{/if}
		</div>

		<!-- 3. importing a project -->
		{#if status?.signedIn}
			<div class="sync-detail">
				<h3 style="font-size:14px">Import a project from Portflow</h3>
				{#if semesters.length === 0}
					<div class="notice">Add a semester first (Shared, Semesters): the project is linked to its collection for one semester.</div>
				{:else}
					<div class="form-row">
						<div class="field grow">
							Collection
							<!-- A different collection names the new project after it. -->
							<Dropdown
								bind:value={() => importCollection, (v) => v !== importCollection && chooseImport(v)}
								options={collectionOptions(pickable.length ? 'Choose what to import' : 'Every collection is in Ledger')}
								label="Collection to import"
								full
							/>
						</div>
						<div class="field">
							Semester
							<Dropdown bind:value={importSemester} options={semesterOptions} label="Semester" full />
						</div>
					</div>
					{#if importing}
						<div class="form-row">
							<label class="field grow">
								Project name
								<input bind:value={importName} placeholder={importing.name} />
							</label>
						</div>
						<label class="choice">
							<input type="checkbox" bind:checked={importThreads} />
							<span>A thread for each evidence item ({importing.evidenceCount}), where its feedback lands</span>
						</label>
					{/if}
					<div class="row-end" style="justify-content:flex-start">
						<button class="btn primary" onclick={importProject} disabled={busy !== null || !importing || !importSemester}>
							{busy === 'import' ? 'Importing...' : 'Import as a new project'}
						</button>
					</div>
					<div class="hint">
						Makes a new project in Ledger, linked to the collection for that semester. The next sync brings in its learning outcomes, its evidence with
						the list of its versions, and the feedback on it. Files you uploaded in Portflow are not copied: they open in Portflow. Nothing changes
						in Portflow. To link a collection to a project you already have,
						use the list below.
					</div>
				{/if}
			</div>
		{/if}

		<!-- 4. projects and collections -->
		<div class="sync-detail">
			<h3 style="font-size:14px">Projects and collections</h3>
			{#each view.links as l (l.project + '/' + l.semester)}
				<div class="kv" style="grid-template-columns: 1fr auto">
					<div>
						<strong>{l.projectName}</strong>
						<span class="hint">| {l.semesterName ?? l.semester}</span>
						<div class="hint">{l.collectionName ?? `Collection ${l.collection}: brought in at the next sync`}</div>
					</div>
					<button class="btn small ghost" onclick={() => unlink(l.project, l.semester, l.collectionName ?? 'The collection')}>Unlink</button>
				</div>
			{:else}
				<div class="hint">No project is linked yet.</div>
			{/each}

			{#if status?.signedIn}
				{#if semesters.length === 0}
					<div class="notice">Add a semester first (Shared, Semesters): a collection is linked for one semester.</div>
				{:else}
					<div class="form-row">
						<div class="field grow">
							Project
							<Dropdown bind:value={pickProject} options={projectOptions} label="Project" full />
						</div>
						<div class="field">
							Semester
							<Dropdown bind:value={pickSemester} options={semesterOptions} label="Semester" full />
						</div>
						<div class="field grow">
							Collection
							<Dropdown
								bind:value={pickCollection}
								options={collectionOptions(pickable.length ? 'Choose a collection' : 'Every collection is linked')}
								label="Collection"
								full
							/>
						</div>
					</div>
					{#if collectionsError}
						<div class="notice bad">{collectionsError}</div>
					{/if}
					<div class="row-end" style="justify-content:flex-start">
						<button class="btn primary" onclick={link} disabled={busy !== null || !pickProject || !pickSemester || !pickCollection}>Link</button>
						{#if collectionsError}<button class="btn" onclick={loadCollections}>Try again</button>{/if}
					</div>
				{/if}
			{/if}
			<div class="hint">
				One collection per project per semester (a collection usually belongs to one semester's section). A project that carries on gets the next semester's
				collection next to it; the old link stays readable.
				{#if view.unplaced > 0}
					<strong>{plural(view.unplaced, 'evidence item')} still {view.unplaced === 1 ? 'needs' : 'need'} a thread:</strong> open the project to place them.
				{/if}
			</div>
		</div>

		<!-- 5. status -->
		<div class="sync-detail">
			<div class="kv">
				<dt>Status</dt>
				<dd>
					<span class="dot" class:ok={status?.state === 'idle'} class:warn={status?.state === 'syncing' || status?.state === 'signed-out'} class:bad={status?.state === 'error'}></span>
					{stateText}
				</dd>
				{#if status?.lastSyncAt}
					<dt>Last sync</dt>
					<dd title={dateTime(status.lastSyncAt)}>{ago(status.lastSyncAt)}{status.summary ? ` - ${status.summary}` : ''}</dd>
				{/if}
				{#if status?.lastError}
					<dt>Problem</dt>
					<dd class="error-text">{status.lastError}</dd>
				{/if}
				<dt>In the vault</dt>
				<dd>{plural(view.outcomes, 'learning outcome')}, {plural(view.people, 'person', 'people')} with access</dd>
			</div>
			{#if status?.report?.notes.length}
				<div class="notice">
					{#each status.report.notes as note, i (i)}<div>{note}</div>{/each}
				</div>
			{/if}
			<div class="row-end" style="justify-content:flex-start">
				<button class="btn small" onclick={syncNow} disabled={!status?.signedIn || status?.state === 'syncing'}>Sync now</button>
			</div>
			<div class="hint">
				Ledger looks at Portflow every ten minutes while you are signed in, and right after you link a collection or place an evidence item. The outcomes and
				people are mirrored to <code>shared/portflow/</code> in your vault, each linked collection to <code>projects/.../portflow/</code>, and each evidence item to
				<code>projects/.../evidence/</code>; Portflow stays their owner, so edits to them are overwritten by the next sync. Version 3 in Portflow is v3.0 of
				the evidence's files: when you published v3.0 in Ledger it links to those files, and a version you uploaded in Portflow is not copied but opens
				there.
			</div>
		</div>
	{:else if !loadError}
		<div class="hint">Loading...</div>
	{/if}
</div>

<style>
	.remember {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		margin-top: 4px;
	}
	.remember-text {
		display: flex;
		flex-direction: column;
		gap: 1px;
		font-size: 13px;
	}
	.choice {
		display: flex;
		gap: 8px;
		align-items: flex-start;
		font-size: 13px;
		padding: 2px 0;
	}
	.choice.small {
		font-size: 12px;
		align-items: center;
	}
	/* A kind of write this version can’t make yet: listed, but greyed out. */
	.choice.off {
		opacity: 0.6;
	}
	.writes {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		gap: 2px 12px;
		padding-left: 24px;
	}
</style>
