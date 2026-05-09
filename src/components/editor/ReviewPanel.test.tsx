import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { current?: number; total?: number }) => {
      if (key === "editor.reviewProgress" && options) {
        return `${options.current} of ${options.total} to review`;
      }
      return key;
    },
  }),
}));

const { useEditorStore } = await import("@/stores/editorStore");
const { usePlayerStore } = await import("@/stores/playerStore");
const { default: ReviewPanel } = await import("./ReviewPanel");

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

interface LowConfidenceWord {
  word_index: number;
  text: string;
  confidence: number;
  start_us: number;
  end_us: number;
}

function flushPromises() {
  return act(async () => {
    await Promise.resolve();
  });
}

describe("ReviewPanel", () => {
  let container: HTMLDivElement;
  let root: Root;
  let selectWordCalls: Array<number | null>;
  let seekToCalls: number[];
  let refreshCount: number;
  let closeCount: number;
  let selectWord: (index: number | null) => void;
  let refreshFromBackend: () => Promise<void>;
  let seekTo: (time: number) => void;
  let onClose: () => void;

  beforeEach(() => {
    mockInvoke.mockReset();
    selectWordCalls = [];
    seekToCalls = [];
    refreshCount = 0;
    closeCount = 0;
    selectWord = (index) => {
      selectWordCalls.push(index);
    };
    refreshFromBackend = async () => {
      refreshCount += 1;
    };
    seekTo = (time) => {
      seekToCalls.push(time);
    };
    onClose = () => {
      closeCount += 1;
    };

    useEditorStore.setState({
      words: [],
      timingContract: null,
      speakerNames: {},
      selectedIndex: null,
      selectionRange: null,
      highlightedIndices: [],
      highlightType: null,
      burnCaptions: false,
      selectWord,
      refreshFromBackend,
    });

    usePlayerStore.setState({
      mediaUrl: null,
      mediaType: null,
      mediaInfo: null,
      isPlaying: false,
      currentTime: 0,
      duration: 0,
      volume: 1,
      playbackRate: 1,
      seekVersion: 0,
      seekTarget: 0,
      seekTo,
    });

    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it("loads low-confidence words and seeks the first match when opened", async () => {
    const words: LowConfidenceWord[] = [
      {
        word_index: 4,
        text: "maybe",
        confidence: 0.42,
        start_us: 2_500_000,
        end_us: 2_900_000,
      },
    ];
    mockInvoke.mockResolvedValueOnce(words);

    await act(async () => {
      root.render(<ReviewPanel isOpen={true} onClose={onClose} />);
    });
    await flushPromises();

    expect(mockInvoke).toHaveBeenCalledWith("get_low_confidence_words", {
      threshold: 0.7,
    });
    expect(selectWordCalls).toContain(4);
    expect(seekToCalls).toContain(2.5);
    expect(container.textContent).toContain("1 of 1 to review");
    expect(container.textContent).toContain("maybe");
  });

  it("verifies the current word on Enter and advances to the next word", async () => {
    const words: LowConfidenceWord[] = [
      {
        word_index: 1,
        text: "first",
        confidence: 0.31,
        start_us: 1_000_000,
        end_us: 1_200_000,
      },
      {
        word_index: 3,
        text: "second",
        confidence: 0.51,
        start_us: 3_000_000,
        end_us: 3_200_000,
      },
    ];
    mockInvoke.mockResolvedValueOnce(words).mockResolvedValueOnce(true);

    await act(async () => {
      root.render(<ReviewPanel isOpen={true} onClose={onClose} />);
    });
    await flushPromises();

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    });
    await flushPromises();

    expect(mockInvoke).toHaveBeenLastCalledWith("verify_word", { index: 1 });
    expect(refreshCount).toBe(1);
    expect(selectWordCalls[selectWordCalls.length - 1]).toBe(3);
    expect(seekToCalls[seekToCalls.length - 1]).toBe(3);
    expect(container.textContent).toContain("second");
  });

  it("supports tab navigation and closes on escape", async () => {
    const words: LowConfidenceWord[] = [
      {
        word_index: 0,
        text: "alpha",
        confidence: 0.25,
        start_us: 0,
        end_us: 100_000,
      },
      {
        word_index: 2,
        text: "beta",
        confidence: 0.45,
        start_us: 500_000,
        end_us: 650_000,
      },
    ];
    mockInvoke.mockResolvedValueOnce(words);

    await act(async () => {
      root.render(<ReviewPanel isOpen={true} onClose={onClose} />);
    });
    await flushPromises();

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab" }));
    });
    await flushPromises();
    expect(selectWordCalls[selectWordCalls.length - 1]).toBe(2);

    await act(async () => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Tab", shiftKey: true }),
      );
    });
    await flushPromises();
    expect(selectWordCalls[selectWordCalls.length - 1]).toBe(0);

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(closeCount).toBe(1);
  });
});
