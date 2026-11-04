<script lang="ts">
	// Gamma: the HBO-i matrix of one semester, from Portflow: activities
	// across, architecture layers down, per cell the evidence that counts for it, coloured by the highest level that has
	// evidence. Goals measured in levels only (PS, PL) stand beside it. Clicking a cell or a level shows what is behind it.
	// With intended development set, the rows aimed for are marked (and the others dimmed, still there), each cell says
	// whether it got there, and the detail shows only the intended level's description; the other levels fold away.
	import FilePlus from '@lucide/svelte/icons/file-plus';
	import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
	import { page } from '$app/state';
	import { deliverableForLevelFlow, threadForLevelFlow, threadUrl, projectUrl } from '$lib/actions';
	import Markdown from '$lib/components/Markdown.svelte';
	import { plural } from '$lib/format';
	import type { Gamma, GammaCell, GammaLevel, GammaSkill, EvidenceRef, ProjectSummary } from '$lib/types';

	let { view }: { view: Gamma } = $props();

	type Picked = { title: string; levels: GammaLevel[]; target: number | null };
	let picked = $state<string | null>(null);
	let showOthers = $state(false);

	// Where a thread or deliverable started from a level goes: a project linked to this semester first, then the rest.
	const projects = $derived.by(() => {
		const all = (page.data.projects ?? []) as ProjectSummary[];
		const linked = new Set(view.linkedProjects);
		return [...all.filter((p) => linked.has(p.slug)), ...all.filter((p) => !linked.has(p.slug))];
	});
	let chosenProject = $state('');
	const project = $derived(projects.some((p) => p.slug === chosenProject) ? chosenProject : (projects[0]?.slug ?? ''));
	const aiming = $derived(view.rows.some((r) => r.target) || view.skills.some((s) => s.target));

	const cellAt = $derived(new Map(view.cells.map((c) => [`${c.layer}:${c.outcome}`, c])));
	const columnName = $derived(new Map(view.columns.map((c) => [c.outcome, c.name])));
	const rowName = $derived(new Map(view.rows.map((r) => [r.layer, r.name])));
	const levels = $derived(Array.from({ length: Math.max(view.maxLevel, 1) }, (_, i) => i + 1));

	const detail = $derived.by((): Picked | null => {
		if (!picked) return null;
		const [kind, a, b] = picked.split(':');
		if (kind === 'cell') {
			const cell = cellAt.get(`${a}:${b}`);
			return cell ? { title: `${columnName.get(cell.outcome)} · ${rowName.get(cell.layer)}`, levels: cell.levels, target: cell.target } : null;
		}
		const skill = view.skills.find((s) => String(s.outcome) === a);
		return skill ? { title: skill.name, levels: skill.levels, target: skill.target } : null;
	});
	// With a level aimed for, that one is shown in full and the others fold away.
	const focus = $derived(detail?.target ? detail.levels.filter((l) => l.level === detail.target) : (detail?.levels ?? []));
	const others = $derived(detail?.target ? detail.levels.filter((l) => l.level !== detail.target) : []);

	// Darker with every level: a share of the accent, so it follows the theme.
	const shade = (level: number | null) => (level ? Math.round(22 + (78 * level) / Math.max(view.maxLevel, 1)) : 0);
	const strong = (level: number | null) => shade(level) >= 70;
	const pick = (key: string) => {
		picked = picked === key ? null : key;
		showOthers = false;
	};
	const cellKey = (c: GammaCell) => `cell:${c.layer}:${c.outcome}`;
	const skillKey = (s: GammaSkill) => `skill:${s.outcome}`;
	const evidenceHref = (e: EvidenceRef) => (e.thread ? threadUrl(e.project, e.thread) : projectUrl(e.project));
	const cellLabel = (c: GammaCell) =>
		`${columnName.get(c.outcome)}, ${rowName.get(c.layer)}: ${plural(c.evidenceCount, 'evidence item', 'evidence items')}${c.level ? `, up to level ${c.level}` : ''}${
			c.target ? ` · aiming for level ${c.target}${c.reached ? ', reached' : ', not yet'}` : ''
		}`;
</script>

