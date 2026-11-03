<script lang="ts">
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { tick } from 'svelte';
	import { LOCALE } from '$lib/format';
	import { addDays, european, parseDay, parseEuropean, toDay, today } from '$lib/semesters';

	// A date the European way: typed as DD-MM-YYYY ("1-9-2026", "01/09/26" work too) or picked from a calendar that
	// starts on Monday. `value` is the stored calendar day, "2026-09-01". In the calendar the arrows move a day or
	// a week, Page Up and Page Down a month, Enter picks and Escape closes. `range` shades a stretch (the semester).

	let {
		value = $bindable(),
		label,
		min = '',
		max = '',
		range = null
	}: { value: string; label: string; min?: string; max?: string; range?: { start: string; end: string } | null } = $props();

	let text = $state('');
	let invalid = $state(false);
	let open = $state(false);
	let month = $state(new Date());
	let focused = $state('');
	/** Where the calendar floats: fixed to the window, so a scrolling dialog can't clip it. */
	let place = $state({ top: 0, left: 0 });
	let root: HTMLElement | undefined = $state();
	let grid: HTMLElement | undefined = $state();

	// Follows the value whenever it changes from outside (or is picked), but not while it is being typed.
	let typing = false;
	$effect(() => {
		const v = value;
		if (!typing) {
			text = european(v);
			invalid = false;
		}
	});

	function oninput() {
		typing = true;
		const day = parseEuropean(text);
		invalid = !day && text.trim() !== '';
		if (day && allowed(day)) value = day;
	}

	function commit() {
		typing = false;
		const day = parseEuropean(text);
		if (day && allowed(day)) value = day;
		text = european(value);
		invalid = false;
	}

	const allowed = (day: string) => (!min || day >= min) && (!max || day <= max);

	const monthTitle = new Intl.DateTimeFormat(LOCALE, { month: 'long', year: 'numeric' });
	const dayName = new Intl.DateTimeFormat(LOCALE, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
	const WEEKDAYS = ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su'];

	/** Six weeks from the Monday on or before the first of the month. */
	const days = $derived.by(() => {
		const first = new Date(month.getFullYear(), month.getMonth(), 1);
		const start = addDays(first, -((first.getDay() + 6) % 7));
		return Array.from({ length: 42 }, (_, i) => {
			const d = addDays(start, i);
			return { day: toDay(d), date: d, inMonth: d.getMonth() === month.getMonth() };
		});
	});

	async function show() {
		const d = parseDay(value) ?? new Date();
		month = new Date(d.getFullYear(), d.getMonth(), 1);
		focused = toDay(d);
		if (root) {
			const r = root.getBoundingClientRect();
			const height = 330;
			const below = r.bottom + 6 + height <= window.innerHeight;
			place = { top: below ? r.bottom + 6 : Math.max(8, r.top - 6 - height), left: Math.max(8, Math.min(r.left, window.innerWidth - 268)) };
		}
		open = true;
		await tick();
		focusDay();
	}

	function close(refocus = true) {
		open = false;
		if (refocus) root?.querySelector<HTMLInputElement>('input')?.focus();
	}

	function pick(day: string) {
		if (!allowed(day)) return;
		typing = false;
		value = day;
		close();
	}

	function focusDay() {
		grid?.querySelector<HTMLElement>(`[data-day="${focused}"]`)?.focus();
	}

	async function moveTo(day: Date) {
		focused = toDay(day);
		if (day.getMonth() !== month.getMonth() || day.getFullYear() !== month.getFullYear()) month = new Date(day.getFullYear(), day.getMonth(), 1);
		await tick();
		focusDay();
	}

	function shiftMonth(n: number) {
		month = new Date(month.getFullYear(), month.getMonth() + n, 1);
	}

	function onGridKey(e: KeyboardEvent) {
		const at = parseDay(focused) ?? new Date();
		const moves: Record<string, () => Date> = {
			ArrowLeft: () => addDays(at, -1),
			ArrowRight: () => addDays(at, 1),
			ArrowUp: () => addDays(at, -7),
			ArrowDown: () => addDays(at, 7),
			PageUp: () => new Date(at.getFullYear(), at.getMonth() - 1, at.getDate()),
			PageDown: () => new Date(at.getFullYear(), at.getMonth() + 1, at.getDate())
		};
		if (moves[e.key]) {
			e.preventDefault();
			void moveTo(moves[e.key]());
		} else if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			pick(focused);
		}
	}

	function onRootKey(e: KeyboardEvent) {
		if (e.key === 'Escape' && open) {
			e.preventDefault();
			e.stopPropagation();
			close();
		}
	}

	// A click anywhere else, or scrolling what it sits in, closes it.
	$effect(() => {
		if (!open) return;
		const away = (e: PointerEvent) => {
			if (root && !root.contains(e.target as Node)) close(false);
		};
		const scrolled = (e: Event) => {
			if (!(e.target instanceof Node && root?.contains(e.target))) close(false);
		};
		document.addEventListener('pointerdown', away);
		document.addEventListener('scroll', scrolled, true);
		window.addEventListener('resize', scrolled);
		return () => {
			document.removeEventListener('pointerdown', away);
			document.removeEventListener('scroll', scrolled, true);
			window.removeEventListener('resize', scrolled);
		};
	});

	const inRange = (day: string) => !!range && day >= range.start && day <= range.end;
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="date-input" class:invalid bind:this={root} onkeydown={onRootKey}>
	<input
		type="text"
		inputmode="numeric"
		placeholder="DD-MM-YYYY"
		aria-label={label}
		aria-invalid={invalid}
		bind:value={text}
		{oninput}
		onblur={commit}
		onkeydown={(e) => {
			if (e.key === 'Enter') commit();
			if (e.key === 'ArrowDown' && e.altKey) show();
		}}
	/>
	<button type="button" class="date-toggle" aria-label="Pick {label.toLowerCase()} from a calendar" aria-expanded={open} onclick={() => (open ? close() : show())}>
		<CalendarDays size={15} />
	</button>

	{#if open}
		<div class="cal" role="dialog" aria-label="Choose {label.toLowerCase()}" style="top: {place.top}px; left: {place.left}px">
			<div class="cal-head">
				<button type="button" class="cal-nav" aria-label="Previous month" onclick={() => shiftMonth(-1)}><ChevronLeft size={16} /></button>
				<span class="cal-title">{monthTitle.format(month)}</span>
				<button type="button" class="cal-nav" aria-label="Next month" onclick={() => shiftMonth(1)}><ChevronRight size={16} /></button>
			</div>
			<div class="cal-grid" role="grid" tabindex="-1" bind:this={grid} onkeydown={onGridKey}>
				{#each WEEKDAYS as w (w)}<span class="cal-wd" role="columnheader">{w}</span>{/each}
				{#each days as d (d.day)}
					<button
						type="button"
						role="gridcell"
						class="cal-day"
						class:out={!d.inMonth}
						class:today={d.day === today()}
						class:selected={d.day === value}
						class:ranged={inRange(d.day)}
						disabled={!allowed(d.day)}
						tabindex={d.day === focused ? 0 : -1}
						data-day={d.day}
						aria-label={dayName.format(d.date)}
						aria-selected={d.day === value}
						onclick={() => pick(d.day)}
						onfocus={() => (focused = d.day)}
					>
						{d.date.getDate()}
					</button>
				{/each}
			</div>
			<div class="cal-foot">
				<button type="button" class="link accent" onclick={() => pick(today())} disabled={!allowed(today())}>Today</button>
				<span class="hint">{european(value)}</span>
			</div>
		</div>
	{/if}
</div>
