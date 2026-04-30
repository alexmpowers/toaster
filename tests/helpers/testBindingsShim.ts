import { Page } from "@playwright/test";

/**
 * Inject a window.__toasterTestApi shim that exposes the Tauri commands
 * without requiring Vite alias resolution inside page.evaluate().
 *
 * Usage:
 *   await injectBindingsShim(page);
 *   const result = await page.evaluate(() => {
 *     return window.__toasterTestApi.commands.exportTranscriptToFile(...);
 *   });
 */

const BINDINGS_SHIM = `
  // Expose all Tauri commands via window.__toasterTestApi for tests.
  // This allows page.evaluate(() => window.__toasterTestApi.commands.*) without @/ imports.
  window.__toasterTestApi = {
    commands: {
      // Commands that appear in tests
      changeAppLanguageSetting: async (language) => {
        return await window.__TAURI_INTERNALS__.invoke("change_app_language_setting", { language });
      },
      changeThemeSetting: async (theme) => {
        return await window.__TAURI_INTERNALS__.invoke("change_theme_setting", { theme });
      },
      getAppSettings: async () => {
        try {
          return await window.__TAURI_INTERNALS__.invoke("get_app_settings");
        } catch (e) {
          throw new Error(String(e));
        }
      },
      getDefaultSettings: async () => {
        try {
          return await window.__TAURI_INTERNALS__.invoke("get_default_settings");
        } catch (e) {
          throw new Error(String(e));
        }
      },
      updateAppSettings: async (settings) => {
        return await window.__TAURI_INTERNALS__.invoke("update_app_settings", { settings });
      },
      exportTranscriptToFile: async (format, path, maxCharsPerLine, includeSilenced) => {
        return await window.__TAURI_INTERNALS__.invoke("export_transcript_to_file", { format, path, maxCharsPerLine, includeSilenced });
      },
      exportTranscript: async (format, maxCharsPerLine, includeSilenced) => {
        return await window.__TAURI_INTERNALS__.invoke("export_transcript", { format, maxCharsPerLine, includeSilenced });
      },
      editorSetWords: async (words) => {
        return await window.__TAURI_INTERNALS__.invoke("editor_set_words", { words });
      },
      editorDeleteWord: async (id) => {
        return await window.__TAURI_INTERNALS__.invoke("editor_delete_word", { index: id });
      },
      editorUndo: async () => {
        return await window.__TAURI_INTERNALS__.invoke("editor_undo");
      },
      editorRedo: async () => {
        return await window.__TAURI_INTERNALS__.invoke("editor_redo");
      },
      editorGetWords: async () => {
        return await window.__TAURI_INTERNALS__.invoke("editor_get_words");
      },
      editorGetProjection: async () => {
        return await window.__TAURI_INTERNALS__.invoke("editor_get_projection");
      },
    },
  };
`;

export async function injectBindingsShim(page: Page) {
  await page.addInitScript(BINDINGS_SHIM);
}

// Type declaration for TypeScript
declare global {
  interface Window {
    __toasterTestApi?: {
      commands: Record<string, (...args: unknown[]) => Promise<unknown>>;
    };
  }
}
