<script lang="ts">
	// The preview dialogs opened from anywhere (New project, New deliverable); see $lib/mock/dialogs.svelte.
	import { previewDialogs } from '$lib/mock/dialogs.svelte';
	import DeliverableModal from './DeliverableModal.svelte';
	import ProjectModal from './ProjectModal.svelte';
</script>

{#if previewDialogs.project}
	<ProjectModal project={previewDialogs.project === 'new' ? null : previewDialogs.project} onclose={() => (previewDialogs.project = null)} />
{/if}
{#if previewDialogs.deliverable}
	<DeliverableModal
		project={previewDialogs.deliverable.project}
		answers={previewDialogs.deliverable.answers ?? null}
		onclose={() => (previewDialogs.deliverable = null)}
	/>
{/if}
