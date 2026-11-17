<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Onboarding from '$lib/components/Onboarding.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Onboarding/Onboarding', component: Onboarding });
</script>

<Story name="Halfway" asChild>
	<Onboarding projects={fx.projects} semesters={[fx.semester]} deliverableCount={3} settings={fx.settings} onboarding={fx.onboarding} />
</Story>

<Story name="A new vault" asChild>
	<Onboarding projects={[]} semesters={[]} deliverableCount={0} settings={fx.settings} onboarding={fx.onboarding} />
</Story>
