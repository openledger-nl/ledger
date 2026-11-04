<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { listen } from '@tauri-apps/api/event';
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
					'Ledger forgets the session and removes the sign-in window’s cookies, so the next sign-in starts with a fresh Fontys login. Nothing in your vault or in Portflow changes.',
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
				message: `${existing.projectName} is already linked to “${existing.collectionName ?? existing.collection}” for this semester. One project has one collection per semester; the old link is replaced. Its evidence items stay in your vault.`,
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
				return 'Syncing…';
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
			Off. Ledger opens no window and sends nothing to Portflow or Canvas, and it doesn’t read or change the Portflow files in your vault. This is kept per
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
					<button class="btn primary" onclick={signIn} disabled={busy !== null}>{busy === 'sign-in' ? 'Waiting for the sign-in…' : 'Sign in with your Fontys account'}</button>
					{#if view.portfolio}
						<button class="btn small ghost danger" onclick={() => signOut(true)} disabled={busy !== null}>Forget the sign-in</button>
					{/if}
				</div>
			{/if}
			<div class="hint">
				A Ledger window opens Portflow the way Canvas does; you sign in there as usual. Ledger never sees or stores your password: once Portflow has opened,
				it takes only the session Portflow’s own page uses (valid for eight hours) and closes the window. The session is kept in this vault’s
				<code>.ledger/.env</code>, like the Trello key: on this machine only, never uploaded. Signing out or switching Portflow off removes it.
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
				<div class="hint">The Canvas page that opens Portflow. Leave it blank for Fontys ICT’s.</div>
			{:else}
				<div class="hint">
					Opens <span class="mono">{view.launchUrl}</span> ·
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
				<input type="radio" checked={!view.write} disabled />
				<span><strong>Read only</strong> · Ledger reads your portfolio and never changes anything in it.</span>
			</label>
			<label class="choice">
				<input type="radio" checked={view.write} disabled={!view.writesAvailable} />
				<span><strong>Read and write</strong> · also make the changes you switch on, one kind at a time.</span>
			</label>
			<div class="writes">
				{#each view.writes as w (w.id)}
					<label class="choice small"><input type="checkbox" checked={w.on} disabled={!view.writesAvailable} />{w.label}</label>
				{/each}
			</div>
			{#if !view.writesAvailable}
				<div class="hint">
					Writing isn’t in this version yet: uploading to your real portfolio comes once it has been tried out with your go-ahead. Until then Ledger only reads,
					and that is enforced in the app’s core, not just here.
				</div>
			{/if}
		</div>

		<!-- 3. importing a project -->
		{#if status?.signedIn}
			<div class="sync-detail">
				<h3 style="font-size:14px">Import a project from Portflow</h3>
				{#if semesters.length === 0}
					<div class="notice">Add a semester first (Settings → Semesters): the project is linked to its collection for one semester.</div>
				{:else}
					<div class="form-row">
						<label class="field grow">
							Collection
							<select
								bind:value={importCollection}
								disabled={collections === null}
								onchange={() => (importName = pickable.find((c) => String(c.id) === importCollection)?.name ?? '')}
							>
								<option value="">{collections === null ? 'Loading collections…' : pickable.length ? 'Choose what to import…' : 'Every collection is in Ledger'}</option>
								{#each bySection as g (g.section)}
									<optgroup label={g.section || 'No section'}>
										{#each g.items as c (c.id)}<option value={String(c.id)}>{c.name}{c.subtitle ? ` · ${c.subtitle}` : ''} ({plural(c.evidenceCount, 'item')})</option>{/each}
									</optgroup>
								{/each}
							</select>
						</label>
						<label class="field">
							Semester
							<select bind:value={importSemester}>
								{#each semesters as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
							</select>
						</label>
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
							{busy === 'import' ? 'Importing…' : 'Import as a new project'}
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
						<span class="hint">· {l.semesterName ?? l.semester}</span>
						<div class="hint">{l.collectionName ?? `Collection ${l.collection}: brought in at the next sync`}</div>
					</div>
					<button class="btn small ghost" onclick={() => unlink(l.project, l.semester, l.collectionName ?? 'The collection')}>Unlink</button>
				</div>
			{:else}
				<div class="hint">No project is linked yet.</div>
			{/each}

			{#if status?.signedIn}
				{#if semesters.length === 0}
					<div class="notice">Add a semester first (Settings → Semesters): a collection is linked for one semester.</div>
				{:else}
					<div class="form-row">
						<label class="field grow">
							Project
							<select bind:value={pickProject}>
								<option value="">Choose a project…</option>
								{#each projects as p (p.slug)}<option value={p.slug}>{p.name}</option>{/each}
							</select>
						</label>
						<label class="field">
							Semester
							<select bind:value={pickSemester}>
								{#each semesters as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
							</select>
						</label>
						<label class="field grow">
							Collection
							<select bind:value={pickCollection} disabled={collections === null}>
								<option value="">{collections === null ? 'Loading collections…' : pickable.length ? 'Choose a collection…' : 'Every collection is linked'}</option>
								{#each bySection as g (g.section)}
									<optgroup label={g.section || 'No section'}>
										{#each g.items as c (c.id)}<option value={String(c.id)}>{c.name}{c.subtitle ? ` · ${c.subtitle}` : ''} ({plural(c.evidenceCount, 'item')})</option>{/each}
									</optgroup>
								{/each}
							</select>
						</label>
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
				One collection per project per semester (a collection usually belongs to one semester’s section). A project that carries on gets the next semester’s
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
					<dd title={dateTime(status.lastSyncAt)}>{ago(status.lastSyncAt)}{status.summary ? ` — ${status.summary}` : ''}</dd>
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
				people are mirrored to <code>portflow/</code> in your vault, each linked collection to <code>projects/…/portflow/</code>, and each evidence item to
				<code>projects/…/evidence/</code>; Portflow stays their owner, so edits to them are overwritten by the next sync. Version 3 in Portflow is v3.0 of
				the evidence’s files: when you published v3.0 in Ledger it links to those files, and a version you uploaded in Portflow is not copied but opens
				there.
			</div>
		</div>
	{:else if !loadError}
		<div class="hint">Loading…</div>
	{/if}
</div>

<style>
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
	.writes {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		gap: 2px 12px;
		padding-left: 24px;
	}
</style>
