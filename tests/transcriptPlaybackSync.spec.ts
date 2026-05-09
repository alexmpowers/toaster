import { expect, test, type Page } from "@playwright/test";

const INIT_SCRIPT = `
  window.__TAURI_OS_PLUGIN_INTERNALS__ = {
    platform: "windows", version: "10.0", os_type: "windows_nt",
    family: "windows", arch: "x86_64", exe_extension: "exe",
    eol: "\\r\\n", hostname: "test-host", locale: "en-US",
  };
  var _cbId = 0;
  window.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main" },
    },
    transformCallback: function() { return _cbId++; },
    invoke: async function(cmd) {
      if (cmd === "plugin:event|listen") return 0;
      if (cmd === "plugin:event|unlisten") return;
      if (cmd === "plugin:app|version") return "0.1.0";
      if (cmd === "get_app_settings" || cmd === "get_default_settings") return {};
      if (cmd === "list_models") return [];
      if (cmd === "get_current_model") return "tiny";
      if (cmd === "model_is_downloaded") return true;
      if (cmd === "model_is_loaded") return true;
      if (cmd === "media_get_current") return null;
      if (cmd === "editor_get_projection") {
        return {
          words: [],
          timing_contract: null,
        };
      }
      if (cmd === "editor_get_keep_segments") return [];
      return null;
    },
    convertFileSrc: function(p) { return p; },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: function() {} };
`;

async function setup(page: Page) {
  await page.addInitScript(INIT_SCRIPT);
  await page.goto("/");
}

test.describe("Transcript playback sync", () => {
  test("karaoke highlight marks the active transcript word during playback", async ({
    page,
  }) => {
    await setup(page);

    const className = await page.evaluate(async () => {
      const ReactModule = await import("/node_modules/.vite/deps/react.js");
      const React = ReactModule.default ?? ReactModule;
      const ReactDomClientModule = await import(
        "/node_modules/.vite/deps/react-dom_client.js"
      );
      const createRoot =
        ReactDomClientModule.createRoot ??
        ReactDomClientModule.default?.createRoot;
      const { default: TranscriptEditor } = await import(
        "/src/components/editor/TranscriptEditor.tsx"
      );
      const { useEditorStore } = await import("/src/stores/editorStore.ts");

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
        selectedIndex: 0,
        selectionRange: null,
        highlightedIndices: [],
        highlightType: null,
      });

      const host = document.createElement("div");
      host.id = "transcript-playback-fixture";
      document.body.appendChild(host);
      const root = createRoot(host);
      root.render(
        React.createElement(TranscriptEditor, {
          activePlaybackIndex: 1,
          isPlaying: true,
        }),
      );

      await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
      const activeWord = Array.from(host.querySelectorAll("span")).find(
        (element) => element.textContent === "beta",
      );
      return activeWord?.className ?? "";
    });

    expect(className).toContain("animate-pulse");
  });

  test("waveform clicks seek and select the word at that time", async ({ page }) => {
    await setup(page);

    await page.evaluate(async () => {
      const ReactModule = await import("/node_modules/.vite/deps/react.js");
      const React = ReactModule.default ?? ReactModule;
      const ReactDomClientModule = await import(
        "/node_modules/.vite/deps/react-dom_client.js"
      );
      const createRoot =
        ReactDomClientModule.createRoot ??
        ReactDomClientModule.default?.createRoot;
      const { default: Waveform } = await import(
        "/src/components/player/Waveform.tsx"
      );

      const host = document.createElement("div");
      host.id = "waveform-playback-fixture";
      host.style.width = "500px";
      document.body.appendChild(host);

      (window as typeof window & {
        __waveformSeek?: number;
        __waveformSelection?: number | null;
      }).__waveformSeek = -1;
      (window as typeof window & {
        __waveformSeek?: number;
        __waveformSelection?: number | null;
      }).__waveformSelection = null;

      const root = createRoot(host);
      root.render(
        React.createElement(Waveform, {
          audioUrl: "mock://audio",
          currentTime: 0,
          duration: 1,
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
              end_us: 600_000,
              deleted: false,
              silenced: false,
              confidence: 1,
              speaker_id: -1,
            },
            {
              text: "gamma",
              start_us: 600_000,
              end_us: 1_000_000,
              deleted: false,
              silenced: false,
              confidence: 1,
              speaker_id: -1,
            },
          ],
          onSeek: (time: number) => {
            (window as typeof window & { __waveformSeek?: number }).__waveformSeek =
              time;
          },
          onWordSelect: (index: number) => {
            (
              window as typeof window & { __waveformSelection?: number | null }
            ).__waveformSelection = index;
          },
        }),
      );

      await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
    });

    await page.locator("#waveform-playback-fixture canvas").click({
      position: { x: 250, y: 20 },
    });

    const result = await page.evaluate(() => {
      const testWindow = window as typeof window & {
        __waveformSeek?: number;
        __waveformSelection?: number | null;
      };
      return {
        seek: testWindow.__waveformSeek,
        selection: testWindow.__waveformSelection,
      };
    });

    expect(result.seek).toBeGreaterThan(0.45);
    expect(result.seek).toBeLessThan(0.55);
    expect(result.selection).toBe(1);
  });
});
