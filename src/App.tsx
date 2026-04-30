import { useEffect, useState, useRef } from "react";
import { toast, Toaster } from "sonner";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import { ModelStateEvent, RecordingErrorEvent } from "./lib/types/events";
import "./App.css";
import Footer from "./components/footer";
import Onboarding from "./components/onboarding";
import {
  Sidebar,
  SidebarSection,
  SECTIONS_CONFIG,
  resolveSidebarSection,
} from "./components/Sidebar";
import ErrorBoundary from "./components/ErrorBoundary";
import { useSettings } from "./hooks/useSettings";
import { useSettingsStore } from "./stores/settingsStore";
import { useSettingsNavStore } from "./stores/settingsNavStore";
import { commands } from "@/bindings";
import { getLanguageDirection, initializeRTL } from "@/lib/utils/rtl";

type OnboardingStep = "model" | "done" | "error";

const ONBOARDING_IPC_TIMEOUT_MS = 5000;

// Module-scope flag so React.StrictMode's intentional dev double-mount
// doesn't fire `frontendBootComplete` twice. A useRef would reset on the
// throwaway first mount, leading to two `[boot] frontend boot complete`
// log lines per dev cold launch and undermining the "single line per boot"
// triage promise. Production is single-pass either way; this purely
// cleans up the dev signal.
let bootCompleteReported = false;

function withTimeout<T>(
  promise: Promise<T>,
  ms: number,
  label: string,
): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(
        () => reject(new Error(`${label} timed out after ${ms} ms`)),
        ms,
      ),
    ),
  ]);
}

const renderSettingsContent = (section: SidebarSection) => {
  const resolved = resolveSidebarSection(section);
  const ActiveComponent = SECTIONS_CONFIG[resolved].component;
  return <ActiveComponent />;
};

