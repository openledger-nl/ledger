<script lang="ts">
	// A post's kind and its labels (Fact, To check, Assumption), set by hand: Ledger never labels a post. Shown in the
	// post's meta row; the tag button opens the choice below it. Wireframe C1; requirements: Threads.
	import Tag from '@lucide/svelte/icons/tag';
	import { api } from '$lib/api';
	import { attempt, refresh } from '$lib/ui.svelte';
	import { LABELS, labelName, POST_KINDS, postKindName } from '$lib/v1';
	import type { EpistemicLabel, PostKind, PostMeta } from '$lib/types';

	let { project, thread, postId, meta }: { project: string; thread: string; postId: string; meta: PostMeta | null } = $props();

	let open = $state(false);

	async function save(kind: PostKind, labels: EpistemicLabel[]) {
		if (await attempt(() => api.setPostMeta(project, thread, postId, kind, labels))) await refresh();
	}

	const toggle = (label: EpistemicLabel) => {
		const labels = meta?.labels ?? [];
		save(meta?.kind ?? 'thought', labels.includes(label) ? labels.filter((l) => l !== label) : [...labels, label]);
	};
</script>

{#if meta}
	<span class="pt-kind">{postKindName(meta.kind)}</span>
	{#each meta.labels as l (l)}<span class="pt-label {l}">{labelName(l)}</span>{/each}
{/if}
<button class="pt-edit" class:on={open} title="Kind and labels" aria-label="Kind and labels" aria-expanded={open} onclick={() => (open = !open)}>
	<Tag size={12} />
</button>

{#if open}
	<div class="pt-panel">
		<div class="seg small" role="group" aria-label="Kind of post">
			{#each POST_KINDS as k (k.kind)}
				<button class:on={meta?.kind === k.kind} aria-pressed={meta?.kind === k.kind} onclick={() => save(k.kind, meta?.labels ?? [])}>{k.name}</button>
			{/each}
		</div>
		<div class="pt-labels">
			{#each LABELS as l (l.label)}
				<button class="pt-label {l.label}" class:off={!meta?.labels.includes(l.label)} aria-pressed={!!meta?.labels.includes(l.label)} title={l.hint} onclick={() => toggle(l.label)}>
					{l.name}
				</button>
			{/each}
		</div>
	</div>
{/if}
