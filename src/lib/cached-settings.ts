import { commands, type AppSettings } from "@/bindings";

/**
 * Shared in-flight cache for `commands.getAppSettings()`.
 *
 * Two boot paths historically issued this same IPC redundantly:
 *
 * 1. `src/i18n/index.ts` — `syncLanguageFromSettings` reads `app_language` to
 *    pick the locale before React mounts. Module-side fire-and-forget.
 * 2. `src/stores/settingsStore.ts` — `refreshSettings` reads the full settings
 *    object to populate the settings UI. Triggered from `App.tsx` via
 *    `useSettings`.
 *
 * Both fire on cold boot. On a slow WebView2 startup an IPC round-trip can
 * cost 200–500 ms, so the duplicate was a measurable chunk of the
 * icon→editor latency the user complained about. Routing both callers
 * through this module collapses the pair into a single round-trip while
 * keeping the existing call sites independent of each other (no implicit
 * ordering coupling between i18n init and settings store init).
 *
 * The cache is keyed on the in-flight promise, not the resolved value: a
 * second caller that arrives after the first one resolves will issue a fresh
 * IPC. This is intentional — the only goal is to dedupe boot-time
 * concurrency, not to provide long-lived caching (which would fight the
 * settings-store live-update contract).
 */
let inflight: Promise<AppSettings | null> | null = null;

export function getCachedAppSettings(): Promise<AppSettings | null> {
  if (inflight) return inflight;

  inflight = (async () => {
    try {
      const result = await commands.getAppSettings();
      return result.status === "ok" ? result.data : null;
    } finally {
      // Clear the cache once resolved so a later explicit refresh
      // (e.g. after the user changes locale) issues a fresh IPC.
      // We keep the slot occupied for the synchronous-await window
      // by deferring with a microtask.
      queueMicrotask(() => {
        inflight = null;
      });
    }
  })();

  return inflight;
}
