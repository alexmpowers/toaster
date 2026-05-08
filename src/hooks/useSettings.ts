import { useEffect, useRef } from "react";
import { useSettingsStore } from "../stores/settingsStore";
import type { AppSettings as Settings, AudioDevice } from "@/bindings";

interface UseSettingsReturn {
  // State
  settings: Settings | null;
  isLoading: boolean;
  isUpdating: (key: string) => boolean;
  audioDevices: AudioDevice[];
  outputDevices: AudioDevice[];

  // Actions
  updateSetting: <K extends keyof Settings>(
    key: K,
    value: Settings[K],
  ) => Promise<void>;
  resetSetting: (key: keyof Settings) => Promise<void>;
  refreshSettings: () => Promise<void>;
  refreshAudioDevices: () => Promise<void>;
  refreshOutputDevices: () => Promise<void>;

  // Convenience getters
  getSetting: <K extends keyof Settings>(key: K) => Settings[K] | undefined;
}

export const useSettings = (): UseSettingsReturn => {
  // Select individual fields to avoid subscribing to the entire store.
  // Zustand's default equality (Object.is) ensures stable primitive/function
  // references and prevents re-renders from unrelated store updates.
  const settings = useSettingsStore((s) => s.settings);
  const isLoading = useSettingsStore((s) => s.isLoading);
  const isUpdatingKey = useSettingsStore((s) => s.isUpdatingKey);
  const audioDevices = useSettingsStore((s) => s.audioDevices);
  const outputDevices = useSettingsStore((s) => s.outputDevices);
  const updateSetting = useSettingsStore((s) => s.updateSetting);
  const resetSetting = useSettingsStore((s) => s.resetSetting);
  const refreshSettings = useSettingsStore((s) => s.refreshSettings);
  const refreshAudioDevices = useSettingsStore((s) => s.refreshAudioDevices);
  const refreshOutputDevices = useSettingsStore((s) => s.refreshOutputDevices);
  const getSetting = useSettingsStore((s) => s.getSetting);
  const initialize = useSettingsStore((s) => s.initialize);

  // Initialize once on first mount. Ref guard prevents StrictMode
  // double-mount from firing two concurrent initialize() calls.
  const initRef = useRef(false);
  useEffect(() => {
    if (isLoading && !initRef.current) {
      initRef.current = true;
      initialize();
    }
  }, [isLoading, initialize]);

  return {
    settings,
    isLoading,
    isUpdating: isUpdatingKey,
    audioDevices,
    outputDevices,
    updateSetting,
    resetSetting,
    refreshSettings,
    refreshAudioDevices,
    refreshOutputDevices,
    getSetting,
  };
};
