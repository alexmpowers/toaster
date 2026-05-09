import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { id?: number }) => {
      if (key === "editor.speaker" && options) {
        return `Speaker ${options.id}`;
      }
      return key;
    },
  }),
}));

vi.mock("./FindReplaceBar", () => ({
  default: () => null,
}));

vi.mock("./TranscriptContextMenu", () => ({
  default: () => null,
}));

const { useEditorStore } = await import("@/stores/editorStore");
const { default: TranscriptEditor } = await import("./TranscriptEditor");

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

type PlaybackAwareTranscriptEditorProps = {
  activePlaybackIndex?: number | null;
  isPlaying?: boolean;
  activeWordRef?: React.RefObject<HTMLSpanElement | null>;
  showSpeakers?: boolean;
  speakerNames?: Record<number, string>;
};

function noOpAsync() {
  return Promise.resolve();
}

describe("TranscriptEditor playback highlighting", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useEditorStore.setState({
      words: [
        {
          text: "alpha",
          start_us: 0,
          end_us: 200_000,
          deleted: false,
          silenced: false,
          confidence: 1,
          speaker_id: -1,
        },
        {
          text: "beta",
          start_us: 200_000,
          end_us: 450_000,
          deleted: false,
          silenced: false,
          confidence: 1,
          speaker_id: -1,
        },
      ],
      timingContract: null,
      speakerNames: {},
      selectedIndex: 0,
      selectionRange: null,
      highlightedIndices: [],
      highlightType: null,
      burnCaptions: false,
      setWords: async () => noOpAsync(),
      deleteWord: async () => noOpAsync(),
      restoreWord: async () => noOpAsync(),
      deleteRange: async () => noOpAsync(),
      restoreAll: async () => noOpAsync(),
      splitWord: async () => noOpAsync(),
      silenceWord: async () => noOpAsync(),
      undo: async () => noOpAsync(),
      redo: async () => noOpAsync(),
      refreshFromBackend: async () => noOpAsync(),
      getKeepSegments: async () => [],
      selectWord: vi.fn(),
      setSelectionRange: vi.fn(),
      setHighlightedIndices: vi.fn(),
      clearHighlights: vi.fn(),
      setBurnCaptions: vi.fn(),
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

  it("applies a karaoke-style class to the active playback word", async () => {
    const PlaybackAwareTranscriptEditor =
      TranscriptEditor as React.FC<PlaybackAwareTranscriptEditorProps>;

    await act(async () => {
      root.render(
        <PlaybackAwareTranscriptEditor activePlaybackIndex={1} isPlaying={true} />,
      );
    });

    const activeWord = Array.from(container.querySelectorAll("span")).find(
      (element) => element.textContent === "beta",
    );

    expect(activeWord?.className).toContain("animate-pulse");
  });

  it("points the activeWordRef at the playing word while playback is running", async () => {
    const activeWordRef = { current: null } as React.RefObject<HTMLSpanElement | null>;
    const PlaybackAwareTranscriptEditor =
      TranscriptEditor as React.FC<PlaybackAwareTranscriptEditorProps>;

    await act(async () => {
      root.render(
        <PlaybackAwareTranscriptEditor
          activePlaybackIndex={1}
          isPlaying={true}
          activeWordRef={activeWordRef}
        />,
      );
    });

    expect(activeWordRef.current?.textContent).toBe("beta");
  });

  it("renders speaker section headers using provided speaker names", async () => {
    useEditorStore.setState({
      words: [
        {
          text: "alpha",
          start_us: 0,
          end_us: 200_000,
          deleted: false,
          silenced: false,
          confidence: 1,
          speaker_id: 0,
        },
        {
          text: "beta",
          start_us: 250_000,
          end_us: 450_000,
          deleted: false,
          silenced: false,
          confidence: 1,
          speaker_id: 1,
        },
      ],
    });
    const PlaybackAwareTranscriptEditor =
      TranscriptEditor as React.FC<PlaybackAwareTranscriptEditorProps>;

    await act(async () => {
      root.render(
        <PlaybackAwareTranscriptEditor
          showSpeakers={true}
          speakerNames={{ 0: "Host", 1: "Guest" }}
        />,
      );
    });

    expect(container.textContent).toContain("Host");
    expect(container.textContent).toContain("Guest");
  });
});
