/** SHA-256 of `text` as UTF-8, in lower-case hex: how the backend writes the hash of a file it holds. */
export async function sha256Hex(text: string): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
	return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
}
