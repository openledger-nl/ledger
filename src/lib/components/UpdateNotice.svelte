<script lang="ts">
	import CircleArrowUp from '@lucide/svelte/icons/circle-arrow-up';
	import GitBranch from '@lucide/svelte/icons/git-branch';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import X from '@lucide/svelte/icons/x';
	import Markdown from './Markdown.svelte';
	import Modal from './Modal.svelte';
	import { date } from '$lib/format';
	import { installUpdate, later, stopWaiting, updates } from '$lib/updates.svelte';

	// What the backend found: a release (a notice in the corner, with its notes), or, in a development build, what
	// `pnpm run dev:app` holds back of the changes in its checkout, with the commits that came with them (a prompt in the
	// middle of the window). Either way nothing happens until the answer is yes, and then it waits for what a restart would
	// cut off before it goes ahead.

	const info = $derived(updates.available);
	const busy = $derived(updates.phase === 'working' || updates.phase === 'waiting');
	const percent = $derived.by(() => {
		const p = updates.progress;
		return p?.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : null;
	});
	const downloading = $derived(updates.phase === 'working' && updates.progress !== null && percent !== 100);

	/** “A”, “B” and “C”. */
	function quoted(titles: string[]) {
		const q = titles.map((t) => `“${t.trim() || 'a prompt'}”`);
		return q.length > 1 ? `${q.slice(0, -1).join(', ')} and ${q.at(-1)}` : (q[0] ?? '');
	}

	const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
	/** The branch, or what to call a checkout that is on a commit. */
	const branchName = (branch: string) => (branch === 'HEAD' ? 'this checkout' : branch);
</script>

