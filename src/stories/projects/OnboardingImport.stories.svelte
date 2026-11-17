<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import OnboardingImport from '$lib/components/OnboardingImport.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Onboarding/OnboardingImport', component: OnboardingImport });
</script>

<Story name="Default" asChild>
	<OnboardingImport settings={fx.settings} projects={[]} semesters={[]} ondone={() => {}} onempty={() => {}} onback={() => {}} />
</Story>
