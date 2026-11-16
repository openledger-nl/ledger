<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import DocSide from '$lib/components/DocSide.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Deliverables/DocSide', component: DocSide });
</script>

<Story name="Beside a document" asChild>
	<div style="max-width: 440px; height: 720px; display: flex">
		<DocSide
			project={fx.PROJECT}
			doc={fx.deliverable}
			provenance={fx.provenance}
			draft={null}
			prompts={[fx.prompt]}
			threads={fx.threads}
			thread={fx.THREAD}
			threadTitle="Sensor research"
			lost={new Set()}
			here={() => true}
			onlocate={() => {}}
		/>
	</div>
</Story>
