<script lang="ts">
	// The templates every project shares, one file each: the ones deliverables start from (shared/templates/prompts/),
	// which a project can replace with its own on its Templates tab, and the ones semesters start from
	// (shared/templates/semesters/). A new vault starts with a project plan and NJ26, the vault's to change or delete.
	import SemesterTemplatesPanel from '$lib/components/SemesterTemplatesPanel.svelte';
	import TemplatesPanel from '$lib/components/TemplatesPanel.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>Templates | Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner">
		<TemplatesPanel scope="" templates={data.templates} />
		<SemesterTemplatesPanel templates={data.semesterTemplates} />
	</div>
</div>
