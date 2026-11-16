import { startBrowserRelay } from '$lib/browserRelay';

// During development the front end also runs in an ordinary browser tab, reaching the backend through Ledger's window
// (src/lib/browserRelay.ts, scripts/browser-relay.mjs). This runs before any page loads, so the first call already goes
// through. A build has neither the dev server nor this.
if (import.meta.env.DEV && import.meta.hot) startBrowserRelay(import.meta.hot);
