<script lang="ts">
	import Switch from '$lib/components/Switch.svelte';
	import { setShowLatest, sidebar } from '$lib/sidebar.svelte';
</script>

<div class="panel">
	<div class="panel-head">
		<Switch checked={sidebar.showLatest} label="Show Latest in the explorer" onchange={setShowLatest} />
		<h2>Latest in the explorer</h2>
	</div>
	<p class="sub" style="margin:0">
		Shown in the explorer on the left, under your projects: the threads and the deliverables you worked on last, from every project, a click away.
		Kept on this machine, like the theme; nothing in the vault changes.
	</p>
</div>
