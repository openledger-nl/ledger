<script lang="ts">
	// Choosing an AI-use label: the five official MMM labels with their pictures and what each means, and a link to
	// mmmlabel.tech. The label is always yours to declare; Ledger never picks one for you.
	import { MMM } from '$lib/mmm';
	import { MMM_ICONS } from '$lib/mmmIcons';
	import type { MmmLevel } from '$lib/types';
	import Dropdown, { type DropdownOption } from './Dropdown.svelte';
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

	const options = $derived<DropdownOption[]>([
		{ value: '', label: empty, hint: 'No label (yet).' },
		...MMM.map((m) => ({ value: m.level, label: m.name, hint: `${m.tagline}. ${m.meaning}`, image: MMM_ICONS[m.level] }))
	]);
</script>

<!-- Dropdown works on strings; the empty string is "no label". -->
<Dropdown bind:value={() => value, (v) => (value = v as MmmLevel | '')} {options} {label} {disabled} full searchFrom={99}>
	{#snippet footer()}What the labels mean: <MmmLink />{/snippet}
</Dropdown>