<div class="gamma">
	<div class="matrix-wrap">
		<table class="matrix">
			<thead>
				<tr>
					<th></th>
					{#each view.columns as col (col.outcome)}<th scope="col">{col.name}</th>{/each}
				</tr>
			</thead>
			<tbody>
				{#each view.rows as row (row.layer)}
					<tr class:dim={aiming && !row.target}>
						<th scope="row">
							<span class="layer-letter">{row.layer}</span>{row.name}
							{#if row.target}<span class="aim" title="Intended: level {row.target}">→ {row.target}</span>{/if}
						</th>
						{#each view.columns as col (col.outcome)}
							{@const cell = cellAt.get(`${row.layer}:${col.outcome}`)}
							<td>
								{#if cell}
									<button
										class="cell"
										class:empty={!cell.evidenceCount}
										class:strong={strong(cell.level)}
										class:picked={picked === cellKey(cell)}
										class:aimed={cell.target}
										class:reached={cell.reached}
										style:--shade="{shade(cell.level)}%"
										aria-label={cellLabel(cell)}
										title={cellLabel(cell)}
										onclick={() => pick(cellKey(cell))}
									>
										{cell.evidenceCount || ''}
										{#if cell.reached}<span class="tick" aria-hidden="true">✓</span>{/if}
										{#if cell.feedbackCount}<span class="fb" title="{plural(cell.feedbackCount, 'piece', 'pieces')} of feedback"></span>{/if}
									</button>
								{:else}
									<span class="cell none"></span>
								{/if}
							</td>
						{/each}
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	<div class="side">
		<div class="legend">
			{#each levels as l (l)}
				<span class="legend-item"><span class="swatch" style:--shade="{shade(l)}%"></span>Level {l}</span>
			{/each}
			<span class="legend-item"><span class="swatch blank"></span>No evidence yet</span>
		</div>

		{#if view.skills.length}
			<div class="skills">
				{#each view.skills as skill (skill.outcome)}
					<button class="skill" class:picked={picked === skillKey(skill)} class:dim={aiming && !skill.target} onclick={() => pick(skillKey(skill))} title={skill.name}>
						<span class="bars">
							{#each [...skill.levels].reverse() as l (l.id)}
								<span class="bar" class:filled={l.evidenceCount > 0} class:aim-bar={l.level === skill.target} style:--shade="{shade(l.level)}%" title="{l.name}: {plural(l.evidenceCount, 'evidence item', 'evidence items')}"></span>
							{/each}
						</span>
						<span class="skill-name">{skill.nickname || skill.name}{#if skill.target}<span class="aim">→ {skill.target}</span>{/if}</span>
						<span class="hint">{skill.evidenceCount}</span>
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>

{#snippet level(l: GammaLevel, open: boolean)}
	<div class="level-row">
		<div class="level-head">
			<span class="level-badge" class:strong={strong(l.evidenceCount ? l.level : null)} style:--shade="{shade(l.evidenceCount ? l.level : null)}%">{l.level}</span>
			<strong>{l.name}</strong>
			<span class="hint">
				{l.evidenceCount ? plural(l.evidenceCount, 'evidence item', 'evidence items') : 'no evidence yet'}{l.feedbackCount
					? ` · ${plural(l.feedbackCount, 'piece', 'pieces')} of feedback`
					: ''}
			</span>
		</div>
		{#if l.evidence.length}
			<div class="chips">
				{#each l.evidence as e (e.project + e.slug)}<a class="chip" href={evidenceHref(e)} title={e.projectName}>{e.name}</a>{/each}
				{#if l.evidenceCount > l.evidence.length}<span class="hint">and {l.evidenceCount - l.evidence.length} more in Portflow</span>{/if}
			</div>
		{:else if l.evidenceCount}
			<div class="hint">In Portflow, in a collection not linked to a project in Ledger.</div>
		{/if}
		{#if project}
			<div class="level-actions">
				<button class="btn small" onclick={() => threadForLevelFlow(project, l)}><MessageSquarePlus size={14} />Start a thread</button>
				<button class="btn small" onclick={() => deliverableForLevelFlow(project, l)}><FilePlus size={14} />New deliverable</button>
			</div>
		{/if}
		{#if l.descriptionHtml}
			<details {open}>
				<summary class="hint">What this level asks</summary>
				<Markdown html={l.descriptionHtml} />
			</details>
		{/if}
	</div>
{/snippet}

{#if detail}
	<section class="card detail">
		<div class="detail-head">
			<h2>{detail.title}</h2>
			<span class="spacer-h"></span>
			{#if projects.length}
				<label class="project-pick">
					Start in
					<select value={project} onchange={(e) => (chosenProject = e.currentTarget.value)}>
						{#each projects as p (p.slug)}<option value={p.slug}>{p.name}{view.linkedProjects.includes(p.slug) ? ' · this semester' : ''}</option>{/each}
					</select>
				</label>
			{/if}
			<button class="btn small" onclick={() => (picked = null)}>Close</button>
		</div>
		{#if detail.target}<p class="hint aim-note">You aim for level {detail.target} here{others.length ? '; the other levels are folded away' : ''}.</p>{/if}
		{#each focus as l (l.id)}
			{@render level(l, !!detail.target)}
		{/each}
		{#if others.length}
			<button class="link others-toggle" onclick={() => (showOthers = !showOthers)}>{showOthers ? 'Hide' : 'Show'} the other levels ({others.length})</button>
			{#if showOthers}
				{#each others as l (l.id)}
					{@render level(l, false)}
				{/each}
			{:else}
				<div class="others-compact">
					{#each others as l (l.id)}
						<span class="hint">{l.name}: {l.evidenceCount ? plural(l.evidenceCount, 'evidence item', 'evidence items') : 'no evidence'}</span>
					{/each}
				</div>
			{/if}
		{/if}
	</section>
{/if}

<style>
	.gamma {
		display: flex;
		gap: 24px;
		align-items: flex-start;
		flex-wrap: wrap;
	}
	.matrix-wrap {
		overflow-x: auto;
		max-width: 100%;
	}
	.matrix {
		border-collapse: separate;
		border-spacing: 4px;
		font-size: 13px;
	}
	.matrix thead th {
		font-weight: 600;
		color: var(--muted);
		padding: 0 4px 4px;
		text-align: center;
		min-width: 84px;
		vertical-align: bottom;
	}
	.matrix tbody th {
		text-align: left;
		font-weight: 500;
		padding-right: 10px;
		white-space: nowrap;
	}
	.layer-letter {
		display: inline-block;
		width: 18px;
		margin-right: 6px;
		color: var(--faint);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.matrix td {
		padding: 0;
	}
	.cell {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 38px;
		border: 1px solid var(--line);
		border-radius: 8px;
		background: color-mix(in srgb, var(--accent) var(--shade, 0%), var(--card));
		color: var(--ink);
		font-weight: 600;
		font-variant-numeric: tabular-nums;
		cursor: pointer;
		transition: transform 0.06s ease;
	}
	button.cell:hover {
		transform: translateY(-1px);
		border-color: var(--accent);
	}
	.cell.empty {
		background: var(--card-2);
		color: var(--faint);
	}
	.cell.none {
		background: transparent;
		border-style: dashed;
		cursor: default;
	}
	.cell.strong,
	.level-badge.strong {
		color: var(--accent-ink);
	}
	.cell.picked,
	.skill.picked {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}
	.dim {
		opacity: 0.5;
	}
	.aim {
		margin-left: 6px;
		padding: 0 5px;
		border-radius: 5px;
		background: var(--accent-soft);
		color: var(--accent);
		font-size: 11px;
		font-weight: 700;
	}
	.cell.aimed {
		box-shadow: inset 0 0 0 2px var(--accent);
	}
	.cell.reached {
		box-shadow: inset 0 0 0 2px var(--green);
	}
	.tick {
		position: absolute;
		bottom: 2px;
		right: 5px;
		font-size: 10px;
		color: var(--green);
	}
	.cell.strong .tick {
		color: var(--accent-ink);
	}
	.bar.aim-bar {
		box-shadow: 0 0 0 2px var(--accent);
	}
	.aim-note {
		margin: 0;
	}
	.others-toggle {
		align-self: flex-start;
		background: none;
		border: 0;
		padding: 0;
		color: var(--accent);
		cursor: pointer;
		font-size: 12.5px;
	}
	.others-compact {
		display: flex;
		gap: 4px 16px;
		flex-wrap: wrap;
	}
	.fb {
		position: absolute;
		top: 5px;
		right: 5px;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--amber);
	}
	.side {
		display: flex;
		flex-direction: column;
		gap: 18px;
		padding-top: 24px;
	}
	.legend {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 12.5px;
		color: var(--muted);
	}
	.legend-item {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.swatch {
		width: 26px;
		height: 14px;
		border-radius: 4px;
		background: color-mix(in srgb, var(--accent) var(--shade, 0%), var(--card));
		border: 1px solid var(--line);
	}
	.swatch.blank {
		background: var(--card-2);
	}
	.skills {
		display: flex;
		gap: 10px;
	}
	.skill {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		padding: 6px;
		border: 1px solid transparent;
		border-radius: 8px;
		background: none;
		color: var(--ink);
		cursor: pointer;
		font-size: 12px;
	}
	.skill:hover {
		border-color: var(--line);
	}
	.bars {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}
	.bar {
		width: 34px;
		height: 16px;
		border-radius: 4px;
		background: var(--card-2);
		border: 1px solid var(--line);
	}
	.bar.filled {
		background: color-mix(in srgb, var(--accent) var(--shade, 0%), var(--card));
	}
	.skill-name {
		font-weight: 600;
	}
	.detail {
		margin-top: 18px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.detail-head {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.project-pick {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 12.5px;
		color: var(--muted);
	}
	.project-pick select {
		font-size: 12.5px;
		max-width: 220px;
	}
	.level-actions {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.detail-head h2 {
		margin: 0;
		font-size: 15px;
	}
	.level-row {
		border-top: 1px solid var(--line);
		padding-top: 8px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.level-head {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 13px;
		flex-wrap: wrap;
	}
	.level-badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		border-radius: 6px;
		font-size: 12px;
		font-weight: 700;
		border: 1px solid var(--line);
		background: color-mix(in srgb, var(--accent) var(--shade, 0%), var(--card-2));
	}
	summary {
		cursor: pointer;
	}
</style>
