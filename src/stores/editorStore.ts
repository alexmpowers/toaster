import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { Word } from "@/bindings";
export type { Word };

export interface TimingContractSnapshot {
  timeline_revision: number;
  total_words: number;
  deleted_words: number;
  active_words: number;
  source_start_us: number;
  source_end_us: number;
  total_keep_duration_us: number;
  keep_segments: Array<{ start_us: number; end_us: number }>;
  quantized_keep_segments: Array<{ start_us: number; end_us: number }>;
  quantization_fps_num: number;
  quantization_fps_den: number;
  keep_segments_valid: boolean;
  warning: string | null;
}

interface EditorProjection {
  words: Word[];
  timing_contract: TimingContractSnapshot;
}

interface EditorState {
  words: Word[];
  timingContract: TimingContractSnapshot | null;
  selectedIndex: number | null;
  selectionRange: [number, number] | null;
  highlightedIndices: number[];
  highlightType: "filler" | "pause" | "duplicate" | null;

  setWords: (words: Word[]) => Promise<void>;
  deleteWord: (index: number) => Promise<void>;
  restoreWord: (index: number) => Promise<void>;
  deleteRange: (start: number, end: number) => Promise<void>;
  restoreAll: () => Promise<void>;
  splitWord: (index: number, position: number) => Promise<void>;
  silenceWord: (index: number) => Promise<void>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  refreshFromBackend: () => Promise<void>;
  getKeepSegments: () => Promise<[number, number][]>;
  selectWord: (index: number | null) => void;
  setSelectionRange: (range: [number, number] | null) => void;
  setHighlightedIndices: (indices: number[], type: "filler" | "pause" | "duplicate" | null) => void;
  clearHighlights: () => void;
}

const fetchProjection = async (): Promise<EditorProjection> =>
  invoke<EditorProjection>("editor_get_projection");

export const useEditorStore = create<EditorState>()((set) => ({
  words: [],
  timingContract: null,
  selectedIndex: null,
  selectionRange: null,
  highlightedIndices: [],
  highlightType: null,

  setWords: async (words: Word[]) => {
    await invoke<Word[]>("editor_set_words", { words });
    const projection = await fetchProjection();
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
      selectedIndex: null,
      selectionRange: null,
    });
  },

  deleteWord: async (index: number) => {
    await invoke<boolean>("editor_delete_word", { index });
    const projection = await fetchProjection();
    set({ words: projection.words, timingContract: projection.timing_contract });
  },

  restoreWord: async (index: number) => {
    await invoke<boolean>("editor_restore_word", { index });
    const projection = await fetchProjection();
    set({ words: projection.words, timingContract: projection.timing_contract });
  },

  deleteRange: async (start: number, end: number) => {
    await invoke<boolean>("editor_delete_range", { start, end });
    const projection = await fetchProjection();
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
      selectedIndex: null,
      selectionRange: null,
    });
  },

  restoreAll: async () => {
    await invoke<boolean>("editor_restore_all");
    const projection = await fetchProjection();
    set({ words: projection.words, timingContract: projection.timing_contract });
  },

  splitWord: async (index: number, position: number) => {
    await invoke<boolean>("editor_split_word", { index, position });
    const projection = await fetchProjection();
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
      selectedIndex: null,
    });
  },

  silenceWord: async (index: number) => {
    await invoke<boolean>("editor_silence_word", { index });
    const projection = await fetchProjection();
    set({ words: projection.words, timingContract: projection.timing_contract });
  },

  undo: async () => {
    await invoke<boolean>("editor_undo");
    const projection = await fetchProjection();
    set({ words: projection.words, timingContract: projection.timing_contract });
  },

  redo: async () => {
    await invoke<boolean>("editor_redo");
    const projection = await fetchProjection();
    set({ words: projection.words, timingContract: projection.timing_contract });
  },

  refreshFromBackend: async () => {
    const projection = await fetchProjection();
    set({ words: projection.words, timingContract: projection.timing_contract });
  },

  getKeepSegments: async () => {
    return await invoke<[number, number][]>("editor_get_keep_segments");
  },

  selectWord: (index: number | null) => {
    set({ selectedIndex: index, selectionRange: null });
  },

  setSelectionRange: (range: [number, number] | null) => {
    set({ selectionRange: range });
  },

  setHighlightedIndices: (indices: number[], type: "filler" | "pause" | "duplicate" | null) => {
    set({ highlightedIndices: indices, highlightType: type });
  },

  clearHighlights: () => {
    set({ highlightedIndices: [], highlightType: null });
  },
}));
