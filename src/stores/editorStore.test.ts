import { describe, it, expect, vi, beforeEach } from "vitest";
import type { Word } from "@/bindings";

// Mock invoke before importing the store
const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

// Must import after mock setup
const { useEditorStore } = await import("./editorStore");

function makeWord(overrides: Partial<Word> = {}): Word {
  return {
    text: "hello",
    start_us: 0,
    end_us: 1_000_000,
    deleted: false,
    silenced: false,
    confidence: 0.95,
    speaker_id: 0,
    ...overrides,
  };
}

function makeProjection(words: Word[] = []) {
  return {
    words,
    timing_contract: {
      timeline_revision: 1,
      total_words: words.length,
      deleted_words: 0,
      active_words: words.length,
      source_start_us: 0,
      source_end_us: 1_000_000,
      total_keep_duration_us: 1_000_000,
      keep_segments: [{ start_us: 0, end_us: 1_000_000 }],
      quantized_keep_segments: [{ start_us: 0, end_us: 1_000_000 }],
      quantization_fps_num: 30,
      quantization_fps_den: 1,
      keep_segments_valid: true,
      warning: null,
    },
  };
}

beforeEach(() => {
  mockInvoke.mockReset();
  // Reset store to initial state
  useEditorStore.setState({
    words: [],
    timingContract: null,
    selectedIndex: null,
    selectionRange: null,
    highlightedIndices: [],
    highlightType: null,
  });
});

