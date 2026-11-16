<script lang="ts">
	// Marks a screen or a part of one that runs on mock data: built from the Penpot wireframes before the backend has
	// what it needs. What you do there is kept in this browser, not in the vault.
	import FlaskConical from '@lucide/svelte/icons/flask-conical';

	let { text = 'Preview' }: { text?: string } = $props();
</script>

<span class="preview-badge" title="Preview: mock data, kept in this browser and not in the vault. Settings › Features › Preview screens.">
	<FlaskConical size={11} />{text}
</span>
