<script lang="ts">
	// "What the labels mean": opens https://mmmlabel.tech in the system browser.
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { MMM_SITE } from '$lib/mmm';

	let { text = 'mmmlabel.tech' }: { text?: string } = $props();
</script>

<button type="button" class="mmm-site" title={MMM_SITE} onclick={() => openUrl(MMM_SITE).catch(() => {})}>{text}</button>
