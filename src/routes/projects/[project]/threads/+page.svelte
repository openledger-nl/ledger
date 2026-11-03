<script lang="ts">
	import MessagesSquare from '@lucide/svelte/icons/messages-square';
	import { newThreadFlow, threadUrl } from '$lib/actions';
	import { ago, threadMeta } from '$lib/format';
	import { orderThreads } from '$lib/order';
	import { listOrder } from '$lib/order.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const project = $derived(data.project);
	const threads = $derived(orderThreads(data.projects.find((p) => p.slug === project.slug)?.threads ?? [], listOrder.order));
</script>

<svelte:head><title>Threads · {project.name} · Ledger</title></svelte:head>

<div class="page">
	<div class="page-inner wide">
		<section class="panel">
			<div class="header-top">
				<div>
					<h2>Threads</h2>
					<div class="sub">One per topic you're juggling: a running conversation with its prompts, feedback and uploads.</div>
				</div>
				<button class="btn primary small" onclick={() => newThreadFlow(project.slug)}>New thread</button>
			</div>
			{#each threads as t (t.slug)}
				<a class="dash-row" href={threadUrl(project.slug, t.slug)}>
					<span class="dash-ico"><MessagesSquare size={16} /></span>
					<span class="dash-main">
						<span class="dash-title">{t.title}</span>
						<span class="dash-meta">{threadMeta(t)}</span>
					</span>
					<span class="dash-when">{ago(t.lastActivity)}</span>
				</a>
			{:else}
				<div class="hint">No threads yet. A thread is a running conversation about one topic.</div>
			{/each}
		</section>
	</div>
</div>
