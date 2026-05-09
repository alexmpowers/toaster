import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useShallow } from "zustand/react/shallow";
import { commands } from "@/bindings";
import { useEditorStore } from "@/stores/editorStore";
import { usePlayerStore } from "@/stores/playerStore";
import { unwrapResult } from "@/components/editor/EditorView.util";

/**
 * Centralises all global keyboard shortcuts for the editor.
 * Extracted from EditorView to respect the 800-line file cap.
 */
export function useEditorKeyboard(
  setShortcutsOpen: React.Dispatch<React.SetStateAction<boolean>>,
) {
  const { t } = useTranslation();

  const {
    deleteWord,
    deleteRange,
    silenceWord,
    splitWord,
    undo,
    redo,
    selectWord,
    setSelectionRange,
    clearHighlights,
    refreshFromBackend,
  } = useEditorStore(
    useShallow((s) => ({
      deleteWord: s.deleteWord,
      deleteRange: s.deleteRange,
      silenceWord: s.silenceWord,
      splitWord: s.splitWord,
      undo: s.undo,
      redo: s.redo,
      selectWord: s.selectWord,
      setSelectionRange: s.setSelectionRange,
      clearHighlights: s.clearHighlights,
      refreshFromBackend: s.refreshFromBackend,
    })),
  );

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't capture when typing in input/textarea
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      const { setPlaying, isPlaying } = usePlayerStore.getState();
      const {
        selectedIndex: selIdx,
        selectionRange: selRange,
        highlightedIndices: hlIndices,
        highlightType: hlType,
      } = useEditorStore.getState();

      // Help overlay: "?" (Shift+/) or F1
      if ((e.key === "?" || e.key === "F1") && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        setShortcutsOpen((v) => !v);
        return;
      }

      if (e.key === " " && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        setPlaying(!isPlaying);
      } else if (e.key === "k" && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
        e.preventDefault();
        setPlaying(!isPlaying);
      } else if (e.key === "j" && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
        e.preventDefault();
        const store = usePlayerStore.getState();
        store.setPlaying(false);
        store.seekTo(Math.max(0, store.currentTime - 10));
      } else if (e.key === "l" && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
        e.preventDefault();
        const store = usePlayerStore.getState();
        store.setPlaying(false);
        store.seekTo(Math.min(store.duration, store.currentTime + 10));
      } else if (
        (e.key === "Delete" || e.key === "Backspace") &&
        !e.ctrlKey &&
        !e.metaKey
      ) {
        e.preventDefault();
        if (hlIndices.length > 0) {
          if (hlType === "cleanup") {
            return;
          }
          if (hlType === "filler") {
            commands
              .deleteFillers()
              .then(async (result) => {
                const count = unwrapResult(result);
                if (count > 0) {
                  await refreshFromBackend();
                  toast.success(t("editor.cleanup.fillersOnly", { count }));
                } else {
                  toast.info(t("editor.cleanup.empty"));
                }
                clearHighlights();
              })
              .catch((err) => {
                console.error("Failed to delete fillers:", err);
                toast.error(t("editor.cleanup.failed"));
                clearHighlights();
              });
          } else {
            (async () => {
              for (const idx of hlIndices) {
                await deleteWord(idx);
              }
              clearHighlights();
            })();
          }
        } else if (selRange) {
          deleteRange(selRange[0], selRange[1]);
        } else if (selIdx !== null) {
          deleteWord(selIdx);
        }
      } else if (e.key === "ArrowLeft" && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        const ws = useEditorStore.getState().words;
        const sel = useEditorStore.getState().selectedIndex;
        if (sel !== null && sel > 0) {
          let prev = sel - 1;
          while (prev >= 0 && ws[prev]?.deleted) prev--;
          if (prev >= 0) {
            if (e.shiftKey) {
              const range = useEditorStore.getState().selectionRange;
              const start = range
                ? Math.min(range[0], prev)
                : Math.min(sel, prev);
              const end = range
                ? Math.max(range[1], prev)
                : Math.max(sel, prev);
              setSelectionRange([start, end]);
            } else {
              selectWord(prev);
              setSelectionRange(null);
            }
            const w = ws[prev];
            if (w) usePlayerStore.getState().seekTo(w.start_us / 1_000_000);
          }
        }
      } else if (e.key === "ArrowRight" && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        const ws = useEditorStore.getState().words;
        const sel = useEditorStore.getState().selectedIndex;
        if (sel !== null && sel < ws.length - 1) {
          let next = sel + 1;
          while (next < ws.length && ws[next]?.deleted) next++;
          if (next < ws.length) {
            if (e.shiftKey) {
              const range = useEditorStore.getState().selectionRange;
              const start = range
                ? Math.min(range[0], next)
                : Math.min(sel, next);
              const end = range
                ? Math.max(range[1], next)
                : Math.max(sel, next);
              setSelectionRange([start, end]);
            } else {
              selectWord(next);
              setSelectionRange(null);
            }
            const w = ws[next];
            if (w) usePlayerStore.getState().seekTo(w.start_us / 1_000_000);
          }
        }
      } else if (e.key === "ArrowLeft" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        const store = usePlayerStore.getState();
        store.seekTo(Math.max(0, store.currentTime - 5));
      } else if (e.key === "ArrowRight" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        const store = usePlayerStore.getState();
        store.seekTo(Math.min(store.duration, store.currentTime + 5));
      } else if (e.key === "d" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        if (selRange) {
          deleteRange(selRange[0], selRange[1]);
        } else if (selIdx !== null) {
          deleteWord(selIdx);
        }
      } else if (e.key === "m" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        if (selIdx !== null) {
          silenceWord(selIdx);
        }
      } else if (e.key === "S" && (e.ctrlKey || e.metaKey) && e.shiftKey) {
        e.preventDefault();
        if (selIdx !== null) {
          const w = useEditorStore.getState().words[selIdx];
          if (w && w.text.length > 1) {
            splitWord(selIdx, Math.floor(w.text.length / 2));
          }
        }
      } else if (e.key === "a" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        const ws = useEditorStore.getState().words;
        if (ws.length > 0) {
          selectWord(0);
          setSelectionRange([0, ws.length - 1]);
        }
      } else if (e.key === "Escape") {
        selectWord(null);
        setSelectionRange(null);
        clearHighlights();
      } else if (e.key === "z" && (e.ctrlKey || e.metaKey) && e.shiftKey) {
        e.preventDefault();
        redo();
      } else if (e.key === "z" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        undo();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    deleteWord,
    deleteRange,
    silenceWord,
    splitWord,
    undo,
    redo,
    selectWord,
    setSelectionRange,
    clearHighlights,
    refreshFromBackend,
    setShortcutsOpen,
    t,
  ]);
}
