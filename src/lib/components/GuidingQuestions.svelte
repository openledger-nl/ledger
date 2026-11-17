<script lang="ts">
	// A template's guiding questions, answered where the deliverable is started (the prompt dialog, the new deliverable
	// dialog) rather than in the thread. A question left blank is skipped; `answers` holds one text per question.
	import MarkdownEditor from './MarkdownEditor.svelte';

	let { questions, answers = $bindable(), template }: { questions: string[]; answers: string[]; template: string } = $props();

	const answered = $derived(answers.filter((a) => a.trim()).length);
</script>

<div class="field">
	<span class="gq-title">
		Guiding questions
		<span class="hint">
			{answered
				? `${answered} of ${questions.length} answered; a blank one is skipped`
				: `From "${template}". Answer what you can, or skip them: leave a question blank.`}
		</span>
		{#if answered}
			<button class="link accent gq-clear" onclick={() => (answers = questions.map(() => ''))}>Skip all</button>
		{/if}
	</span>
	<div class="gq-list">
		{#each questions as question, i (i)}
			<div class="gq">
				<span class="gq-q">{question}</span>
				<!-- Read through a function: the answers can lag a render behind a template that was just chosen. -->
				<MarkdownEditor bind:value={() => answers[i] ?? '', (v) => (answers[i] = v)} rows={2} label={question} placeholder="Your answer, or leave it blank to skip" />
			</div>
		{/each}
	</div>
</div>

<style>
	.gq-title {
		display: flex;
		align-items: baseline;
		gap: 8px;
		flex-wrap: wrap;
	}
	.gq-clear {
		margin-left: auto;
	}
	.gq-list {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.gq {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.gq-q {
		font-weight: 600;
		font-size: 13px;
	}
</style>
