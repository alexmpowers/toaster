import { create } from "zustand";
import type { ModelCategory } from "@/bindings";
import type { SidebarSection } from "@/components/Sidebar";

/**
 * Settings-page navigation state. Tracks which sidebar section is active and
 * carries a one-shot `pendingModelsFilter` used by inter-section deep links
 * (e.g. the Editor "Choose model" button jumps to the Models panel with the
 * transcription filter pre-selected).
 *
 * `consumePendingModelsFilter` is single-read by design — the Models panel
 * calls it on mount so the filter is cleared after it has been applied.
 */
type ModelsFilter = ModelCategory | "all";

interface SettingsNavStore {
  currentSection: SidebarSection;
  pendingModelsFilter: ModelsFilter | null;
  setCurrentSection: (section: SidebarSection) => void;
  navigateToModels: (filter: ModelsFilter) => void;
  consumePendingModelsFilter: () => ModelsFilter | null;
}

export const useSettingsNavStore = create<SettingsNavStore>((set, get) => ({
  currentSection: "editor",
  pendingModelsFilter: null,
  setCurrentSection: (section) => set({ currentSection: section }),
  navigateToModels: (filter) =>
    set({ currentSection: "models", pendingModelsFilter: filter }),
  consumePendingModelsFilter: () => {
    const { pendingModelsFilter } = get();
    if (pendingModelsFilter !== null) {
      set({ pendingModelsFilter: null });
    }
    return pendingModelsFilter;
  },
}));
