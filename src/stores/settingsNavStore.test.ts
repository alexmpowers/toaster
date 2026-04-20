import { describe, it, expect, beforeEach } from "vitest";
import { useSettingsNavStore } from "./settingsNavStore";

function reset() {
  useSettingsNavStore.setState({
    currentSection: "editor",
    pendingModelsFilter: null,
  });
}

describe("settingsNavStore", () => {
  beforeEach(reset);

  it("starts on the editor section with no pending filter", () => {
    const s = useSettingsNavStore.getState();
    expect(s.currentSection).toBe("editor");
    expect(s.pendingModelsFilter).toBeNull();
  });

  it("setCurrentSection switches the active section", () => {
    useSettingsNavStore.getState().setCurrentSection("advanced");
    expect(useSettingsNavStore.getState().currentSection).toBe("advanced");
  });

  describe("navigateToModels", () => {
    it("jumps to the models section and stores the filter", () => {
      useSettingsNavStore.getState().navigateToModels("Transcription");
      const s = useSettingsNavStore.getState();
      expect(s.currentSection).toBe("models");
      expect(s.pendingModelsFilter).toBe("Transcription");
    });

    it("accepts the 'all' sentinel filter", () => {
      useSettingsNavStore.getState().navigateToModels("all");
      expect(useSettingsNavStore.getState().pendingModelsFilter).toBe("all");
    });
  });

  describe("consumePendingModelsFilter", () => {
    it("returns the pending filter and clears it (one-shot)", () => {
      useSettingsNavStore.getState().navigateToModels("Transcription");
      const first = useSettingsNavStore.getState().consumePendingModelsFilter();
      expect(first).toBe("Transcription");
      const second = useSettingsNavStore
        .getState()
        .consumePendingModelsFilter();
      expect(second).toBeNull();
      expect(useSettingsNavStore.getState().pendingModelsFilter).toBeNull();
    });

    it("returns null when nothing is pending (no state change)", () => {
      const before = useSettingsNavStore.getState();
      const result = useSettingsNavStore
        .getState()
        .consumePendingModelsFilter();
      expect(result).toBeNull();
      expect(useSettingsNavStore.getState()).toEqual(before);
    });
  });
});
