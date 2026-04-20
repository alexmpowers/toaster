import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { Word } from "@/bindings";
export type { Word };

/**
 * Transcript-editor state. The frontend holds a projection of the backend
 * editor's word list; the **backend is the source of truth** for keep
 * segments, time mapping, and undo history (AGENTS.md: editor & time-mapping
 * authority lives in Rust).
 *
 * Mutation flow for every word op (delete/silence/split/undo/redo):
 *   1. UI handler calls the corresponding Tauri `editor_*` command.
 *   2. Backend applies the op and emits the new Word list + timing contract.
 *   3. `refreshFromBackend()` replays the result into this store.
 *
 * Never mutate `words` directly in response to a user action — round-trip
 * through the backend so preview and export stay aligned. Local-only UI
 * state (selectedIndex, selection ranges, highlights) is safe to set here
 * without a backend call.
 */
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
  /**
   * Session-scoped per-project export intent. Not persisted to backend —
   * lives with this store's lifetime so switching to the Advanced settings
   * tab and back does NOT reset the "Add captions" toggle (FB-7 E-1).
   */
  burnCaptions: boolean;

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
  setHighlightedIndices: (
    indices: number[],
    type: "filler" | "pause" | "duplicate" | null,
  ) => void;
  clearHighlights: () => void;
  setBurnCaptions: (next: boolean) => void;
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
  burnCaptions: false,

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
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
    });
  },

  restoreWord: async (index: number) => {
    await invoke<boolean>("editor_restore_word", { index });
    const projection = await fetchProjection();
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
    });
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
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
    });
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
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
    });
  },

  undo: async () => {
    await invoke<boolean>("editor_undo");
    const projection = await fetchProjection();
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
    });
  },

  redo: async () => {
    await invoke<boolean>("editor_redo");
    const projection = await fetchProjection();
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
    });
  },

  refreshFromBackend: async () => {
    const projection = await fetchProjection();
    set({
      words: projection.words,
      timingContract: projection.timing_contract,
    });
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

  setHighlightedIndices: (
    indices: number[],
    type: "filler" | "pause" | "duplicate" | null,
  ) => {
    set({ highlightedIndices: indices, highlightType: type });
  },

  clearHighlights: () => {
    set({ highlightedIndices: [], highlightType: null });
  },

  setBurnCaptions: (next: boolean) => {
    set({ burnCaptions: next });
  },
}));
