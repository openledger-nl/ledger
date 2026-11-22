<script lang="ts">
	// A time on the 24-hour clock, "14:05", whatever the system's locale: the browser's own time field follows the
	// system and can show AM/PM. Typed as 1405, 14.05, 14:5 or 9 (= 09:00); the arrow keys move a quarter of an hour.
	// `value` is "HH:MM", or "" when empty.
	import Clock from '@lucide/svelte/icons/clock';

	let { value = $bindable(''), label }: { value?: string; label: string } = $props();

	let text = $state('');
	let invalid = $state(false);
	let typing = false;

	// Follows the value whenever it changes from outside, but not while it is being typed.
	$effect(() => {
		const v = value;
		if (!typing) {
			text = v;
			invalid = false;
		}
	});

	/** "1405", "14.05", "14:5", "9" → "14:05" / "09:00"; null when it isn't a time. */
	function parse(raw: string): string | null {
		const t = raw.trim();
		if (!t) return '';
		const m = /^(\d{1,2})(?:[:.\s]?(\d{1,2}))?$/.exec(t) ?? /^(\d{2})(\d{2})$/.exec(t);
		if (!m) return null;
		const h = +m[1];
		const min = m[2] === undefined ? 0 : +m[2];
		if (h > 23 || min > 59) return null;
		return `${String(h).padStart(2, '0')}:${String(min).padStart(2, '0')}`;
	}

	function commit() {
		typing = false;
		const parsed = parse(text);
		invalid = parsed === null;
		if (parsed !== null) {
			value = parsed;
			text = parsed;
		}
	}

	function step(minutes: number) {
		const [h, m] = (parse(text) || '09:00').split(':').map(Number);
		const total = (((h * 60 + m + minutes) % 1440) + 1440) % 1440;
		const snapped = Math.round(total / 15) * 15 === total ? total : Math.round(total / 15) * 15;
		value = text = `${String(Math.floor(snapped / 60) % 24).padStart(2, '0')}:${String(snapped % 60).padStart(2, '0')}`;
		invalid = false;
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
			e.preventDefault();
			step(e.key === 'ArrowUp' ? 15 : -15);
		} else if (e.key === 'Enter') {
			commit();
		}
	}
</script>

<span class="time-input" class:invalid>
	<input
		bind:value={text}
		oninput={() => (typing = true)}
		onblur={commit}
		{onkeydown}
		inputmode="numeric"
		placeholder="14:00"
		aria-label={label}
		aria-invalid={invalid}
		title={invalid ? 'A time on the 24-hour clock, e.g. 14:05' : label}
		autocomplete="off"
		spellcheck="false"
	/>
	<Clock size={14} />
</span>
