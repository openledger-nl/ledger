<script lang="ts">
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { jiraApi as api } from '$lib/api';
	import { ago, dateTime } from '$lib/format';
	import { attempt, confirmDialog, toast } from '$lib/ui.svelte';
	import type { JiraMapping, JiraProjectInfo, JiraStatus, JiraView } from '$lib/types';

	const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

	let view = $state<JiraView | null>(null);
	/** The newest status the backend announced; until one arrives, the one that came with the view. */
	let live = $state<JiraStatus | null>(null);
	let loadError = $state<string | null>(null);
	const status = $derived(live ?? view?.status ?? null);

	let site = $state('');
	let email = $state('');
	let token = $state('');
	let busy = $state<string | null>(null);
	let result = $state<{ ok: boolean; message: string } | null>(null);

	/** The Jira projects the account can see, loaded once there are credentials. */
	let jiraProjects = $state<JiraProjectInfo[] | null>(null);
	let projectsError = $state<string | null>(null);

	/** Per linked project: its columns and the statuses they use. */
	let mappings = $state<Record<string, JiraMapping>>({});

	// ---- loading -------------------------------------------------------------------------------

	async function load() {
		try {
			view = await api.jiraGet();
			loadError = null;
		} catch (e) {
			loadError = message(e);
			return;
		}
		if (ready && jiraProjects === null) await loadProjects();
		await loadMappings();
	}

	async function loadProjects() {
		projectsError = null;
		try {
			jiraProjects = await api.jiraProjects();
		} catch (e) {
			jiraProjects = null;
			projectsError = message(e);
		}
	}

	async function loadMapping(slug: string) {
		try {
			mappings[slug] = await api.jiraProjectMapping(slug);
		} catch {
			delete mappings[slug];
		}
	}

	async function loadMappings() {
		if (!view) return;
		const linked = view.projects.filter((p) => p.jiraProject);
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
		listen<JiraStatus>('jira-status', (event) => {
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

	// ---- the site and credentials --------------------------------------------------------------

	const typed = () => [site.trim() || null, email.trim() || null, token.trim() || null] as const;

	async function test() {
		busy = 'test';
		result = null;
		try {
			result = { ok: true, message: await api.jiraTest(...typed()) };
		} catch (e) {
			result = { ok: false, message: message(e) };
		}
		busy = null;
	}

	async function saveCredentials() {
		busy = 'save';
		const saved = await attempt(() => api.jiraSaveCredentials(...typed()));
		busy = null;
		if (!saved) return;
		view = saved;
		site = '';
		email = '';
		token = '';
		result = null;
		toast('Jira site and token saved');
		await loadProjects();
	}

	async function disconnect() {
		const ok = await confirmDialog({
			title: 'Remove the Jira email address and token?',
			message:
				'They are removed from this machine and syncing stops. Your tasks and stories stay as they are, here and on Jira; nothing is deleted on Jira.',
			confirmLabel: 'Remove',
			danger: true
		});
		if (!ok) return;
		const saved = await attempt(() => api.jiraDisconnect());
		if (!saved) return;
		view = saved;
		jiraProjects = null;
		result = null;
	}

	// ---- projects and columns ------------------------------------------------------------------

	async function linkProject(slug: string, key: string, select: HTMLSelectElement) {
		const project = view?.projects.find((p) => p.slug === slug);
		const before = project?.jiraProject ?? '';
		if (before && before !== key) {
			const ok = await confirmDialog({
				title: key ? 'Use another Jira project?' : 'Stop syncing this project?',
				message:
					'Items already synced with the old Jira project stop syncing: their issues aren’t in the new one. They stay here, marked as no longer linked, and nothing is deleted on Jira.',
				confirmLabel: key ? 'Use this Jira project' : 'Stop syncing',
				danger: true
			});
			if (!ok) {
				select.value = before;
				return;
			}
		}
		busy = 'link:' + slug;
		const saved = await attempt(() => api.jiraLinkProject(slug, key || null));
		busy = null;
		if (!saved) {
			select.value = before;
			return;
		}
		view = saved;
		await loadMappings();
	}

	async function chooseStatus(slug: string, column: string, choice: string) {
		if ((await attempt(() => api.jiraSetColumnStatus(slug, column, choice))) !== undefined) await loadMapping(slug);
	}

	// ---- syncing -------------------------------------------------------------------------------

	async function syncNow() {
		await attempt(() => api.jiraSyncNow());
	}

	async function relink(kind: 'task' | 'story', project: string, id: string) {
		const ok = await confirmDialog({
			title: 'Give it a new issue?',
			message: 'The old issue is left alone on Jira. The next sync makes a new issue for this item and syncs it from then on.',
			confirmLabel: 'Make a new issue'
		});
		if (!ok) return;
		const saved = await attempt(() => api.jiraRelinkItem(project, kind, id));
		if (saved) view = saved;
	}

	const stateText = $derived.by(() => {
		if (!status?.configured) return 'Not syncing';
		if (status.state === 'syncing') return 'Syncing…';
		if (status.state === 'error') return 'Last sync failed';
		return status.pending ? 'Changes waiting to be looked at' : 'Up to date';
	});

	const ready = $derived(!!view && !!view.site && !!view.email && view.hasToken);
</script>

<div class="panel">
	<h2>Sync to Jira</h2>
	<p class="sub" style="margin:0">
		Keep a project’s tasks and user stories in step with the Tasks and Stories of a Jira project. Issues of other types (bugs, epics, subtasks) are never
		read or changed. Only projects you link to a Jira project are synced.
	</p>

	{#if loadError}
		<div class="notice bad">{loadError}</div>
	{/if}

	{#if view}
		<!-- 1. the site and credentials -->
		<div class="form-row">
			<label class="field grow">
				Jira site
				<input bind:value={site} placeholder={view.site ? `${view.site} — leave blank to keep it` : 'https://yourteam.atlassian.net'} spellcheck="false" autocomplete="off" />
			</label>
		</div>
		<div class="form-row">
			<label class="field grow">
				Email address
				<input bind:value={email} placeholder={view.email ? `${view.email} — leave blank to keep it` : 'The email of your Atlassian account'} spellcheck="false" autocomplete="off" />
			</label>
			<label class="field grow">
				API token
				<input
					type="password"
					bind:value={token}
					placeholder={view.hasToken ? `Saved ${view.tokenHint} — leave blank to keep it` : 'Your Atlassian API token'}
					spellcheck="false"
					autocomplete="off"
				/>
			</label>
		</div>
		<div class="hint">
			Make a token at
			<a
				href="https://id.atlassian.com/manage-profile/security/api-tokens"
				onclick={(e) => {
					e.preventDefault();
					openUrl(e.currentTarget.href).catch(() => {});
				}}>Atlassian’s API tokens page</a
			>. Ledger sends the email address and token in an <code>Authorization</code> header, never in a web address, and keeps them in this vault’s
			<code>.ledger/.env</code>: on this machine only, never uploaded, and the token is never shown here again beyond its last four characters.
		</div>

		{#if result}
			<div class="notice" class:ok={result.ok} class:bad={!result.ok}>{result.message}</div>
		{/if}

		<div class="row-end" style="justify-content:flex-start">
			<button
				class="btn"
				onclick={test}
				disabled={busy !== null || (!site.trim() && !view.site) || (!email.trim() && !view.email) || (!token.trim() && !view.hasToken)}
			>
				Test connection
			</button>
			<button
				class="btn primary"
				onclick={saveCredentials}
				disabled={busy !== null ||
					(!site.trim() && !email.trim() && !token.trim()) ||
					(!view.site && !site.trim()) ||
					(!view.email && !email.trim()) ||
					(!view.hasToken && !token.trim())}
			>
				Save
			</button>
			{#if view.email || view.hasToken}
				<button class="btn ghost danger" onclick={disconnect} disabled={busy !== null}>Remove email and token</button>
			{/if}
		</div>

		{#if ready}
			<!-- 2. projects and columns -->
			<div class="sync-detail">
				<h3 style="font-size:14px">Projects</h3>
				{#if view.projects.length === 0}
					<div class="hint">There are no projects yet.</div>
				{/if}
				{#if projectsError}
					<div class="notice bad">{projectsError}</div>
					<div class="row-end" style="justify-content:flex-start">
						<button class="btn" onclick={loadProjects}>Try again</button>
					</div>
				{/if}
				{#each view.projects as p (p.slug)}
					<div class="field">
						{p.name}
						<select
							value={p.jiraProject ?? ''}
							onchange={(e) => linkProject(p.slug, e.currentTarget.value, e.currentTarget)}
							disabled={busy !== null || jiraProjects === null}
						>
							<option value="">Not synced</option>
							{#each jiraProjects ?? [] as j (j.key)}<option value={j.key}>{j.name} ({j.key})</option>{/each}
							{#if p.jiraProject && !(jiraProjects ?? []).some((j) => j.key === p.jiraProject)}<option value={p.jiraProject}>{p.jiraProjectName}</option>{/if}
						</select>
					</div>
					{#if p.jiraProject && mappings[p.slug]}
						{@const mapping = mappings[p.slug]}
						{#if mapping.missingTypes.length > 0}
							<div class="hint" style="padding-left:12px">
								This Jira project has no {mapping.missingTypes.join(' or ')} issue type, so those items aren’t synced.
							</div>
						{/if}
						<div class="kv" style="grid-template-columns: minmax(90px, 160px) 1fr; padding-left:12px">
							{#each mapping.columns as c (c.id)}
								<dt>{c.name}</dt>
								<dd>
									<select value={c.choice} onchange={(e) => chooseStatus(p.slug, c.id, e.currentTarget.value)} disabled={busy !== null}>
										<option value="auto">{c.choice === 'auto' && c.statusName ? `Status “${c.statusName}” (same name)` : 'The status with the same name'}</option>
										<option value="none">Don’t sync this column</option>
										{#each mapping.statuses as s (s.id)}<option value={s.id}>Status “{s.name}”</option>{/each}
									</select>
									{#if !c.statusId}<span class="hint">no status: its tasks aren’t synced</span>{/if}
								</dd>
							{/each}
						</div>
						{#if mapping.unusedStatuses.length > 0}
							<div class="hint" style="padding-left:12px">
								Tasks with the status {mapping.unusedStatuses.map((n) => `“${n}”`).join(', ')} aren’t added, because no column uses it.
							</div>
						{/if}
					{/if}
				{/each}
				<div class="hint">
					A column takes the Jira status with the same name (ignoring capitals) unless you choose another. A task whose column has no status isn’t put on
					Jira, and an issue whose status has no column isn’t added here; both are counted below. Moving a task moves its issue through the Jira workflow.
					One Jira project serves one project.
				</div>
			</div>

			<!-- 3. status -->
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
							issues created {status.report.issuesCreated}, updated {status.report.issuesUpdated} · items added {status.report.itemsCreated}, updated
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
					<div class="hint">Syncing starts once there is a site, an email address and token, and at least one project linked to a Jira project.</div>
				{/if}
			</div>

			{#if view.unlinked.length > 0}
				<div class="sync-detail">
					<h3 style="font-size:14px">No longer synced</h3>
					<div class="hint">These items stopped syncing because their issue was deleted, moved to another Jira project, or made another type. They are still here, untouched.</div>
					{#each view.unlinked as item (item.project + '/' + item.kind + '/' + item.id)}
						<div class="kv" style="grid-template-columns: 1fr auto">
							<div>
								<strong>{item.title}</strong>
								<span class="hint">· {item.kind === 'story' ? 'user story' : 'task'} in {item.projectName}</span>
								<div class="hint">{item.reason}</div>
							</div>
							<button class="btn small" onclick={() => relink(item.kind, item.project, item.id)}>Make a new issue</button>
						</div>
					{/each}
				</div>
			{/if}

			<div class="hint">
				Nothing is ever deleted on Jira, and deleting on one side never deletes on the other: an item whose issue is gone stays here and stops syncing, and
				an issue whose item you delete stays on Jira. When both sides change the same text, this machine’s version wins and Jira’s is kept in the vault’s
				<code>.ledger/conflicts</code> folder. A description on Jira with pictures, tables or mentions in it is never overwritten from here. Ledger looks at Jira
				every few minutes and shortly after you make changes, and waits when Jira asks it to slow down.
			</div>
			<div class="hint">
				Every issue that has an item here carries the label <code>ledger</code>. An issue with that label is never added from Jira (it reaches your other
				machines through the vault), so an item you deleted stays deleted and two machines never add the same item twice. Take the label off to bring an
				issue in anyway. A user story’s description on Jira reads <code>**As a** … / **I want to** … / **Such that** …</code>, then the acceptance criteria as a
				list, then the notes; you can edit it there.
			</div>
		{/if}
	{:else if !loadError}
		<div class="hint">Loading…</div>
	{/if}
</div>
