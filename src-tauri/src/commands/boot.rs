//! Frontend boot-timing telemetry.
//!
//! Lets the frontend bootstrap pipe a single line of timing data into the rust
//! log once the editor has rendered. Before this existed, Toaster had no
//! triagable record of frontend cold-boot latency: the rust log was silent
//! between window-shown and the first user-driven IPC, which made boot
//! regressions (most notably the v0.1.0 manualChunks white-screen) impossible
//! to diagnose without attaching WebView2 devtools live.
//!
//! Cost on the critical path: one IPC round-trip, fired AFTER the editor is
//! visible (see `App.tsx` post-onboarding `useEffect`), so it doesn't slow
//! down the boot it's measuring.
//!
//! Single payload (vs. a per-phase command) is intentional: 4 round-trips
//! during boot would themselves move the numbers we're trying to record.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Frontend-side boot timings, in milliseconds since the bootstrap entry point.
///
/// Phases, in order:
///
/// - `bootstrap_start_ms`: always 0. Anchors the marker so the rust log shows
///   a clear start of the frontend boot phase.
/// - `imports_done_ms`: i18n + modelStore dynamic imports both resolved
///   (parallelized via `Promise.all`).
/// - `react_mount_ms`: `ReactDOM.createRoot(...).render(<App />)` returned.
///   React hasn't necessarily flushed yet, but the synchronous render call
///   has been issued.
/// - `editor_ready_ms`: App.tsx `onboardingStep === "done"` and the main
///   editor surface (Sidebar + Footer + content) has mounted at least once.
///   This is "user can see the editor" from the frontend's perspective.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct FrontendBootTimings {
    pub bootstrap_start_ms: u32,
    pub imports_done_ms: u32,
    pub react_mount_ms: u32,
    pub editor_ready_ms: u32,
}

#[tauri::command]
#[specta::specta]
pub fn frontend_boot_complete(timings: FrontendBootTimings) {
    log::info!(
        "[boot] frontend boot complete: imports_done={}ms react_mount={}ms editor_ready={}ms (start={}ms)",
        timings.imports_done_ms,
        timings.react_mount_ms,
        timings.editor_ready_ms,
        timings.bootstrap_start_ms,
    );
}
