<script lang="ts">
	// Gamma's mark, Γγ: a capital gamma and, lower, a small one whose right arm curves down into its tail. Lucide has no
	// gamma, so it is drawn here on Lucide's grid (24) with Lucide's props: <GammaIcon size={16} />. Its ends are cut
	// square rather than rounded, which is what makes it read as a mark.
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
	class="lucide-icon lucide lucide-gamma {className}"
	aria-hidden="true"
	{...rest}
>
	<!-- Γ: the stem and the bar, one stroke. -->
	<path d="M3 19V4h7.5" />
	<!-- γ: the left arm straight down to where the arms meet; the right arm curving on into the tail. -->
	<path d="M13 9.5l5.5 7" />
	<path d="M22 9.5c-2.6 3.4-4.2 6.6-4.2 12" />
</svg>
