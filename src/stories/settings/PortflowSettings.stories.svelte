<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, userEvent, within } from 'storybook/test';
	import PortflowSettings from '$lib/components/settings/PortflowSettings.svelte';

	// Settings > Portflow, signed in. Remember me saves the Fontys login: the password in the system keychain.
	const { Story } = defineMeta({ title: 'Settings/PortflowSettings', component: PortflowSettings });

	async function openRemember(canvasElement: HTMLElement) {
		const canvas = within(canvasElement);
		await userEvent.click(await canvas.findByRole('switch', { name: 'Remember me' }));
		await userEvent.type(canvas.getByLabelText('Fontys username'), 'i123456@fontysict.nl');
		await userEvent.type(canvas.getByLabelText('Password'), 'not-a-real-password');
		return canvas;
	}
</script>

<Story name="Signed in" />

<Story
	name="Remember me, filling in"
	play={async ({ canvasElement }) => {
		const canvas = await openRemember(canvasElement);
		await expect(canvas.getByText(/kept in the system keychain/)).toBeInTheDocument();
	}}
/>

<Story
	name="Remembered"
	play={async ({ canvasElement }) => {
		const canvas = await openRemember(canvasElement);
		await userEvent.click(canvas.getByRole('button', { name: 'Remember' }));
		await expect(await canvas.findByText('i123456@fontysict.nl')).toBeInTheDocument();
	}}
/>
