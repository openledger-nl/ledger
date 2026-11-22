<script lang="ts">
	// The dialogs opened from anywhere (New project, Edit project, New deliverable); see $lib/dialogs.svelte.
	import { projectDialogs } from '$lib/dialogs.svelte';
	import DeliverableModal from './DeliverableModal.svelte';
	import ProjectModal from './ProjectModal.svelte';
</script>

{#if projectDialogs.project}
	<ProjectModal project={projectDialogs.project === 'new' ? null : projectDialogs.project} onclose={() => (projectDialogs.project = null)} />
{/if}
{#if projectDialogs.deliverable}
	<DeliverableModal
		project={projectDialogs.deliverable.project}
		answers={projectDialogs.deliverable.answers ?? null}
		onclose={() => (projectDialogs.deliverable = null)}
	/>
{/if}
