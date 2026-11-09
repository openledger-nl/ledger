<script lang="ts" module>
	import type { Component, Snippet } from 'svelte';

	export interface MenuItem {
		id: string;
		label: string;
		/** A second, quieter line under the label. */
		sub?: string;
		icon?: Component<{ size?: number; class?: string }>;
		/** A coloured dot before the label (the AI-use levels). */
		dot?: string;
		/** Items that have this set, true or false, are choices: the one that is true gets a check. */
		checked?: boolean;
		danger?: boolean;
		/** A rule above this item. */
		rule?: boolean;
	}
</script>

<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import { tick } from 'svelte';

	// A button that opens a short list: actions (a ⋯ menu) or a choice (who made a version). Like Dropdown, but the button is
	// yours and the list is placed on the screen, so a scrolling panel or a narrow column never clips it. Keyboard: Enter,
	// Space or the arrows open it; the arrows, Home and End move; Enter or Space picks; Escape or Tab closes.

	let {
		items,
		label,
		onpick,
		align = 'start',
		disabled = false,
		title,
		class: cls = '',
		children
	}: {
		items: MenuItem[];
		/** What the menu is for, read out by screen readers ("Made by v1.2"). */
		label: string;
		onpick: (id: string) => void;
		align?: 'start' | 'end';
		disabled?: boolean;
		/** The tooltip of the button; defaults to the label. */
		title?: string;
		class?: string;
		/** What the button shows. */
		children: Snippet;
	} = $props();

	const id = `menu-${Math.random().toString(36).slice(2, 9)}`;
	let open = $state(false);
	let active = $state(0);
	let place = $state<{ top: number; left: number } | null>(null);
	let root: HTMLElement | undefined = $state();
	let trigger: HTMLButtonElement | undefined = $state();
	let list: HTMLElement | undefined = $state();

	async function show() {
		active = Math.max(0, items.findIndex((i) => i.checked));
		place = null;
		open = true;
		await tick();
		position();
		await tick(); // placed, so no longer hidden: only then can it take the focus
		list?.focus({ preventScroll: true });
	}

	/** Below the button, or above it when there is no room; kept inside the window. */
	function position() {
		if (!trigger || !list) return;
		const r = trigger.getBoundingClientRect();
		const w = list.offsetWidth;
		const h = list.offsetHeight;
		const fitsBelow = r.bottom + 6 + h <= window.innerHeight - 8;
		const top = fitsBelow || r.top - 6 - h < 8 ? r.bottom + 6 : r.top - 6 - h;
		const left = align === 'end' ? r.right - w : r.left;
		place = { top, left: Math.max(8, Math.min(left, window.innerWidth - w - 8)) };
	}

	function close(refocus = true) {
		open = false;
		if (refocus) trigger?.focus();
	}

	function pick(i: number) {
		const picked = items[i];
		close();
		onpick(picked.id);
	}

	function move(to: number) {
		active = Math.min(items.length - 1, Math.max(0, to));
		list?.querySelector(`#${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
	}

	function onTriggerKey(e: KeyboardEvent) {
		if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
			e.preventDefault();
			void show();
		}
	}

	function onListKey(e: KeyboardEvent) {
		const keys: Record<string, () => void> = {
			ArrowDown: () => move(active + 1),
			ArrowUp: () => move(active - 1),
			Home: () => move(0),
			End: () => move(items.length - 1),
			Enter: () => pick(active),
			' ': () => pick(active),
			Escape: () => close()
		};
		if (keys[e.key]) {
			e.preventDefault();
			keys[e.key]();
		} else if (e.key === 'Tab') {
			close(false);
		}
	}

	// A click anywhere else closes it, and so does scrolling or resizing, which would leave it hanging in the wrong place.
	$effect(() => {
		if (!open) return;
		const away = (e: PointerEvent) => {
			if (root && !root.contains(e.target as Node)) close(false);
		};
		const drift = (e: Event) => {
			if (!list?.contains(e.target as Node)) close(false);
		};
		document.addEventListener('pointerdown', away);
		document.addEventListener('scroll', drift, true);
		window.addEventListener('resize', drift);
		return () => {
			document.removeEventListener('pointerdown', away);
			document.removeEventListener('scroll', drift, true);
			window.removeEventListener('resize', drift);
		};
	});
</script>

<span class="menu-host" bind:this={root}>
	<button
		bind:this={trigger}
		type="button"
		class={cls}
		{disabled}
		title={title ?? label}
		aria-label={label}
		aria-haspopup="menu"
		aria-expanded={open}
		onclick={() => (open ? close() : void show())}
		onkeydown={onTriggerKey}
	>
		{@render children()}
	</button>

	{#if open}
		<div
			bind:this={list}
			class="dd-menu pop"
			class:unplaced={!place}
			style={place ? `top: ${place.top}px; left: ${place.left}px` : undefined}
			role="menu"
			tabindex="-1"
			aria-label={label}
			aria-activedescendant="{id}-{active}"
			onkeydown={onListKey}
		>
			{#each items as item, i (item.id)}
				{#if item.rule}<div class="dd-rule" role="separator"></div>{/if}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					id="{id}-{i}"
					class="dd-option"
					class:active={i === active}
					class:selected={item.checked}
					class:danger={item.danger}
					role={item.checked === undefined ? 'menuitem' : 'menuitemradio'}
					aria-checked={item.checked}
					tabindex="-1"
					onpointerenter={() => (active = i)}
					onclick={() => pick(i)}
				>
					<span class="mi">
						{#if item.icon}<item.icon size={15} class="mi-ico" />{/if}
						{#if item.dot}<span class="mi-dot" style="--c: {item.dot}"></span>{/if}
						<span class="mi-text">
							<span class="dd-label">{item.label}</span>
							{#if item.sub}<span class="mi-sub">{item.sub}</span>{/if}
						</span>
					</span>
					{#if item.checked}<Check size={14} />{/if}
				</div>
			{/each}
		</div>
	{/if}
</span>
