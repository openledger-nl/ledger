<script lang="ts">
	// What the AI-use labels mean: opens https://mmmlabel.tech in the system browser, from the name of a label's field.
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { MMM_SITE } from '$lib/mmm';

	let { text = 'mmmlabel.tech' }: { text?: string } = $props();
</script>

<button type="button" class="mmm-site" title={MMM_SITE} onclick={() => openUrl(MMM_SITE).catch(() => {})}>{text}</button>
