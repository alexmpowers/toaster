import React, { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, CheckCheck, Eye, SkipForward } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { Button } from "@/components/ui/Button";
import { useEditorStore } from "@/stores/editorStore";
import { usePlayerStore } from "@/stores/playerStore";

interface LowConfidenceWord {
  word_index: number;
  text: string;
  confidence: number;
  start_us: number;
  end_us: number;
}

interface ReviewPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

type ReviewStatus = "idle" | "empty" | "complete";

const REVIEW_THRESHOLD = 0.7;

function isEditableTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      target.tagName === "INPUT" ||
      target.tagName === "TEXTAREA" ||
      target.tagName === "SELECT")
  );
}

const ReviewPanel: React.FC<ReviewPanelProps> = ({ isOpen, onClose }) => {
  const { t } = useTranslation();
  const [words, setWords] = useState<LowConfidenceWord[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [status, setStatus] = useState<ReviewStatus>("idle");
  const { selectWord, refreshFromBackend } = useEditorStore(
    useShallow((state) => ({
      selectWord: state.selectWord,
      refreshFromBackend: state.refreshFromBackend,
    })),
  );
  const seekTo = usePlayerStore((state) => state.seekTo);

  const loadWords = useCallback(async () => {
    try {
      const result = await invoke<LowConfidenceWord[]>(
        "get_low_confidence_words",
        {
          threshold: REVIEW_THRESHOLD,
        },
      );
      setWords(result);
      setCurrentIndex(0);
      setStatus(result.length === 0 ? "empty" : "idle");
    } catch (error) {
      console.error("Failed to load low-confidence words", error);
      setWords([]);
      setCurrentIndex(0);
      setStatus("empty");
    }
  }, []);

  useEffect(() => {
    if (!isOpen) {
      setWords([]);
      setCurrentIndex(0);
      setStatus("idle");
      return;
    }

    void loadWords();
  }, [isOpen, loadWords]);

  const currentWord = useMemo(
    () => words[currentIndex] ?? null,
    [currentIndex, words],
  );

  useEffect(() => {
    if (!isOpen || !currentWord) {
      return;
    }

    selectWord(currentWord.word_index);
    seekTo(currentWord.start_us / 1_000_000);
  }, [currentWord, isOpen, seekTo, selectWord]);

  const moveSelection = useCallback(
    (delta: -1 | 1) => {
      if (words.length === 0) {
        return;
      }
      setCurrentIndex((previous) => {
        const next = previous + delta;
        if (next < 0) {
          return 0;
        }
        if (next >= words.length) {
          return words.length - 1;
        }
        return next;
      });
    },
    [words.length],
  );

  const handleSkip = useCallback(() => {
    moveSelection(1);
  }, [moveSelection]);

  const handleVerify = useCallback(async () => {
    if (!currentWord) {
      return;
    }

    try {
      const updated = await invoke<boolean>("verify_word", {
        index: currentWord.word_index,
      });
      if (!updated) {
        return;
      }

      await refreshFromBackend();
      const nextWords = words.filter((_, index) => index !== currentIndex);
      setWords(nextWords);
      setCurrentIndex(
        Math.min(currentIndex, Math.max(nextWords.length - 1, 0)),
      );
      setStatus(nextWords.length === 0 ? "complete" : "idle");
    } catch (error) {
      console.error("Failed to verify word", error);
    }
  }, [currentIndex, currentWord, refreshFromBackend, words]);

  const handleVerifyAll = useCallback(async () => {
    if (words.length === 0) {
      return;
    }

    try {
      await invoke<number>("verify_all_words", {
        indices: words.map((word) => word.word_index),
      });
      await refreshFromBackend();
      setWords([]);
      setCurrentIndex(0);
      setStatus("complete");
    } catch (error) {
      console.error("Failed to verify all words", error);
    }
  }, [refreshFromBackend, words]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (isEditableTarget(event.target)) {
        return;
      }

      if (event.key === "Tab") {
        event.preventDefault();
        if (event.shiftKey) {
          moveSelection(-1);
        } else {
          handleSkip();
        }
        return;
      }

      if (event.key === "Enter") {
        event.preventDefault();
        void handleVerify();
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleSkip, handleVerify, isOpen, moveSelection, onClose]);

  if (!isOpen) {
    return null;
  }

  const emptyMessage =
    status === "complete"
      ? t("editor.reviewComplete")
      : t("editor.noLowConfidenceWords");

  return (
    <div className="rounded-lg border border-mid-gray/20 bg-background p-3">
      <div className="flex flex-wrap items-center gap-2 text-sm">
        <Eye className="h-4 w-4 shrink-0 text-mid-gray" />
        <span className="font-medium">{t("editor.reviewMode")}</span>
        <span className="text-xs text-mid-gray">
          {t("editor.confidenceThreshold")}:{" "}
          {Math.round(REVIEW_THRESHOLD * 100)}%
        </span>
        {currentWord ? (
          <>
            <span className="text-mid-gray">
              {t("editor.reviewProgress", {
                current: currentIndex + 1,
                total: words.length,
              })}
            </span>
            <span className="rounded bg-mid-gray/10 px-2 py-0.5 font-mono text-xs">
              {currentWord.text}
              <span className="ml-1 text-mid-gray">
                ({Math.round(currentWord.confidence * 100)}%)
              </span>
            </span>
          </>
        ) : (
          <span className="text-mid-gray">{emptyMessage}</span>
        )}
        <div className="ml-auto flex flex-wrap gap-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={handleSkip}
            disabled={!currentWord}
            title={t("editor.skipWord")}
            className="inline-flex items-center gap-1.5"
          >
            <SkipForward className="h-3.5 w-3.5" />
            {t("editor.skipWord")}
          </Button>
          <Button
            type="button"
            variant="primary-soft"
            size="sm"
            onClick={() => void handleVerify()}
            disabled={!currentWord}
            title={t("editor.verifyWord")}
            className="inline-flex items-center gap-1.5"
          >
            <Check className="h-3.5 w-3.5" />
            {t("editor.verifyWord")}
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            onClick={() => void handleVerifyAll()}
            disabled={words.length === 0}
            title={t("editor.verifyAll")}
            className="inline-flex items-center gap-1.5"
          >
            <CheckCheck className="h-3.5 w-3.5" />
            {t("editor.verifyAll")}
          </Button>
        </div>
      </div>
    </div>
  );
};

export default ReviewPanel;