describe("editorStore", () => {
  describe("initial state", () => {
    it("has empty words, no selection, no highlights", () => {
      const state = useEditorStore.getState();
      expect(state.words).toEqual([]);
      expect(state.selectedIndex).toBeNull();
      expect(state.selectionRange).toBeNull();
      expect(state.highlightedIndices).toEqual([]);
      expect(state.highlightType).toBeNull();
      expect(state.timingContract).toBeNull();
    });
  });

  describe("selectWord", () => {
    it("sets selectedIndex and clears selectionRange", () => {
      useEditorStore.setState({ selectionRange: [0, 2] });
      useEditorStore.getState().selectWord(5);
      const state = useEditorStore.getState();
      expect(state.selectedIndex).toBe(5);
      expect(state.selectionRange).toBeNull();
    });

    it("clears selectedIndex with null", () => {
      useEditorStore.getState().selectWord(3);
      useEditorStore.getState().selectWord(null);
      expect(useEditorStore.getState().selectedIndex).toBeNull();
    });
  });

  describe("setSelectionRange", () => {
    it("sets range", () => {
      useEditorStore.getState().setSelectionRange([1, 4]);
      expect(useEditorStore.getState().selectionRange).toEqual([1, 4]);
    });

    it("clears range with null", () => {
      useEditorStore.getState().setSelectionRange([1, 4]);
      useEditorStore.getState().setSelectionRange(null);
      expect(useEditorStore.getState().selectionRange).toBeNull();
    });
  });

  describe("setHighlightedIndices / clearHighlights", () => {
    it("sets highlighted indices and type", () => {
      useEditorStore.getState().setHighlightedIndices([0, 2, 5], "filler");
      const state = useEditorStore.getState();
      expect(state.highlightedIndices).toEqual([0, 2, 5]);
      expect(state.highlightType).toBe("filler");
    });

    it("clearHighlights resets indices and type", () => {
      useEditorStore.getState().setHighlightedIndices([1, 3], "pause");
      useEditorStore.getState().clearHighlights();
      const state = useEditorStore.getState();
      expect(state.highlightedIndices).toEqual([]);
      expect(state.highlightType).toBeNull();
    });
  });

  describe("setWords", () => {
    it("invokes editor_set_words and updates state from projection", async () => {
      const words = [makeWord({ text: "one" }), makeWord({ text: "two" })];
      const projection = makeProjection(words);

      mockInvoke
        .mockResolvedValueOnce(words) // editor_set_words
        .mockResolvedValueOnce(projection); // editor_get_projection

      // Pre-set selection to verify it gets cleared
      useEditorStore.setState({ selectedIndex: 2, selectionRange: [0, 1] });

      await useEditorStore.getState().setWords(words);

      expect(mockInvoke).toHaveBeenCalledWith("editor_set_words", { words });
      expect(mockInvoke).toHaveBeenCalledWith("editor_get_projection");
      const state = useEditorStore.getState();
      expect(state.words).toEqual(words);
      expect(state.timingContract).toEqual(projection.timing_contract);
      expect(state.selectedIndex).toBeNull();
      expect(state.selectionRange).toBeNull();
    });
  });

  describe("deleteWord", () => {
    it("invokes editor_delete_word with index", async () => {
      const projection = makeProjection([makeWord({ deleted: true })]);
      mockInvoke.mockResolvedValueOnce(true).mockResolvedValueOnce(projection);

      await useEditorStore.getState().deleteWord(3);

      expect(mockInvoke).toHaveBeenCalledWith("editor_delete_word", {
        index: 3,
      });
      expect(mockInvoke).toHaveBeenCalledWith("editor_get_projection");
      expect(useEditorStore.getState().words).toEqual(projection.words);
    });
  });

  describe("restoreWord", () => {
    it("invokes editor_restore_word with index", async () => {
      const projection = makeProjection([makeWord()]);
      mockInvoke.mockResolvedValueOnce(true).mockResolvedValueOnce(projection);

      await useEditorStore.getState().restoreWord(1);

      expect(mockInvoke).toHaveBeenCalledWith("editor_restore_word", {
        index: 1,
      });
      expect(mockInvoke).toHaveBeenCalledWith("editor_get_projection");
    });
  });

  describe("deleteRange", () => {
    it("invokes editor_delete_range and clears selection", async () => {
      const projection = makeProjection([]);
      mockInvoke.mockResolvedValueOnce(true).mockResolvedValueOnce(projection);

      useEditorStore.setState({ selectedIndex: 1, selectionRange: [0, 3] });

      await useEditorStore.getState().deleteRange(0, 3);

      expect(mockInvoke).toHaveBeenCalledWith("editor_delete_range", {
        start: 0,
        end: 3,
      });
      const state = useEditorStore.getState();
      expect(state.selectedIndex).toBeNull();
      expect(state.selectionRange).toBeNull();
    });
  });

  describe("restoreAll", () => {
    it("invokes editor_restore_all", async () => {
      const projection = makeProjection([makeWord()]);
      mockInvoke.mockResolvedValueOnce(true).mockResolvedValueOnce(projection);

      await useEditorStore.getState().restoreAll();

      expect(mockInvoke).toHaveBeenCalledWith("editor_restore_all");
      expect(mockInvoke).toHaveBeenCalledWith("editor_get_projection");
    });
  });

  describe("splitWord", () => {
    it("invokes editor_split_word and clears selectedIndex", async () => {
      const projection = makeProjection([
        makeWord({ text: "hel" }),
        makeWord({ text: "lo" }),
      ]);
      mockInvoke.mockResolvedValueOnce(true).mockResolvedValueOnce(projection);

      useEditorStore.setState({ selectedIndex: 0 });

      await useEditorStore.getState().splitWord(0, 3);

      expect(mockInvoke).toHaveBeenCalledWith("editor_split_word", {
        index: 0,
        position: 3,
      });
      expect(useEditorStore.getState().selectedIndex).toBeNull();
    });
  });

  describe("silenceWord", () => {
    it("invokes editor_silence_word with index", async () => {
      const projection = makeProjection([makeWord({ silenced: true })]);
      mockInvoke.mockResolvedValueOnce(true).mockResolvedValueOnce(projection);

      await useEditorStore.getState().silenceWord(2);

      expect(mockInvoke).toHaveBeenCalledWith("editor_silence_word", {
        index: 2,
      });
      expect(mockInvoke).toHaveBeenCalledWith("editor_get_projection");
    });
  });

  describe("undo", () => {
    it("invokes editor_undo and refreshes projection", async () => {
      const projection = makeProjection([makeWord()]);
      mockInvoke.mockResolvedValueOnce(true).mockResolvedValueOnce(projection);

      await useEditorStore.getState().undo();

      expect(mockInvoke).toHaveBeenCalledWith("editor_undo");
      expect(mockInvoke).toHaveBeenCalledWith("editor_get_projection");
      expect(useEditorStore.getState().words).toEqual(projection.words);
    });
  });

  describe("redo", () => {
    it("invokes editor_redo and refreshes projection", async () => {
      const projection = makeProjection([makeWord()]);
      mockInvoke.mockResolvedValueOnce(true).mockResolvedValueOnce(projection);

      await useEditorStore.getState().redo();

      expect(mockInvoke).toHaveBeenCalledWith("editor_redo");
      expect(mockInvoke).toHaveBeenCalledWith("editor_get_projection");
      expect(useEditorStore.getState().words).toEqual(projection.words);
    });
  });

  describe("refreshFromBackend", () => {
    it("fetches projection and updates state", async () => {
      const projection = makeProjection([makeWord({ text: "fresh" })]);
      mockInvoke.mockResolvedValueOnce(projection);

      await useEditorStore.getState().refreshFromBackend();

      expect(mockInvoke).toHaveBeenCalledWith("editor_get_projection");
      expect(useEditorStore.getState().words).toEqual(projection.words);
      expect(useEditorStore.getState().timingContract).toEqual(
        projection.timing_contract,
      );
    });
  });

  describe("getKeepSegments", () => {
    it("invokes editor_get_keep_segments and returns result", async () => {
      const segments: [number, number][] = [
        [0, 500_000],
        [700_000, 1_000_000],
      ];
      mockInvoke.mockResolvedValueOnce(segments);

      const result = await useEditorStore.getState().getKeepSegments();

      expect(mockInvoke).toHaveBeenCalledWith("editor_get_keep_segments");
      expect(result).toEqual(segments);
    });
  });
});
