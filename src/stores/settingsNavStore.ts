import { create } from "zustand";
import type { ModelCategory } from "@/bindings";
import type { SidebarSection } from "@/components/Sidebar";

export type ModelsFilter = ModelCategory | "all";

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
