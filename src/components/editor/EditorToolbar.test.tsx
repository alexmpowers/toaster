import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("@/hooks/useSettings", () => ({
  useSettings: () => ({
    settings: { loudness_target: "off" },
    updateSetting: vi.fn(),
    isUpdating: () => false,
  }),
}));

const { default: EditorToolbar } = await import("./EditorToolbar");

type EditorToolbarProps = {
  words: Array<{ text: string }>;
  burnCaptions: boolean;
  onBurnCaptionsChange: (next: boolean) => void;
  normalizeAudio: boolean;
  onNormalizeAudioToggle: () => void;
};

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("EditorToolbar", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
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

  it("renders export controls without review or cleanup actions", async () => {
    const Toolbar = EditorToolbar as React.FC<EditorToolbarProps>;

    await act(async () => {
      root.render(
        <Toolbar
          words={[{ text: "hello" }]}
          burnCaptions={false}
          onBurnCaptionsChange={() => {}}
          normalizeAudio={false}
          onNormalizeAudioToggle={() => {}}
        />,
      );
    });

    expect(container.textContent).toContain("editor.addCaptions");
    expect(container.textContent).toContain("editor.normalizeAudio");
    expect(container.textContent).not.toContain("editor.reviewMode");
    expect(container.textContent).not.toContain("editor.cleanupTitle");
    expect(container.textContent).not.toContain("editor.speakerPanel");
  });
});
