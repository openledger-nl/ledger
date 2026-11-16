<script lang="ts" module>
	import type { Component } from 'svelte';

	export interface DropdownOption {
		value: string;
		label: string;
		/** Options that share a group are listed under its name; a new name starts a new section. */
		group?: string;
		/** A second line under the label, in the menu only. */
		hint?: string;
		/** A Lucide icon (or one drawn like one) before the label. */
		icon?: Component<{ size?: number }>;
		/** A picture before the label (an imported asset's URL), e.g. an MMM label. */
		image?: string;
		disabled?: boolean;
	}
</script>

<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Search from '@lucide/svelte/icons/search';
	import { tick, type Snippet } from 'svelte';

	// The app's one select: use it instead of <select> everywhere. A button that opens a list you can type in: typing
	// on it (open or not) filters the options by their label, hint and group, a long list (more than `searchFrom`
	// options) always shows the box. The arrows, Home and End move; Enter (or Space without a search) picks; Escape or
	// Tab closes.
	// The menu is placed against the window, not its parent, so a modal or a scrolling pane never cuts it off; it
	// opens upwards when there is no room below.

	let {
		value = $bindable(),
		options,
		label,
		align = 'start',
		placeholder = '',
		full = false,
		size = 'small',
		searchFrom = 7,
		disabled = false,
		footer
	}: {
		value: string;
		options: DropdownOption[];
		/** What it chooses, for screen readers (and the search box). */
		label: string;
		align?: 'start' | 'end';
		/** Shown when no option has the value. */
		placeholder?: string;
		/** As wide as its container, in the size of a form field: for forms rather than toolbars. */
		full?: boolean;
		size?: 'small' | 'field';
		searchFrom?: number;
		/** Shown as it is, but it can't be opened (loading, or busy saving). */
		disabled?: boolean;
		/** Under the options: a link or a word about them. */
		footer?: Snippet;
	} = $props();

	const id = `dd-${Math.random().toString(36).slice(2, 9)}`;
	let open = $state(false);
	let active = $state(0);
	let query = $state('');
	let place = $state({ left: 0, top: 0, width: 0, up: false, maxHeight: 320 });
	let root: HTMLElement | undefined = $state();
	let trigger: HTMLButtonElement | undefined = $state();
	let list: HTMLElement | undefined = $state();
	let search: HTMLInputElement | undefined = $state();

	// A value no option has shows the placeholder, or, without one, the first option (as a native select would).
	const current = $derived(options.find((o) => o.value === value) ?? (placeholder ? undefined : options[0]));
	/** The search box shows on a long list, and on any list as soon as something is typed. */
	const searchable = $derived(options.length > searchFrom || query !== '');
	const shown = $derived(
		query.trim() ? options.filter((o) => `${o.label} ${o.hint ?? ''} ${o.group ?? ''}`.toLowerCase().includes(query.trim().toLowerCase())) : options
	);
	const field = $derived(full || size === 'field');

	function measure() {
		if (!trigger) return;
		const box = trigger.getBoundingClientRect();
		const below = window.innerHeight - box.bottom - 12;
		const above = box.top - 12;
		const up = below < 220 && above > below;
		place = {
			left: align === 'end' ? box.right : box.left,
			top: up ? box.top - 6 : box.bottom + 6,
			width: box.width,
			up,
			maxHeight: Math.max(160, Math.min(360, up ? above : below))
		};
	}

	/** Opens the list; `typed` starts a search with what was typed on the closed button. */
	function show(typed = '') {
		query = typed;
		active = typed ? firstEnabled() : Math.max(0, shown.findIndex((o) => o.value === value));
		measure();
		open = true;
		void tick().then(() => {
			(searchable ? search : list)?.focus();
			list?.querySelector(`#${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
		});
	}

	const firstEnabled = () => Math.max(0, shown.findIndex((o) => !o.disabled));
	const printable = (e: KeyboardEvent) => e.key.length === 1 && e.key !== ' ' && !e.ctrlKey && !e.metaKey && !e.altKey;

	/** Typing in the box starts again at the first match. */
	function onSearch() {
		active = firstEnabled();
		list?.querySelector(`#${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
	}

	function close(refocus = true) {
		open = false;
		if (refocus) trigger?.focus();
	}

	function pick(i: number) {
		const o = shown[i];
		if (!o || o.disabled) return;
		value = o.value;
		close();
	}

	function move(to: number, step = 1) {
		if (!shown.length) return;
		let next = Math.min(shown.length - 1, Math.max(0, to));
		// Skip disabled options in the direction of travel.
		while (shown[next]?.disabled && next + step >= 0 && next + step < shown.length) next += step;
		active = next;
		list?.querySelector(`#${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
	}

	function onTriggerKey(e: KeyboardEvent) {
		if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
			e.preventDefault();
			show();
		} else if (printable(e)) {
			e.preventDefault();
			show(e.key);
		}
	}

	function onListKey(e: KeyboardEvent) {
		const keys: Record<string, () => void> = {
			ArrowDown: () => move(active + 1, 1),
			ArrowUp: () => move(active - 1, -1),
			Home: () => move(0, 1),
			End: () => move(shown.length - 1, -1),
			Enter: () => pick(active),
			Escape: () => close()
		};
		if (!searchable) keys[' '] = () => pick(active);
		if (keys[e.key]) {
			e.preventDefault();
			// Escape closes the menu only, not the dialog it sits in.
			if (e.key === 'Escape') e.stopPropagation();
			keys[e.key]();
		} else if (e.key === 'Tab') {
			close(false);
		} else if (!searchable && printable(e)) {
			// The first letter typed in a short list opens the box, with that letter in it.
			e.preventDefault();
			query = e.key;
			onSearch();
			void tick().then(() => search?.focus());
		}
	}

	// A click anywhere else closes it; scrolling or resizing the window moves it along.
	$effect(() => {
		if (!open) return;
		const away = (e: PointerEvent) => {
			const target = e.target as Node;
			if (root && !root.contains(target) && list && !list.contains(target)) close(false);
		};
		const follow = () => measure();
		document.addEventListener('pointerdown', away);
		window.addEventListener('resize', follow);
		window.addEventListener('scroll', follow, true);
		return () => {
			document.removeEventListener('pointerdown', away);
			window.removeEventListener('resize', follow);
			window.removeEventListener('scroll', follow, true);
		};
	});
</script>

{#snippet face(o: DropdownOption, big: boolean)}
	{#if o.image}<img class="dd-image" class:big src={o.image} alt="" />{:else if o.icon}<span class="dd-icon"><o.icon size={big ? 16 : 14} /></span>{/if}
{/snippet}

<div class="dd" class:full bind:this={root}>
	<button
		bind:this={trigger}
		type="button"
		class="dd-trigger"
		class:dd-field={field}
		class:open
		class:empty={!current}
		aria-haspopup="listbox"
		aria-expanded={open}
		{disabled}
		aria-label="{label}: {current?.label ?? placeholder}"
		onclick={() => (open ? close() : show())}
		onkeydown={onTriggerKey}
	>
		{#if current}{@render face(current, false)}{/if}
		<span class="dd-value">{current?.label ?? placeholder}</span>
		<ChevronDown size={14} class="dd-chevron" />
	</button>

	{#if open}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			bind:this={list}
			class="dd-menu"
			class:up={place.up}
			class:end={align === 'end'}
			style:left="{place.left}px"
			style:top="{place.top}px"
			style:min-width="{place.width}px"
			style:max-height="{place.maxHeight}px"
			onkeydown={onListKey}
		>
			{#if searchable}
				<label class="dd-search">
					<Search size={13} />
					<input bind:this={search} bind:value={query} oninput={onSearch} placeholder="Search {label.toLowerCase()}" aria-label="Search {label}" autocomplete="off" spellcheck="false" />
				</label>
			{/if}
			<div class="dd-options" role="listbox" tabindex="-1" aria-label={label} aria-activedescendant="{id}-{active}">
				{#each shown as o, i (o.value)}
					{#if o.group && o.group !== shown[i - 1]?.group}
						<div class="dd-group" role="presentation">{o.group}</div>
					{/if}
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<div
						id="{id}-{i}"
						class="dd-option"
						class:active={i === active}
						class:selected={o.value === value}
						class:disabled={o.disabled}
						role="option"
						aria-selected={o.value === value}
						aria-disabled={o.disabled}
						tabindex="-1"
						onpointerenter={() => !o.disabled && (active = i)}
						onclick={() => pick(i)}
					>
						{@render face(o, !!o.hint)}
						<span class="dd-text">
							<span class="dd-label">{o.label}</span>
							{#if o.hint}<span class="dd-hint">{o.hint}</span>{/if}
						</span>
						{#if o.value === value}<Check size={14} class="dd-check" />{/if}
					</div>
				{:else}
					<div class="dd-none">Nothing matches.</div>
				{/each}
			</div>
			{#if footer}<div class="dd-footer">{@render footer()}</div>{/if}
		</div>
	{/if}
</div>
