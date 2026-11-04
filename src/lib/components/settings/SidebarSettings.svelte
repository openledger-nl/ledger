<script lang="ts">
	import { setSidebarView, sidebar, type SidebarView } from '$lib/sidebar.svelte';

	const choices: { value: SidebarView; label: string }[] = [
		{ value: 'both', label: 'Latest and projects' },
		{ value: 'projects', label: 'Only projects' },
		{ value: 'latest', label: 'Only latest' }
	];
</script>

<div class="panel">
	<h2>Sidebar</h2>
	<p class="sub" style="margin:0">
		What the sidebar on the left shows under Home. Kept on this machine, like the theme; nothing in the vault changes.
	</p>

	<div class="field">
		<div class="seg" role="group" aria-label="What the sidebar shows" style="align-self:flex-start">
			{#each choices as c (c.value)}
				<button class:on={sidebar.view === c.value} aria-pressed={sidebar.view === c.value} onclick={() => setSidebarView(c.value)}>
					{c.label}
				</button>
			{/each}
		</div>
		<span class="hint">
			Latest lists the threads and deliverables you worked on last, across all projects. Projects is the tree of your projects with their threads and
			deliverables. Your projects stay on the home page either way.
		</span>
	</div>
</div>
