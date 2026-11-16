<script lang="ts">
	// Choosing an AI-use label: no label, then the five official MMM labels from the most AI to the least, side by side
	// in one row to click, each with its picture; hovering one tells what it means. A link to mmmlabel.tech says more.
	// The label is always yours to declare; Ledger never picks one for you. Radio buttons underneath, so the arrow keys
	// move along the row.
	import CircleDashed from '@lucide/svelte/icons/circle-dashed';
	import { MMM } from '$lib/mmm';
	import { MMM_ICONS } from '$lib/mmmIcons';
	import type { MmmLevel } from '$lib/types';
	import MmmLink from './MmmLink.svelte';

	let {
		value = $bindable(''),
		label = 'AI-use label',
		empty = 'Not set',
		disabled = false
	}: {
		value?: MmmLevel | '';
		label?: string;
		/** What no label is called here: "Not set", "Not labelled". */
		empty?: string;
		disabled?: boolean;
	} = $props();

	const name = $props.id();
	const options = $derived([
		{ level: '' as const, name: empty, title: 'No label (yet).', icon: null },
		...MMM.map((m) => ({ level: m.level, name: m.name, title: `${m.name} - ${m.tagline}. ${m.meaning}`, icon: MMM_ICONS[m.level] }))
	]);
</script>

<div class="mmm-picker">
	<div class="mmm-row" role="radiogroup" aria-label={label}>
		{#each options as o (o.level)}
			<label class="mmm-option" class:on={value === o.level} class:disabled title={o.title}>
				<input type="radio" {name} value={o.level} checked={value === o.level} {disabled} onchange={() => (value = o.level)} />
				{#if o.icon}
					<img src={o.icon} alt="" />
				{:else}
					<span class="mmm-none"><CircleDashed size={22} /></span>
				{/if}
				<span class="mmm-name">{o.name}</span>
			</label>
		{/each}
	</div>
	<span class="hint">What the labels mean: <MmmLink /></span>
</div>

<style>
	.mmm-picker {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	/* Six in a row; below about 80px a tile they wrap rather than squeeze the names. */
	.mmm-row {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(76px, 1fr));
		gap: 6px;
	}
	.mmm-option {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		padding: 8px 4px 6px;
		border: 1px solid var(--line);
		border-radius: 8px;
		background: var(--card);
		cursor: pointer;
		text-align: center;
	}
	.mmm-option:hover {
		border-color: var(--muted);
	}
	.mmm-option.on {
		border-color: var(--accent);
		background: var(--accent-soft);
		box-shadow: 0 0 0 1px var(--accent);
	}
	.mmm-option:has(input:focus-visible) {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
	.mmm-option.disabled {
		cursor: default;
		opacity: 0.55;
	}
	.mmm-option.disabled:hover {
		border-color: var(--line);
	}
	.mmm-option.on.disabled:hover {
		border-color: var(--accent);
	}
	/* The radio button itself is not drawn: the tile is. */
	input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}
	img,
	.mmm-none {
		width: 32px;
		height: 32px;
	}
	.mmm-none {
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--muted);
	}
	.mmm-name {
		font-size: 11px;
		font-weight: 600;
		line-height: 1.2;
		color: var(--ink);
	}
</style>
