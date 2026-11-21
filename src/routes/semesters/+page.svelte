<script lang="ts">
	// The vault's semesters, shared by every project: kept in shared/semesters/, one file each. The list and the + that
	// adds one (from a preset or from scratch) are SemesterList's; this page puts them under the Shared heading of the
	// explorer.
	import SemesterList from '$lib/components/SemesterList.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>Semesters | Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner">
		<SemesterList semesters={data.semesters} />
	</div>
</div>
