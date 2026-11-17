<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import MeetingNoteEditor from '$lib/components/MeetingNoteEditor.svelte';
	import * as fx from '../fixtures';

	const { Story } = defineMeta({ title: 'Meetings/MeetingNoteEditor', component: MeetingNoteEditor });
</script>

<!-- Saves as you type, to the sample backend. -->
<Story name="Two meetings" asChild>
	<MeetingNoteEditor date="2026-11-16" note={fx.dailyNote} projectOptions={fx.projectOptions} peopleOptions={fx.peopleOptions} people={fx.people} />
</Story>

<Story name="A day without meetings" asChild>
	<MeetingNoteEditor date="2026-11-17" note={null} projectOptions={fx.projectOptions} peopleOptions={fx.peopleOptions} people={fx.people} />
</Story>
