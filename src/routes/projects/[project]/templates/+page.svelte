<script lang="ts">
	import ProjectBranding from '$lib/components/ProjectBranding.svelte';
	import TemplatesPanel from '$lib/components/TemplatesPanel.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project.slug);
	// For the PDFs: who they are for when no name is given (a company project's client, else the first client listed),
	// and the other projects to take your side from.
	const clientName = $derived(data.meta?.client ?? data.people.find((p) => p.role === 'client')?.name ?? null);
	const others = $derived(data.projects.filter((p) => p.slug !== project).map((p) => ({ slug: p.slug, name: p.name })));
</script>

<svelte:head><title>Templates | {data.project.name} | Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner">
		<TemplatesPanel scope={project} templates={data.templates} />
		<ProjectBranding {project} {others} client={clientName} study={data.meta?.kind === 'study'} />
	</div>
</div>
