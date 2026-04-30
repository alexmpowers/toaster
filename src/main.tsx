import React from "react";
import ReactDOM from "react-dom/client";
import { platform } from "@tauri-apps/plugin-os";
import App from "./App";

// Forensic anchor for bug reports. Logged before any other module touches the
// DOM so devtools captures a timestamp + version even if the rest of bootstrap
// throws. import.meta.env.PACKAGE_VERSION is injected by Vite via define() in
// vite.config.ts; if undefined, fall back to the literal string.
const BOOT_VERSION =
  (typeof import.meta.env.PACKAGE_VERSION === "string" &&
    import.meta.env.PACKAGE_VERSION) ||
  "unknown";
console.info(`[boot] toaster v${BOOT_VERSION} ${new Date().toISOString()}`);

// Render a visible failure panel into #root if anything in the bootstrap
// throws synchronously. This guards against the v0.1.0 white-screen regression
// (a vendor-chunk CJS-interop race that crashed before React mounted): without
// this fallback the user saw a blank window with no error indication. The
// fallback is plain HTML — no React, no i18n, no Tauri APIs — so it survives
// even if every framework module is broken.
function renderBootFailure(error: unknown): void {
  const root = document.getElementById("root");
  if (!root) return;
  const message =
    error instanceof Error ? error.message : String(error ?? "Unknown error");
  const stack = error instanceof Error && error.stack ? error.stack : "";
  root.innerHTML = `
    <div style="font-family: system-ui, sans-serif; padding: 2rem; max-width: 640px; margin: 0 auto; color: #e5e5e5; background: #171717; min-height: 100vh; box-sizing: border-box;">
      <h1 style="font-size: 1.25rem; margin: 0 0 0.5rem 0;">Toaster failed to start</h1>
      <p style="color: #a3a3a3; font-size: 0.875rem; margin: 0 0 1rem 0;">
        Toaster v${BOOT_VERSION} hit an unrecoverable error before the editor could load.
        Please report this with the message and stack below.
      </p>
      <pre style="background: #262626; padding: 0.75rem; border-radius: 0.375rem; font-size: 0.8125rem; white-space: pre-wrap; word-break: break-word; color: #fca5a5; margin: 0 0 1rem 0;">${escapeHtml(message)}</pre>
      ${stack ? `<details style="font-size: 0.75rem; color: #737373;"><summary style="cursor: pointer;">Stack trace</summary><pre style="background: #262626; padding: 0.75rem; border-radius: 0.375rem; white-space: pre-wrap; word-break: break-word; margin-top: 0.5rem;">${escapeHtml(stack)}</pre></details>` : ""}
      <button onclick="window.location.reload()" style="margin-top: 1rem; padding: 0.5rem 1rem; background: #404040; color: #e5e5e5; border: none; border-radius: 0.375rem; cursor: pointer; font-size: 0.875rem;">Reload</button>
    </div>
  `;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

window.addEventListener("error", (event) => {
  console.error("[boot] window.error", event.error ?? event.message);
});
window.addEventListener("unhandledrejection", (event) => {
  console.error("[boot] unhandledrejection", event.reason);
});

// Boot-timing collector. Populated by `bootstrap()` and read once by App.tsx
// after the editor is visible — see `commands.frontendBootComplete` for why
// we batch these into a single post-render IPC instead of phase-by-phase.
//
// All `*_ms` fields measure milliseconds elapsed from `bootstrap_start_ts`
// (a `performance.now()` reading taken at the top of `bootstrap()`), NOT
// from page navigation start. App.tsx subtracts `bootstrap_start_ts` from
// its own `performance.now()` to compute `editor_ready_ms` on the same
// origin.
declare global {
  interface Window {
    __toasterBootTimings?: {
      bootstrap_start_ts: number;
      bootstrap_start_ms: number;
      imports_done_ms?: number;
      react_mount_ms?: number;
    };
  }
}

async function bootstrap(): Promise<void> {
  const bootStart = performance.now();
  window.__toasterBootTimings = {
    bootstrap_start_ts: bootStart,
    bootstrap_start_ms: Math.round(bootStart),
  };
  console.info(
    `[boot] bootstrap-start +0ms (page-relative ${Math.round(bootStart)}ms)`,
  );

  // Set platform before render so CSS can scope per-platform (e.g. scrollbar styles)
  document.documentElement.dataset.platform = platform();

  // i18n and modelStore are independent — load them in parallel rather than
  // serially. Eliminates ~one round-trip-per-import of latency on cold boot.
  const [, modelStoreModule] = await Promise.all([
    import("./i18n"),
    import("./stores/modelStore"),
  ]);
  const importsDone = Math.round(performance.now() - bootStart);
  window.__toasterBootTimings.imports_done_ms = importsDone;
  console.info(`[boot] bootstrap-imports-done +${importsDone}ms`);

  // Initialize model store (loads models and sets up event listeners).
  // Fire-and-forget on purpose: the store handles its own loading state and
  // App.tsx already shows a splash/onboarding while these IPC calls resolve.
  void modelStoreModule.useModelStore.getState().initialize();

  const rootEl = document.getElementById("root");
  if (!rootEl) {
    throw new Error("missing #root element in index.html");
  }

  ReactDOM.createRoot(rootEl as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
  const reactMount = Math.round(performance.now() - bootStart);
  window.__toasterBootTimings.react_mount_ms = reactMount;
  console.info(`[boot] react-mount +${reactMount}ms`);
}

bootstrap().catch((error) => {
  console.error("[boot] bootstrap failed", error);
  renderBootFailure(error);
});
