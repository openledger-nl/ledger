<script lang="ts">
	import CircleAlert from '@lucide/svelte/icons/circle-alert';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import CornerDownRight from '@lucide/svelte/icons/corner-down-right';
	import FileCode from '@lucide/svelte/icons/file-code';
	import GitCompareArrows from '@lucide/svelte/icons/git-compare-arrows';
	import Info from '@lucide/svelte/icons/info';
	import MessageSquare from '@lucide/svelte/icons/message-square';
	import PencilLine from '@lucide/svelte/icons/pencil-line';
	import { dateTime, dayKey, plural, time } from '$lib/format';
	import type { Review, ReviewComment } from '$lib/types';
	import Markdown from './Markdown.svelte';

	/**
	 * A code review written with margin-code in VS Code: how it stands, what it compared, its summary, and its comments by
	 * file, each with the code it is about and its replies. Read-only: the review is edited in VS Code, and Ledger never
	 * writes its files.
	 */
	let { review }: { review: Review } = $props();

	const VERDICTS: Record<string, string> = { approve: 'Approved', changes: 'Changes requested', comment: 'Commented' };
	const status = $derived.by(() => {
		if (review.state === 'draft') return { label: 'In progress', tone: 'draft', icon: PencilLine };
		const verdict = review.verdict ?? '';
		const label = VERDICTS[verdict] ?? (verdict ? verdict[0].toUpperCase() + verdict.slice(1) : 'Submitted');
		if (verdict === 'approve') return { label, tone: 'approve', icon: CircleCheck };
		if (verdict === 'changes') return { label, tone: 'changes', icon: CircleAlert };
		return { label, tone: 'comment', icon: MessageSquare };
	});
	const StatusIcon = $derived(status.icon);

	const short = (sha: string) => sha.slice(0, 7);
	/** What was compared, by the names the commits had ("main..feat/code"), or else by their short SHAs. */
	const named = $derived(review.baseRef || review.headRef);
	const range = $derived(
		named ? `${review.baseRef || short(review.base)}..${review.headRef || short(review.head)}` : `${short(review.base)}..${short(review.head)}`
	);

	/** A comment's time, with its day when that isn't the day the review was started (the day it is filed under). */
	const when = (iso: string) => (dayKey(iso) === dayKey(review.createdAt) ? time(iso) : dateTime(iso));

	/** Where a comment is: `path:42`, `path:42-44`, or the path alone for the whole file. */
	function place(path: string, c: ReviewComment): string {
		const name = path || 'No file';
		if (c.line === null) return name;
		return c.endLine !== null && c.endLine !== c.line ? `${name}:${c.line}-${c.endLine}` : `${name}:${c.line}`;
	}

	/**
	 * Whether a comment's lines are numbered in another commit than the head the review shows: a draft moved on to a
	 * newer head keeps the comments made before, with their own commit and lines.
	 */
	const elsewhere = (c: ReviewComment) => c.side === 'new' && c.line !== null && !!c.commit && c.commit !== review.head;

	/** The lines of a block of code, without the newline that ends it. */
	function lines(text: string | null): string[] {
		if (!text) return [];
		return text.replace(/\r\n?/g, '\n').replace(/\n$/, '').split('\n');
	}

	/** The commented lines with the ones around them, numbered where the comment says where it is. */
	function excerpt(c: ReviewComment) {
		const before = lines(c.codeBefore);
		const code = lines(c.code);
		const after = lines(c.codeAfter);
		const at = (n: number | null) => (n !== null && n >= 1 ? n : null);
		const first = c.line;
		const last = c.endLine ?? (first !== null ? first + code.length - 1 : null);
		return [
			...before.map((text, i) => ({ text, n: at(first !== null ? first - before.length + i : null), context: true })),
			...code.map((text, i) => ({ text, n: at(first !== null ? first + i : null), context: false })),
			...after.map((text, i) => ({ text, n: at(last !== null ? last + 1 + i : null), context: true }))
		];
	}
</script>

<div class="card review-card {status.tone}" id="rv-{review.id}">
	<div class="meta">
		<span class="card-kind">Code review</span>
		<span class="badge rv-status {status.tone}"><StatusIcon size={12} />{status.label}</span>
		<span class="time">{time(review.createdAt)}</span>
		{#if review.author}<span class="file">by {review.author}</span>{/if}
		{#if review.submittedAt}<span class="file">· submitted {when(review.submittedAt)}</span>{/if}
	</div>

	<div class="rv-target">
		<GitCompareArrows size={14} />
		<span class="rv-repo" title={review.repo}>{review.repoName || review.repo || 'Unknown repository'}</span>
		<code class="rv-range" title="{review.base}..{review.head}">{range}</code>
		{#if named}<span class="rv-shas" title="{review.base}..{review.head}">{short(review.base)}..{short(review.head)}</span>{/if}
		{#if review.commentCount}
			<span class="hint">{plural(review.commentCount, 'comment')}{review.openCount ? ` · ${review.openCount} open` : ''}</span>
		{/if}
	</div>

	{#if review.newerFormat}
		<div class="rv-note">
			<Info size={14} />
			<span>Written by a newer margin-code (format {review.format}) than this version of Ledger knows. It is shown as far as it can be read; update Ledger to see all of it.</span>
		</div>
	{/if}

	{#if review.summary.trim()}<Markdown html={review.summaryHtml} />{/if}

	{#if review.files.length}
		<div class="rv-files">
			{#each review.files as file (file.path)}
				<div class="rv-file">
					<div class="rv-path"><FileCode size={14} />{file.path || 'Not on a file'}</div>
					{#each file.comments as c (c.id)}
						{@const rows = excerpt(c)}
						<div class="rv-comment" class:resolved={c.resolved} id="rc-{c.id}">
							<div class="rv-head">
								<span class="rv-place">{place(file.path, c)}</span>
								{#if c.side === 'old'}<span class="hint" title="The line is in the version compared against">old version</span>{/if}
								{#if elsewhere(c)}
									<span class="hint" title="Lines as they were in {c.commit}; the review has since moved on to {review.head}">at {short(c.commit ?? '')}</span>
								{/if}
								<span class="rv-state">{c.resolved ? 'Resolved' : 'Open'}</span>
								<span>{c.author ? `${c.author} · ` : ''}{when(c.createdAt)}</span>
								{#if c.newerFormat}<span class="hint" title="Written by a newer margin-code: shown as far as it can be read">newer format</span>{/if}
							</div>
							{#if rows.length}
								<div class="rv-code" role="figure" aria-label="The code commented on">
									{#each rows as row, i (i)}
										<div class="rv-row" class:context={row.context}><span class="rv-ln">{row.n ?? ''}</span><span class="rv-text">{row.text}</span></div>
									{/each}
								</div>
							{/if}
							{#if c.body.trim()}<Markdown html={c.html} />{/if}
							{#if c.replies.length}
								<div class="rv-replies">
									{#each c.replies as r (r.id)}
										<div class="rv-reply" id="rc-{r.id}">
											<CornerDownRight size={13} />
											<div class="rv-reply-body">
												<div class="rv-by">{r.author ? `${r.author} · ` : ''}{when(r.createdAt)}</div>
												<Markdown html={r.html} />
											</div>
										</div>
									{/each}
								</div>
							{/if}
						</div>
					{/each}
				</div>
			{/each}
		</div>
	{/if}

	<div class="rv-foot hint"><Info size={13} />Read-only here: reviews are written and edited in VS Code with margin-code.</div>
</div>