function App() {
  const { t, i18n } = useTranslation();
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep | null>(
    null,
  );
  const [onboardingError, setOnboardingError] = useState<string | null>(null);
  const [currentSection, setCurrentSection] = [
    useSettingsNavStore((s) => s.currentSection),
    useSettingsNavStore((s) => s.setCurrentSection),
  ];
  const { settings, updateSetting } = useSettings();
  const direction = getLanguageDirection(i18n.language);
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const hasCompletedPostOnboardingInit = useRef(false);

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  // Initialize RTL direction when language changes
  useEffect(() => {
    initializeRTL(i18n.language);
  }, [i18n.language]);

  // Initialize Enigo, shortcuts, and refresh audio devices when main app loads
  useEffect(() => {
    if (onboardingStep === "done" && !hasCompletedPostOnboardingInit.current) {
      hasCompletedPostOnboardingInit.current = true;
      refreshAudioDevices();
      refreshOutputDevices();
    }
  }, [onboardingStep, refreshAudioDevices, refreshOutputDevices]);

  // Report frontend boot timings to the rust log exactly once, the first
  // time the main editor surface mounts. The rust log is the single shared
  // triage surface that survives across user machines (no devtools needed),
  // so emitting one INFO line per cold boot lets us spot regressions in
  // bootstrap latency without re-instrumenting. Single payload (vs.
  // per-phase commands) avoids the IPC round-trips themselves moving the
  // numbers we're recording. See `commands::boot::frontend_boot_complete`.
  // Dedupe flag is module-scope (not useRef) so React.StrictMode's dev
  // double-mount doesn't cause two `[boot]` lines per cold launch.
  useEffect(() => {
    if (onboardingStep !== "done" || bootCompleteReported) return;
    bootCompleteReported = true;
    const timings = window.__toasterBootTimings;
    if (!timings) return;
    const editorReadyMs = Math.round(
      performance.now() - timings.bootstrap_start_ts,
    );
    void commands.frontendBootComplete({
      bootstrap_start_ms: timings.bootstrap_start_ms,
      imports_done_ms: timings.imports_done_ms ?? 0,
      react_mount_ms: timings.react_mount_ms ?? 0,
      editor_ready_ms: editorReadyMs,
    });
    console.info(`[boot] editor-ready +${editorReadyMs}ms`);
  }, [onboardingStep]);

  // Handle keyboard shortcuts for debug mode toggle
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check for Ctrl+Shift+D (Windows/Linux) or Cmd+Shift+D (macOS)
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    // Add event listener when component mounts
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup event listener when component unmounts
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [settings?.debug_mode, updateSetting]);

  // Listen for recording errors from the backend and show a toast
  useEffect(() => {
    const unlisten = listen<RecordingErrorEvent>("recording-error", (event) => {
      const { error_type, detail } = event.payload;

      if (error_type === "microphone_permission_denied") {
        const currentPlatform = platform();
        const platformKey = `errors.micPermissionDenied.${currentPlatform}`;
        const description = t(platformKey, {
          defaultValue: t("errors.micPermissionDenied.generic"),
        });
        toast.error(t("errors.micPermissionDeniedTitle"), { description });
      } else if (error_type === "no_input_device") {
        toast.error(t("errors.noInputDeviceTitle"), {
          description: t("errors.noInputDevice"),
        });
      } else {
        toast.error(
          t("errors.recordingFailed", { error: detail ?? "Unknown error" }),
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for paste failures and show a toast.
  // The technical error detail is logged to toaster.log on the Rust side
  // (see actions.rs `error!("Failed to paste transcription: ...")`),
  // so we show a localized, user-friendly message here instead of the raw error.
  useEffect(() => {
    const unlisten = listen("paste-error", () => {
      toast.error(t("errors.pasteFailedTitle"), {
        description: t("errors.pasteFailed"),
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for model loading failures and show a toast
  useEffect(() => {
    const unlisten = listen<ModelStateEvent>("model-state-changed", (event) => {
      if (event.payload.event_type === "loading_failed") {
        toast.error(
          t("errors.modelLoadFailed", {
            model:
              event.payload.model_name || t("errors.modelLoadFailedUnknown"),
          }),
          {
            description: event.payload.error,
          },
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  const checkOnboardingStatus = async () => {
    try {
      // Race the IPC call against a 5-second timeout. v0.1.0 had a bootstrap
      // failure mode where the JS bridge crashed before any IPC could resolve,
      // and `hasAnyModelsAvailable` would never settle — leaving the app on a
      // permanent white loading screen. Surfacing the timeout as an "error"
      // step renders a visible retry/reload UI instead of an indefinite
      // spinner. Successful path is unchanged: backend resolves in <100 ms.
      const result = await withTimeout(
        commands.hasAnyModelsAvailable(),
        ONBOARDING_IPC_TIMEOUT_MS,
        "hasAnyModelsAvailable",
      );
      const hasModels = result.status === "ok" && result.data;
      setOnboardingError(null);
      setOnboardingStep(hasModels ? "done" : "model");
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      const message =
        error instanceof Error ? error.message : String(error ?? "unknown");
      // Distinguish "backend reachable but reported error" (recoverable —
      // proceed to model picker) from "IPC bridge unreachable" (unrecoverable
      // without a reload — show error state with details).
      if (message.includes("timed out")) {
        setOnboardingError(message);
        setOnboardingStep("error");
      } else {
        setOnboardingError(null);
        setOnboardingStep("model");
      }
    }
  };

  const handleModelSelected = () => {
    // Transition to main app - user has started a download
    setOnboardingStep("done");
  };

  // Still checking onboarding status. The check pings backend IPC
  // (`commands.hasAnyModelsAvailable`); if the IPC bridge is healthy this
  // resolves in <100 ms. Render a minimal loading splash instead of `null`
  // so the user sees an animated indicator rather than a blank window. If
  // the IPC call hangs (the v0.1.0 white-screen failure mode), the splash
  // gives way to a visible error state via WS-E2 below.
  if (onboardingStep === null) {
    return (
      <div
        dir={direction}
        className="h-screen flex flex-col items-center justify-center bg-background gap-3 select-none"
      >
        <div className="w-8 h-8 border-2 border-mid-gray/30 border-t-mid-gray rounded-full animate-spin" />
        <p className="text-sm text-mid-gray">{t("common.loading")}</p>
      </div>
    );
  }

  if (onboardingStep === "model") {
    return <Onboarding onModelSelected={handleModelSelected} />;
  }

  if (onboardingStep === "error") {
    // Hardcoded English fallback strings: this UI fires when the IPC bridge
    // is broken, so i18next backend may also be unreachable. Same precedent
    // as ErrorBoundary.tsx — emergency fallback, always English, always
    // readable. Variables (vs. inline literals) appease eslint i18next/
    // no-literal-string under markupOnly: true.
    const errorTitle = "Toaster could not start";
    const errorBody =
      "The app failed to reach the backend within 5 seconds. This usually means the IPC bridge is broken. Please reload, and report this if it persists.";
    const detailsLabel = "Error details";
    const retryLabel = "Retry";
    const reloadLabel = "Reload";
    return (
      <div
        dir={direction}
        className="h-screen flex flex-col items-center justify-center bg-background gap-4 select-none p-8"
      >
        <h2 className="text-lg font-semibold">{errorTitle}</h2>
        <p className="text-sm text-mid-gray text-center max-w-md">
          {errorBody}
        </p>
        {onboardingError ? (
          <details className="text-xs text-mid-gray/70 max-w-md">
            <summary className="cursor-pointer">{detailsLabel}</summary>
            <pre className="mt-2 whitespace-pre-wrap break-words bg-mid-gray/10 p-3 rounded">
              {onboardingError}
            </pre>
          </details>
        ) : null}
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => {
              setOnboardingError(null);
              setOnboardingStep(null);
              checkOnboardingStatus();
            }}
            className="px-4 py-2 bg-mid-gray/20 hover:bg-mid-gray/30 rounded-lg text-sm font-medium transition-colors"
          >
            {retryLabel}
          </button>
          <button
            type="button"
            onClick={() => window.location.reload()}
            className="px-4 py-2 bg-mid-gray/10 hover:bg-mid-gray/20 rounded-lg text-sm font-medium transition-colors"
          >
            {reloadLabel}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div
      dir={direction}
      className="h-screen flex flex-col select-none cursor-default"
    >
      <Toaster
        theme="system"
        expand
        visibleToasts={5}
        toastOptions={{
          unstyled: true,
          classNames: {
            toast:
              "bg-background border border-mid-gray/20 rounded-lg shadow-lg px-4 py-3 flex items-center gap-3 text-sm",
            title: "font-medium",
            description: "text-mid-gray",
          },
        }}
      />
      {/* Main content area that takes remaining space */}
      <div className="flex-1 flex overflow-hidden">
        <ErrorBoundary>
          <Sidebar
            activeSection={currentSection}
            onSectionChange={setCurrentSection}
          />
        </ErrorBoundary>
        {/* Scrollable content area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          <div className="flex-1 overflow-y-auto">
            <ErrorBoundary>
              <div className="flex flex-col items-center py-4 px-4 sm:px-6 lg:px-8 gap-4">
                {renderSettingsContent(currentSection)}
              </div>
            </ErrorBoundary>
          </div>
        </div>
      </div>
      {/* Fixed footer at bottom */}
      <Footer />
    </div>
  );
}

export default App;
