<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import DocHistory from '$lib/components/DocHistory.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Deliverables/DocHistory', component: DocHistory });
</script>

<Story name="What Claude Code changed" asChild>
	<div style="max-width: 760px">
		<DocHistory project={fx.PROJECT} path="sensor-research.md" view="changes" versions={fx.historyVersions} hasDraft from="1.0" to="1.1" focus="" />
	</div>
</Story>

<Story name="What you changed since" asChild>
	<div style="max-width: 760px">
		<DocHistory project={fx.PROJECT} path="sensor-research.md" view="changes" versions={fx.historyVersions} hasDraft from="1.1" to="live" focus="" />
	</div>
</Story>

<Story name="Who wrote what" asChild>
	<div style="max-width: 760px">
		<DocHistory project={fx.PROJECT} path="sensor-research.md" view="authors" versions={fx.historyVersions} hasDraft from="" to="live" focus="" />
	</div>
</Story>

<Story name="Where the AI's words went" asChild>
	<div style="max-width: 760px">
		<DocHistory project={fx.PROJECT} path="sensor-research.md" view="authors" versions={fx.historyVersions} hasDraft from="" to="live" focus="1.1" />
	</div>
</Story>
