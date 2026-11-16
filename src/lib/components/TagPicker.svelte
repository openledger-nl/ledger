<script lang="ts" module>
	export interface TagOption {
		/** The tag itself, without `#`: `project/thrym-player` is passed as `thrym-player` for projects, `coach/frank` for people. */
		value: string;
		/** How it reads: the project's name, the person's name. */
		label: string;
		/** A second line: the tag as written, how often it is used. */
		hint?: string;
	}

	/** A button beside the chips that adds several tags at once: "Whole team" adds every teammate of the projects. */
	export interface TagQuickPick {
		label: string;
		tags: string[];
		/** Said on hover: who it adds. */
		title?: string;
	}
</script>

<script lang="ts">
	// Tags as chips, picked from the tags already in the vault (the index) in a menu like Dropdown's: type to search,
	// the arrows and Enter pick, and something new can be added as a tag of its own. Used for every `#` tag: the
	// projects and people of a meeting, who gave feedback. With `max={1}` it holds one tag. `quick` adds buttons that put
	// several tags in at once (the whole team); one whose tags are all in already is left out.
	import Plus from '@lucide/svelte/icons/plus';
	import Search from '@lucide/svelte/icons/search';
	import Users from '@lucide/svelte/icons/users';
	import X from '@lucide/svelte/icons/x';
	import { tick } from 'svelte';
	import { cleanTag } from '$lib/v1';

	let {
		value = $bindable([]),
		options,
		label,
		placeholder = 'Add',
		max = Infinity,
		allowNew = true,
		prefix = '',
		quick = []
	}: {
		value?: string[];
		/** The indexed tags to pick from. */
		options: TagOption[];
		label: string;
		placeholder?: string;
		max?: number;
		/** Whether something typed that isn't in the index can be added as a new tag. */
		allowNew?: boolean;
		/** Written before the tag where it is shown as a tag: `project/` for projects. */
		prefix?: string;
		quick?: TagQuickPick[];
	} = $props();


	const id = `tp-${Math.random().toString(36).slice(2, 9)}`;
	let open = $state(false);
	let query = $state('');
	let active = $state(0);
	let place = $state({ left: 0, top: 0, width: 0, up: false, maxHeight: 320 });
	let root: HTMLElement | undefined = $state();
	let menu: HTMLElement | undefined = $state();
	let search: HTMLInputElement | undefined = $state();

	const known = $derived(new Map(options.map((o) => [o.value, o])));
	const full = $derived(value.length >= max);
	const quickShown = $derived(full ? [] : quick.filter((q) => q.tags.some((t) => !value.includes(t))));
	function addAll(tags: string[]) {
		const next = [...value, ...tags.filter((t) => !value.includes(t))];
		value = next.slice(0, max === Infinity ? next.length : max);
	}
	const typed = $derived(cleanTag(query.replace(prefix ? new RegExp(`^#?${prefix}`) : /^$/, '')));

	/** What the menu lists: the matching tags not picked yet, and the typed one as a new tag when it is new. */
	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase().replace(/^#/, '');
		const free = options.filter((o) => !value.includes(o.value));
		const matching = q ? free.filter((o) => `${o.label} ${prefix}${o.value} ${o.hint ?? ''}`.toLowerCase().includes(q)) : free;
		const items = matching.map((o) => ({ ...o, isNew: false }));
		if (allowNew && typed && !known.has(typed) && !value.includes(typed)) {
			items.push({ value: typed, label: `Add "${query.trim().replace(/^#/, '') || typed}"`, isNew: true });
		}
		return items;
	});

	function measure() {
		if (!root) return;
		const box = root.getBoundingClientRect();
		const below = window.innerHeight - box.bottom - 12;
		const above = box.top - 12;
		const up = below < 220 && above > below;
		place = { left: box.left, top: up ? box.top - 6 : box.bottom + 6, width: box.width, up, maxHeight: Math.max(160, Math.min(340, up ? above : below)) };
	}

	function show() {
		if (full) return;
		query = '';
		active = 0;
		measure();
		open = true;
		void tick().then(() => search?.focus());
	}

	function close() {
		open = false;
		query = '';
	}

	function pick(i: number) {
		const o = shown[i];
		if (!o) return;
		value = [...value, o.value];
		query = '';
		active = 0;
		if (value.length >= max) close();
		else void tick().then(() => (measure(), search?.focus()));
	}

	const remove = (tag: string) => (value = value.filter((t) => t !== tag));

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			active = Math.min(shown.length - 1, active + 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			active = Math.max(0, active - 1);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			pick(active);
		} else if (e.key === 'Escape') {
			// Closes the menu only, not the dialog it sits in.
			e.preventDefault();
			e.stopPropagation();
			close();
		} else if (e.key === 'Backspace' && !query && value.length) {
			remove(value[value.length - 1]);
		} else if (e.key === 'Tab') {
			close();
		}
		menu?.querySelector(`#${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
	}

	// A click anywhere else closes it; scrolling or resizing moves it along.
	$effect(() => {
		if (!open) return;
		const away = (e: PointerEvent) => {
			const t = e.target as Node;
			if (root && !root.contains(t) && menu && !menu.contains(t)) close();
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

<div class="tag-picker" class:open bind:this={root}>
	{#each value as tag (tag)}
		<span class="chip plain">
			{known.get(tag)?.label ?? tag}
			<button type="button" class="x" aria-label="Remove {known.get(tag)?.label ?? tag}" onclick={() => remove(tag)}><X size={12} /></button>
		</span>
	{/each}
	{#if !full}
		<button type="button" class="tp-add" aria-haspopup="listbox" aria-expanded={open} aria-label="{label}: add" onclick={() => (open ? close() : show())}>
			<Plus size={13} />{value.length ? '' : placeholder}
		</button>
	{/if}
	{#each quickShown as q (q.label)}
		<button type="button" class="tp-add tp-quick" title={q.title} onclick={() => addAll(q.tags)}><Users size={13} />{q.label}</button>
	{/each}

	{#if open}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			bind:this={menu}
			class="dd-menu"
			class:up={place.up}
			style:left="{place.left}px"
			style:top="{place.top}px"
			style:min-width="{Math.max(260, place.width)}px"
			style:max-height="{place.maxHeight}px"
			{onkeydown}
		>
			<label class="dd-search">
				<Search size={13} />
				<input
					bind:this={search}
					bind:value={query}
					oninput={() => (active = 0)}
					placeholder="Search, or type a new one"
					aria-label="Search {label.toLowerCase()}"
					autocomplete="off"
					spellcheck="false"
				/>
			</label>
			<div class="dd-options" role="listbox" tabindex="-1" aria-label={label} aria-activedescendant="{id}-{active}">
				{#each shown as o, i (o.value + (o.isNew ? '+' : ''))}
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<div
						id="{id}-{i}"
						class="dd-option"
						class:active={i === active}
						role="option"
						aria-selected="false"
						tabindex="-1"
						onpointerenter={() => (active = i)}
						onclick={() => pick(i)}
					>
						{#if o.isNew}<span class="dd-icon"><Plus size={14} /></span>{/if}
						<span class="dd-text">
							<span class="dd-label">{o.label}</span>
							{#if o.hint}<span class="dd-hint">{o.hint}</span>{/if}
						</span>
					</div>
				{:else}
					<div class="dd-none">{options.length ? 'Nothing matches.' : 'Nothing yet: type a name.'}</div>
				{/each}
			</div>
		</div>
	{/if}
</div>
