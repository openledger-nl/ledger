<script lang="ts">
	import GitCompareArrows from '@lucide/svelte/icons/git-compare-arrows';
	import UserRoundPen from '@lucide/svelte/icons/user-round-pen';
	import type { ChangeTracker } from '$lib/liveChanges.svelte';
	import { authorLabel } from '$lib/mmm';
	import Dropdown from './Dropdown.svelte';
	import Switch from './Switch.svelte';

	/**
	 * The Changes section of a deliverable's side window. The changes themselves are in the text (editorChanges.ts); here is
	 * what doesn't fit there: which version they are counted from, how they show, and the views over the whole text (two versions compared, who wrote what).
	 */
	let {
		tracker,
		live,
		onhistory
	}: {
		tracker: ChangeTracker;
		/** The live file is open (not a published version), so its changes can be followed. */
		live: boolean;
		/** Opens a view over the whole text: two versions compared, or who wrote what. */
		onhistory: (view: 'changes' | 'authors') => void;
	} = $props();

	const versions = $derived(tracker.versions);
	const options = $derived([...versions].reverse().map((v) => ({ value: v.version, label: `v${v.version}`, hint: authorLabel(v) })));
	const since = $derived(tracker.data?.against ?? versions.at(-1)?.version ?? '');
	const count = $derived(tracker.changes.length);

</script>

<div class="cp">
	{#if live}
		{#if !versions.length}
			<p class="hint">Publish a version, and from then on every change since shows here and beside the text: who made it, and what was there before.</p>
		{:else}
			<div class="cp-since">
				<span class="cp-count">{count ? `${count} ${count === 1 ? 'change' : 'changes'}` : 'No changes'}</span>
				<span class="cp-muted">since</span>
				<Dropdown bind:value={() => since, (v) => tracker.since(v)} {options} label="Changes since" />
			</div>


			{#if tracker.data && !count}
				<p class="hint">The text is the same as v{since}.</p>
			{:else if count}
				<p class="hint">Shown in the text: click a change to see what was there before, or to undo it.</p>
			{/if}

			<div class="cp-settings">
				<label class="cp-setting">
					<span>Bars in the margin<span class="hint">Green for you, the accent colour for AI</span></span>
					<Switch checked={tracker.shown} label="Bars in the margin" onchange={(on) => tracker.setShown(on)} />
				</label>
				<label class="cp-setting">
					<span>In the text<span class="hint">Added words underlined, removed ones struck through</span></span>
					<Switch checked={tracker.inline} label="Changes in the text" onchange={(on) => tracker.setInline(on)} />
				</label>
			</div>
		{/if}
	{/if}

	<div class="cp-views">
		<div class="cp-views-title">Over the whole text</div>
		<button class="cp-view" disabled={versions.length + (live ? 1 : 0) < 2} onclick={() => onhistory('changes')}>
			<GitCompareArrows size={16} />
			<span><strong>Compare versions</strong><span class="hint">As Word's track changes, or line by line as git</span></span>
		</button>
		<button class="cp-view" disabled={!versions.length} onclick={() => onhistory('authors')}>
			<UserRoundPen size={16} />
			<span><strong>Who wrote what</strong><span class="hint">What AI wrote, and where its words ended up</span></span>
		</button>
	</div>
</div>

<style>
	.cp {
		display: flex;
		flex-direction: column;
		gap: 14px;
		font-size: 13px;
	}
	.cp-since {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
	}
	.cp-count {
		font-weight: 600;
	}
	.cp-muted {
		color: var(--muted);
	}
	.cp-settings {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding-top: 12px;
		border-top: 1px solid var(--line);
	}
	.cp-setting {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		cursor: pointer;
	}
	.cp-setting > span {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
	.cp-views {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding-top: 12px;
		border-top: 1px solid var(--line);
	}
	.cp-views-title {
		font-size: 12px;
		color: var(--faint);
	}
	.cp-view {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 9px 10px;
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--card);
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.cp-view:hover:not(:disabled) {
		border-color: var(--accent);
	}
	.cp-view:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.cp-view :global(svg) {
		color: var(--accent);
		flex: none;
		margin-top: 1px;
	}
	.cp-view > span {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
</style>
