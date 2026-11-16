<script lang="ts">
	// Epsilon+'s mark, ε+: a small epsilon and a plus beside it, raised. Lucide has neither, so it is drawn here on
	// Lucide's grid (24) with Lucide's props: <EpsilonIcon size={16} />. Its ends are cut square rather than rounded,
	// which is what makes it read as a mark.
	import type { SVGAttributes } from 'svelte/elements';

	let {
		size = 24,
		color = 'currentColor',
		strokeWidth = 2,
		class: className = '',
		...rest
	}: { size?: number | string; color?: string; strokeWidth?: number | string; class?: string } & SVGAttributes<SVGSVGElement> = $props();
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke={color}
	stroke-width={strokeWidth}
	stroke-linecap="butt"
	stroke-linejoin="miter"
	class="lucide-icon lucide lucide-epsilon-plus {className}"
	aria-hidden="true"
	{...rest}
>
	<!-- ε: an upper bowl and a wider lower bowl, open to the right, meeting at a short middle stroke. -->
	<path d="M13 9.2C12.1 8 10.8 7.4 9.2 7.4 6.9 7.4 5.2 8.7 5.2 10.6c0 1.6 1.4 2.8 3.6 2.8H10.5" />
	<path d="M10.5 13.4H8.6c-2.4 0-4.1 1.3-4.1 3.3 0 2.1 1.9 3.6 4.6 3.6 1.9 0 3.4-.7 4.4-2" />
	<!-- +: raised to the top right, like a superscript. -->
	<path d="M19 3.5v7M15.5 7h7" />
</svg>
