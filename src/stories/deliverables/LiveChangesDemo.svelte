<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import ChangesPanel from '$lib/components/ChangesPanel.svelte';
	import LiveChanges from '$lib/components/LiveChanges.svelte';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import { ChangeTracker } from '$lib/liveChanges.svelte';
	import * as fx from '../fixtures';

	// The deliverable page's editor with its changes on the sample text, and beside it the side window's Changes section:
	// `inline` (on unless told) marks them in the text, `open` opens the first change's card, `review` goes through what Claude Code changed.
	let { inline = true, open = false, review = false }: { inline?: boolean; open?: boolean; review?: boolean } = $props();

	try {
		localStorage.setItem('ledger.changes.inline', untrack(() => inline) ? '1' : '0');
		localStorage.setItem('ledger.changes.shown', '1');
		localStorage.removeItem(`ledger.reviewed:${fx.PROJECT}/sensor-research.md`);
	} catch {
		// Storybook without storage: the defaults.
	}

	const tracker = new ChangeTracker();
	let body = $state(fx.liveText);
	let editor: MarkdownEditor | undefined = $state();

	onMount(() => {
		const timer = setTimeout(() => {
			if (review) tracker.startReview();
			else if (open && tracker.changes[0]) tracker.open(tracker.changes[0].id);
		}, 600);
		return () => clearTimeout(timer);
	});
</script>

<div style="display:flex;gap:16px;align-items:flex-start">
	<div style="flex:1;min-width:0">
		<LiveChanges {tracker} project={fx.PROJECT} path="sensor-research.md" text={body} enabled versions={fx.historyVersions} {editor} />
		<MarkdownEditor
			bind:this={editor}
			bind:value={body}
			variant="doc"
			minHeight="420px"
			label="sensor-research.md"
			changes={tracker.view}
			onchangeclick={(id, rect) => tracker.open(id, rect)}
		/>
	</div>
	<aside class="side-window" style="padding:14px 16px;border-radius:12px;border:1px solid var(--line)">
		<ChangesPanel {tracker} live onhistory={() => {}} />
	</aside>
</div>
