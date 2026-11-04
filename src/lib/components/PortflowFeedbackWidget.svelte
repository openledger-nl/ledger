<script lang="ts">
	// The dashboard's list of open feedback from Portflow: what coaches and experts wrote that you haven't resolved.
	import MessageSquare from '@lucide/svelte/icons/message-square';
	import { portflowApi } from '$lib/api';
	import { threadUrl } from '$lib/actions';
	import { ago } from '$lib/format';
	import { portflow } from '$lib/portflow.svelte';
	import type { PortflowFeedback } from '$lib/types';

	let { version }: { version: unknown } = $props();
	let items = $state<PortflowFeedback[]>([]);

	$effect(() => {
		void version;
		void portflow.status?.lastSyncAt;
		if (!portflow.enabled) {
			items = [];
			return;
		}
		let stale = false;
		portflowApi.recentFeedback(6).then(
			(all) => !stale && (items = all),
			() => !stale && (items = [])
		);
		return () => (stale = true);
	});
</script>

{#if portflow.enabled && items.length}
	<section class="proj-group" aria-label="Open feedback from Portflow">
		<h2 class="proj-group-head"><MessageSquare size={13} />Open feedback from Portflow</h2>
		<div class="hit-list">
			{#each items as f (f.id)}
				<a class="hit" href="{threadUrl(f.project, f.thread)}?item=f-{f.id}">
					<div class="hit-head">
						<span class="hit-kind">{f.from ?? 'Someone'}{f.fromRole ? ` · ${f.fromRole}` : ''}</span>
						<span class="hit-title">{f.evidenceName ?? f.evidence ?? ''}</span>
						<span class="badge accent">{f.projectName}</span>
						<span class="hint">{ago(f.createdAt)}</span>
					</div>
					<div class="hit-snippet">{f.excerpt}</div>
				</a>
			{/each}
		</div>
	</section>
{/if}