<!-- Where installing has got to, the same for both: the answer was yes, or it is over. -->
{#snippet status(branch: boolean)}
	{#if updates.phase === 'working'}
		<div class="un-status">
			{#if downloading}
				<div class="un-progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent ?? undefined}>
					<span style:width="{percent ?? 0}%"></span>
				</div>
				<span>Downloading… {percent ?? 0}%</span>
			{:else if branch}
				<span><span class="dot pulse"></span> Bringing in the new version…</span>
			{:else}
				<span><span class="dot pulse"></span> Getting the update ready. Ledger restarts when it is installed.</span>
			{/if}
		</div>
	{:else if updates.phase === 'waiting'}
		<div class="un-status">
			<span>
				<span class="dot warn pulse"></span>
				{branch ? 'Ready.' : 'Downloaded.'} Waiting for {updates.waitingFor.join(' and ')}, then Ledger {branch ? 'updates' : 'installs it and restarts'}.
			</span>
			<div class="un-foot"><button class="btn small ghost" onclick={stopWaiting}>Don’t install now</button></div>
		</div>
	{:else if updates.phase === 'ask'}
		<div class="un-status">
			<span>
				Claude Code is still running {quoted(updates.runs)}. Restarting stops {updates.runs.length === 1 ? 'that run' : 'those runs'}.
			</span>
			<div class="un-foot">
				<button class="btn primary" onclick={() => installUpdate('wait')}>Wait, then {branch ? 'update' : 'install'}</button>
				<button class="btn danger" onclick={() => installUpdate('stop')}>Stop and {branch ? 'update' : 'install'}</button>
				<button class="btn ghost" onclick={stopWaiting}>Cancel</button>
			</div>
		</div>
	{:else if updates.phase === 'applied' && updates.applied}
		<div class="un-status">
			<div class="notice ok">
				{updates.applied.restart
					? `Restarting into the new build, at ${updates.applied.head}. pnpm run dev:app starts it as soon as it is ready.`
					: 'Only the front end changed, so the window reloads.'}
			</div>
			<div class="un-foot"><button class="btn small" onclick={later}>Close</button></div>
		</div>
	{:else if updates.phase === 'failed'}
		<div class="un-status">
			<div class="notice bad">{updates.error}</div>
			<div class="un-foot">
				<button class="btn primary" onclick={() => installUpdate()}>Try again</button>
				<button class="btn ghost" onclick={later}>{branch ? 'Not now' : 'Later'}</button>
			</div>
		</div>
	{/if}
{/snippet}

{#if info && !updates.hidden}
	{#if info.kind === 'branch'}
		<Modal title={info.behind ? `New on ${branchName(info.branch)}` : 'Changes in your checkout'} onclose={() => !busy && later()}>
			<div class="un-branch">
				<p>
					<GitBranch size={14} /> This development build runs <code>{branchName(info.branch)}</code>{#if info.head}
						as it was at <code>{info.head}</code>{/if}.
					{#if info.behind}
						The branch has <b>{plural(info.behind, 'commit')}</b> since:
					{:else}
						Files in the checkout changed since it was built.
					{/if}
				</p>
				<ul class="un-commits">
					{#each info.commits as commit (commit.hash)}
						<li>
							<code>{commit.hash}</code>
							<span>{commit.subject}</span>
							{#if commit.breaking}<span class="badge draft">breaking</span>{/if}
						</li>
					{/each}
				</ul>
				{#if info.more}<div class="hint">…and {plural(info.more, 'older commit')}.</div>{/if}
				{#if updates.phase !== 'applied' && info.commits.some((c) => c.breaking)}
					<div class="un-major">
						<TriangleAlert size={15} />
						<span><b>Some of these are breaking changes.</b> Something you rely on may work differently afterwards.</span>
					</div>
				{/if}
				{#if updates.phase !== 'applied'}
					{#if info.build === 'building'}
						<div class="un-status"><span><span class="dot pulse"></span> Building the new version. You can update once it is ready.</span></div>
					{:else if info.build === 'failed'}
						<div class="notice bad">
							The new version doesn’t build{info.error ? `: ${info.error}` : '.'} This one keeps running. Fix it and save, and it is built again.
						</div>
					{:else if !info.restart}
						<div class="hint">Only the front end changed: updating reloads the window.</div>
					{/if}
					{#if info.dependencies}
						<div class="notice info"><code>package.json</code> changed: the packages are installed when you update.</div>
					{/if}
				{/if}
				{#if updates.phase === 'idle'}
					<p><b>{info.restart ? 'Update and restart Ledger?' : 'Update the window?'}</b></p>
					<!-- It can come up while something is being typed, so the key that is pressed by accident is the one that says no. -->
					<div class="un-foot">
						<button class="btn primary" disabled={info.build === 'building' || info.build === 'failed'} onclick={() => installUpdate()}>
							{info.restart ? 'Update and restart' : 'Update'}
						</button>
						<button class="btn ghost" data-autofocus onclick={later}>Not now</button>
					</div>
				{:else}
					{@render status(true)}
				{/if}
			</div>
		</Modal>
	{:else}
		<aside class="update-notice" class:major={info.major} aria-label="Update available">
			<div class="un-head">
				<span class="un-icon"><CircleArrowUp size={18} /></span>
				<div class="un-title">
					<strong>Ledger v{info.version} is available</strong>
					<span class="un-sub">You have v{info.currentVersion}{info.date ? ` · released ${date(info.date)}` : ''}</span>
				</div>
				{#if !busy}
					<button class="icon-btn" aria-label="Later" title="Later" onclick={later}><X size={16} /></button>
				{/if}
			</div>

			{#if info.major}
				<div class="un-major">
					<TriangleAlert size={15} />
					<span>
						<b>A major update.</b> Something you rely on may work differently afterwards{info.breakingHtml
							? ': read what changes before you install.'
							: '.'}
					</span>
				</div>
			{/if}

			<div class="un-body">
				{#if info.breakingHtml}
					<div class="un-breaking">
						<div class="un-label">Breaking changes</div>
						<Markdown html={info.breakingHtml} />
					</div>
				{/if}
				{#if info.notesHtml}
					<div class="un-notes"><Markdown html={info.notesHtml} /></div>
				{:else if !info.breakingHtml}
					<div class="hint">No release notes.</div>
				{/if}
			</div>

			{#if updates.phase === 'idle'}
				<div class="un-foot">
					<button class="btn primary" onclick={() => installUpdate()}>Install and restart</button>
					<button class="btn ghost" onclick={later}>Later</button>
				</div>
			{:else}
				{@render status(false)}
			{/if}
		</aside>
	{/if}
{/if}
