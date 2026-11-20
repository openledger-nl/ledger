<script lang="ts">
	// How the project's exported PDFs look and who they name: your name and your own side (a school or a company) with
	// its logo, the client with theirs, and one accent colour for the headings and rules. Kept in the project's
	// project.md, the logos in its branding/ folder. Each change is saved straight away.
	import ImagePlus from '@lucide/svelte/icons/image-plus';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import X from '@lucide/svelte/icons/x';
	import { open } from '@tauri-apps/plugin-dialog';
	import { api } from '$lib/api';
	import Dropdown from '$lib/components/Dropdown.svelte';
	import FileImage from '$lib/components/FileImage.svelte';
	import { attempt, toast } from '$lib/ui.svelte';
	import type { Branding, BrandingSide } from '$lib/types';

	let {
		project,
		others = [],
		client = null,
		study = false
	}: {
		project: string;
		/** The other projects, to take your side from: slug and name. */
		others?: { slug: string; name: string }[];
		/** Who the PDF says it is for when no name is given here: the project's client. */
		client?: string | null;
		/** A study project shows the AI-use label on the cover unless it is turned off. */
		study?: boolean;
	} = $props();

	/** The PDF's own colour, when the project has none: the orange of the report layout. */
	const DEFAULT_ACCENT = '#e07a3f';
	const LOGO_TYPES = ['png', 'jpg', 'jpeg', 'svg', 'webp'];

	let branding = $state<Branding | null>(null);
	let busy = $state(false);
	/** Counts logo changes, so a new picture under the same name is shown. */
	let logoVersion = $state(0);

	$effect(() => {
		const slug = project;
		branding = null;
		void api.getProjectBranding(slug).then(
			(b) => slug === project && (branding = b),
			() => slug === project && (branding = emptyBranding())
		);
	});

	function emptyBranding(): Branding {
		return { author: null, accent: null, ownName: null, ownLogo: null, clientName: null, clientLogo: null, showLabel: null };
	}

	async function save(change: Partial<Branding>) {
		if (!branding) return;
		busy = true;
		const saved = await attempt(() => api.saveProjectBranding(project, { ...branding!, ...change }));
		busy = false;
		if (saved) branding = saved;
	}

	const text = (e: Event) => (e.currentTarget as HTMLInputElement).value.trim() || null;

	async function chooseLogo(side: BrandingSide) {
		const chosen = await attempt(() =>
			open({
				multiple: false,
				directory: false,
				title: side === 'own' ? 'Your logo' : "The client's logo",
				filters: [{ name: 'Pictures', extensions: LOGO_TYPES }]
			})
		);
		if (typeof chosen !== 'string') return;
		await setLogo(side, chosen);
	}

	async function setLogo(side: BrandingSide, source: string | null) {
		busy = true;
		const saved = await attempt(() => api.setProjectLogo(project, side, source));
		busy = false;
		if (saved) {
			branding = saved;
			logoVersion++;
		}
	}

	async function copyFrom(from: string) {
		if (!from) return;
		busy = true;
		const saved = await attempt(() => api.copyProjectBranding(project, from));
		busy = false;
		if (saved) {
			branding = saved;
			logoVersion++;
			toast(`Took your side from ${others.find((o) => o.slug === from)?.name ?? from}`);
		}
	}

	const accent = $derived(branding?.accent ?? DEFAULT_ACCENT);
	const showLabel = $derived(branding?.showLabel ?? study);
</script>

