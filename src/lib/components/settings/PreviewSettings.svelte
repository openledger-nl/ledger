<script lang="ts">
	// The v1 preview screens: drawn from the Penpot wireframes, working on mock data until the backend has what they need.
	import Switch from '$lib/components/Switch.svelte';
	import { mockApi } from '$lib/mock/api';
	import { previews, setPreviews } from '$lib/mock/flag.svelte';
	import { confirmDialog, refresh, toast } from '$lib/ui.svelte';

	async function reset() {
		const ok = await confirmDialog({
			title: 'Reset the preview data?',
			message: 'Everything made in the preview screens (meeting notes, feedback, goals, labels) is forgotten and the examples come back. Nothing in the vault changes.',
			confirmLabel: 'Reset',
			danger: true
		});
		if (!ok) return;
		await mockApi.reset();
		await refresh();
		toast('Preview data reset');
	}
</script>

<div class="panel">
	<div class="panel-head">
		<Switch checked={previews.enabled} label="Show the preview screens" onchange={setPreviews} />
		<h2>Preview screens</h2>
	</div>
	<p class="sub" style="margin:0">
		The v1 screens, before the vault can hold what they need: project kinds, goals and end dates, the new project and deliverable dialogs,
		post kinds and labels, meeting notes, the feedback inbox, and the intended label when publishing. They are marked <em>Preview</em>, and what you do
		in them is kept in this browser, not in the vault. Projects and deliverables they create are real; the extra facts about them are not, yet.
	</p>
	<div class="row-end" style="justify-content:flex-start">
		<button class="btn small ghost danger" onclick={reset}>Reset the preview data</button>
	</div>
</div>
