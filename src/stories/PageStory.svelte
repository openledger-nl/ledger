<script lang="ts">
	// A whole page of Ledger in Storybook: the route's own load functions run (root layout first, then the nested
	// layouts, then the page), each getting the data of the ones before as `parent()`, and answered by the sample
	// backend (backend.ts). Then the page is drawn inside its layouts, as SvelteKit would. The story's
	// `sveltekit_experimental` parameters give `$app/state` the same address, for the top bar and the page itself.
	import type { Component } from 'svelte';
	import { page as here } from '$app/state';

	type Load = (event: never) => unknown;
	type Shell = Component<{ data: Record<string, unknown>; children: import('svelte').Snippet }>;

	let {
		page: Page,
		layouts = [],
		loads,
		url,
		params = {}
	}: {
		page: Component<{ data: never }>;
		layouts?: Component<never>[];
		loads: (Load | undefined)[];
		url: string;
		params?: Record<string, string>;
	} = $props();

	async function run(): Promise<Record<string, unknown>> {
		let data: Record<string, unknown> = {};
		const address = new URL(url, 'http://localhost');
		for (const load of loads) {
			if (!load) continue;
			const parentData = data;
			const event = {
				params,
				url: address,
				route: { id: null },
				parent: async () => parentData,
				depends: () => {},
				untrack: <T,>(fn: () => T) => fn(),
				fetch,
				setHeaders: () => {}
			};
			const out = await (load as unknown as (e: typeof event) => unknown)(event);
			data = { ...data, ...((out as Record<string, unknown>) ?? {}) };
		}
		// Components that read `page.data` (the top bar, the settings panels) see it as in the app.
		(here as unknown as { data: unknown }).data = data;
		return data;
	}

	const shells = $derived(layouts as unknown as Shell[]);
	const Shown = $derived(Page as unknown as Component<{ data: Record<string, unknown> }>);
</script>

{#snippet nest(data: Record<string, unknown>, i: number)}
	{#if i < shells.length}
		{@const Layout = shells[i]}
		<Layout {data}>{@render nest(data, i + 1)}</Layout>
	{:else}
		<Shown {data} />
	{/if}
{/snippet}

{#await run()}
	<p class="hint" style="padding: 24px">Loading the page...</p>
{:then data}
	{@render nest(data, 0)}
{:catch e}
	<p class="notice" style="margin: 24px">This page can't be drawn in Storybook: {e instanceof Error ? e.message : String(e)}</p>
{/await}