{#snippet logo(side: BrandingSide, path: string | null, label: string)}
	<div class="br-logo">
		<div class="br-logo-box">
			{#if path}
				{#key logoVersion}<FileImage {project} {path} alt={label} cls="br-logo-img" />{/key}
			{:else}
				<span class="hint">No logo</span>
			{/if}
		</div>
		<div class="br-logo-actions">
			<button class="btn small" onclick={() => chooseLogo(side)} disabled={busy || !branding}><ImagePlus size={14} />{path ? 'Change logo' : 'Choose logo'}</button>
			{#if path}
				<button class="icon-btn" aria-label="Remove {label}" title="Remove the logo" onclick={() => setLogo(side, null)} disabled={busy}><X size={15} /></button>
			{/if}
		</div>
	</div>
{/snippet}

<section class="panel" aria-label="On exported PDFs">
	<div class="panel-head">
		<h2>On exported PDFs</h2>
	</div>
	<p class="sub" style="margin:0">
		A deliverable exported as a PDF has a cover that says who it is for and who made it, each with a logo, and its headings and rules take the colour
		below. Kept in the project's <code>project.md</code>, the logos in its <code>branding/</code> folder.
	</p>

	{#if branding}
		<div class="br-sides">
			<div class="br-side">
				<h3>You</h3>
				<label class="field">
					Your name
					<input value={branding.author ?? ''} placeholder="e.g. Sam Student" onchange={(e) => save({ author: text(e) })} disabled={busy} />
				</label>
				<label class="field">
					Your school or company
					<input value={branding.ownName ?? ''} placeholder="e.g. Fontys ICT" onchange={(e) => save({ ownName: text(e) })} disabled={busy} />
				</label>
				{@render logo('own', branding.ownLogo, 'your logo')}
			</div>
			<div class="br-side">
				<h3>Client</h3>
				<label class="field">
					Who it is for
					<input
						value={branding.clientName ?? ''}
						placeholder={client ? `${client} (the project's client)` : 'e.g. Police'}
						onchange={(e) => save({ clientName: text(e) })}
						disabled={busy}
					/>
				</label>
				{@render logo('client', branding.clientLogo, "the client's logo")}
			</div>
		</div>

		<div class="field">
			<label class="check">
				<input type="checkbox" checked={showLabel} onchange={(e) => save({ showLabel: (e.currentTarget as HTMLInputElement).checked })} disabled={busy} />
				Show the AI-use label at the bottom left of the cover
			</label>
			<span class="hint">
				The Me & My Machine label of the version exported (a draft shows the one it is meant to get). {study
					? 'On by default in a study project.'
					: 'Off by default outside a study project.'}
			</span>
		</div>

		<div class="form-row">
			<label class="field">
				Colour
				<span class="br-colour">
					<input type="color" value={accent} onchange={(e) => save({ accent: (e.currentTarget as HTMLInputElement).value })} disabled={busy} aria-label="Colour" />
					<code>{accent}</code>
					{#if branding.accent}
						<button class="icon-btn" title="Back to the standard colour" aria-label="Back to the standard colour" onclick={() => save({ accent: null })} disabled={busy}>
							<RotateCcw size={14} />
						</button>
					{/if}
				</span>
			</label>
			{#if others.length}
				<div class="field grow">
					Take your side from another project
					<Dropdown
						bind:value={() => '', (v) => void copyFrom(v)}
						options={others.map((o) => ({ value: o.slug, label: o.name }))}
						placeholder="Your name, logo and colour from..."
						label="Take your side from another project"
						disabled={busy}
						full
					/>
				</div>
			{/if}
		</div>
	{:else}
		<span class="hint">Loading...</span>
	{/if}
</section>

<style>
	.br-sides {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
		gap: 16px;
	}
	.br-side {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px 14px;
		border: 1px solid var(--line);
		border-radius: 10px;
	}
	.br-side h3 {
		margin: 0;
		font-size: 13px;
	}
	.br-logo {
		display: flex;
		align-items: center;
		gap: 12px;
		flex-wrap: wrap;
	}
	.br-logo-box {
		width: 150px;
		height: 56px;
		display: grid;
		place-items: center;
		border: 1px dashed var(--line);
		border-radius: 8px;
		background: var(--field);
		overflow: hidden;
	}
	.br-logo-box :global(.br-logo-img) {
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
	}
	.br-logo-actions {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.br-colour {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.br-colour input[type='color'] {
		width: 40px;
		height: 30px;
		padding: 2px;
		border: 1px solid var(--line);
		border-radius: 6px;
		background: var(--field);
		cursor: pointer;
	}
</style>
