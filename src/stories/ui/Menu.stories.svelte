<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import Menu from '$lib/components/Menu.svelte';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import type { MenuItem } from '$lib/components/Menu.svelte';

	const { Story } = defineMeta({ title: 'UI/Menu', component: Menu });
</script>

<script lang="ts">
	const items: MenuItem[] = [
		{ id: 'rename', label: 'Rename…', icon: Pencil },
		{ id: 'delete', label: 'Delete', sub: 'Its versions stay in versions/', icon: Trash2 }
	];
</script>

<Story name="More menu" asChild>
	<Menu {items} label="More" onpick={() => {}} class="btn icon ghost"><Ellipsis size={18} /></Menu>
</Story>

<Story name="Aligned to the end" asChild>
	<div style="display:flex;justify-content:flex-end">
		<Menu {items} label="More" align="end" onpick={() => {}} class="btn small">Actions</Menu>
	</div>
</Story>
