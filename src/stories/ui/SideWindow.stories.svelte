<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import FileText from '@lucide/svelte/icons/file-text';
	import MessageSquareText from '@lucide/svelte/icons/message-square-text';
	import SideWindow from '$lib/components/SideWindow.svelte';

	// The side window beside a page (a deliverable's document, a thread's timeline): a rail with a button per section.
	const { Story } = defineMeta({ title: 'UI/SideWindow', component: SideWindow });

	const sections = [
		{ id: 'deliverables', title: 'Deliverables', icon: FileText, count: 2, headCount: '2' },
		{ id: 'notes', title: 'Notes', icon: MessageSquareText }
	];
</script>

<Story name="Beside a page" asChild>
	<div style="height: 520px; display: flex">
		<div style="flex: 1; padding: 24px; color: var(--muted)">The page</div>
		<SideWindow {sections} storageKey="storybook-side-window">
			{#snippet panel(id)}
				{#if id === 'deliverables'}
					<div class="card"><strong>Sensor research</strong></div>
					<div class="card"><strong>Project plan</strong></div>
				{:else}
					<p class="hint">Nothing here yet.</p>
				{/if}
			{/snippet}
		</SideWindow>
	</div>
</Story>
