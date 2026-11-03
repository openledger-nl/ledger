<script lang="ts">
	import { api } from '$lib/api';
	import Markdown from '$lib/components/Markdown.svelte';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import { attempt, refresh } from '$lib/ui.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project);
	// The guide is the project's README.md: pictures in it are looked for from the project's folder.
	const images = $derived({ project: project.slug, dir: '' });

	let editing = $state(false);
	let draft = $state('');
	let saving = $state(false);

	function edit() {
		draft = project.readme;
		editing = true;
	}

	async function save() {
		saving = true;
		const done = await attempt(() => api.saveReadme(project.slug, draft));
		saving = false;
		if (!done) return;
		editing = false;
		await refresh();
	}
</script>

<div class="page">
	<div class="page-inner wide">
		<section class="panel">
			<div class="header-top">
				<div>
					<h2>Reading guide</h2>
					<div class="sub">README.md — where to start, what matters, how this project works.</div>
				</div>
				{#if !editing}
					<button class="btn small" onclick={edit}>Edit</button>
				{/if}
			</div>
			{#if editing}
				<MarkdownEditor
					bind:value={draft}
					variant="doc"
					minHeight="58vh"
					label="Reading guide"
					autofocus
					spellcheck={false}
					{images}
					onsubmit={save}
					oncancel={() => (editing = false)}
				/>
				<div class="row-end">
					<span class="hint" style="align-self:center">Ctrl+Enter saves</span>
					<button class="btn ghost" onclick={() => (editing = false)}>Cancel</button>
					<button class="btn primary" onclick={save} disabled={saving}>Save</button>
				</div>
			{:else if project.readme.trim()}
				<Markdown html={project.readmeHtml} {images} />
			{:else}
				<p class="hint" style="margin:0">The guide is empty. Write down what a newcomer should read first.</p>
			{/if}
		</section>
	</div>
</div>
