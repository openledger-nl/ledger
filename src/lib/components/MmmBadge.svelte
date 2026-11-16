<script lang="ts">
	// An AI-use label as the official MMM label shows it: its picture and its name (https://mmmlabel.tech).
	import { mmmInfo } from '$lib/mmm';
	import { mmmIcon } from '$lib/mmmIcons';
	import type { MmmLevel } from '$lib/types';

	let { level, tagline = false }: { level: MmmLevel | string; tagline?: boolean } = $props();

	const info = $derived(mmmInfo(level));
	const icon = $derived(mmmIcon(level));
</script>

{#if info}
	<span class="mmm" title="{info.name} — {info.tagline}. {info.meaning} (Me & My Machine label, mmmlabel.tech)" style="--c: {info.color}">
		{#if icon}<img class="mmm-icon" src={icon} alt="" />{:else}<span class="mmm-dot"></span>{/if}{info.name}{#if tagline}<span class="mmm-tag">{info.tagline}</span>{/if}
	</span>
{/if}
