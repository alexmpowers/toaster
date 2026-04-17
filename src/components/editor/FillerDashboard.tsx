import React, { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { AudioLines, ChevronDown, ChevronUp } from "lucide-react";
import { useEditorStore } from "@/stores/editorStore";

interface PauseInfo {
  after_word_index: number;
  gap_duration_us: number;
}

interface FillerAnalysis {
  filler_indices: number[];
  pauses: PauseInfo[];
  filler_count: number;
  pause_count: number;
  duplicate_indices: number[];
  duplicate_count: number;
}

interface FillerDashboardProps {
  className?: string;
}

const FillerDashboard: React.FC<FillerDashboardProps> = ({ className = "" }) => {
  const { t } = useTranslation();
  const { words, refreshFromBackend, selectWord } = useEditorStore();
  const [analysis, setAnalysis] = useState<FillerAnalysis | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [isAnalyzing, setIsAnalyzing] = useState(false);

  const handleAnalyze = useCallback(async () => {
    setIsAnalyzing(true);
    try {
      const result = await invoke<FillerAnalysis>("analyze_fillers", {});
      setAnalysis(result);
      setExpanded(true);
    } catch (err) {
      console.error("Filler analysis failed:", err);
    } finally {
      setIsAnalyzing(false);
    }
  }, []);


  const handleRemoveAll = useCallback(async () => {
    if (!analysis) return;
    try {
      // Use iterative cleanup — handles cascading duplicates after filler removal
      await invoke("cleanup_all", {});
      await refreshFromBackend();
      handleAnalyze();
    } catch (err) {
      console.error("Cleanup failed:", err);
    }
  }, [analysis, refreshFromBackend, handleAnalyze]);

  const handleClickFiller = useCallback(
    (index: number) => {
      selectWord(index);
    },
    [selectWord],
  );

  // Group fillers by word text for summary
  const fillerGroups = analysis
    ? analysis.filler_indices.reduce(
        (acc, idx) => {
          const text = words[idx]?.text?.toLowerCase() ?? "?";
          acc[text] = (acc[text] || 0) + 1;
          return acc;
        },
        {} as Record<string, number>,
      )
    : {};

  const sortedGroups = Object.entries(fillerGroups).sort((a, b) => b[1] - a[1]);

  // Group duplicates by word text for summary
  const duplicateGroups = analysis
    ? analysis.duplicate_indices.reduce(
        (acc, idx) => {
          const text = words[idx]?.text?.toLowerCase() ?? "?";
          acc[text] = (acc[text] || 0) + 1;
          return acc;
        },
        {} as Record<string, number>,
      )
    : {};

  const sortedDuplicateGroups = Object.entries(duplicateGroups).sort((a, b) => b[1] - a[1]);

  if (words.length === 0) return null;

  return (
    <div className={`space-y-2 ${className}`}>
      {/* Analyze button */}
      <div className="flex items-center gap-2">
        <button
          onClick={handleAnalyze}
          disabled={isAnalyzing}
          className="flex items-center gap-1.5 px-3 py-1.5 bg-background border border-mid-gray/20 rounded-lg text-xs hover:bg-mid-gray/10 transition-colors disabled:opacity-50"
        >
          <AudioLines size={14} />
          {isAnalyzing ? t("editor.analyzing") : t("editor.analyzeFillers")}
        </button>
      </div>

      {/* Remove All button — single action for all cleanup */}
      {analysis &&
        (analysis.filler_count > 0 || analysis.duplicate_count > 0 || analysis.pauses.length > 0) && (
          <button
            onClick={handleRemoveAll}
            className="w-full px-3 py-2 rounded bg-[#E8A838] text-black text-sm font-medium hover:bg-[#E8A838]/80"
          >
            {t("editor.removeAll", {
              count: analysis.filler_count + analysis.duplicate_count + analysis.pauses.length,
            })}
          </button>
        )}

      {/* Summary bar */}
      {analysis && (
        <div className="rounded-lg border border-mid-gray/20 bg-[#1E1E1E] overflow-hidden">
          <button
            onClick={() => setExpanded(!expanded)}
            className="w-full flex items-center justify-between px-3 py-2 text-xs text-mid-gray hover:bg-mid-gray/5 transition-colors"
          >
            <span>
              {analysis.filler_count === 0 && analysis.duplicate_count === 0 && analysis.pause_count === 0
                ? t("editor.noIssuesFound")
                : t("editor.cleanupMetrics", {
                    fillers: analysis.filler_count,
                    duplicates: analysis.duplicate_count,
                    pauses: analysis.pause_count,
                  })}
            </span>
            {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>

          {expanded && (
            <div className="border-t border-mid-gray/10 px-3 py-2 space-y-2">
              {/* Filler word groups */}
              {sortedGroups.length > 0 && (
                <div className="space-y-1">
                  <p className="text-[10px] uppercase tracking-wider text-mid-gray/60">
                    {t("editor.fillerWords")}
                  </p>
                  <div className="flex flex-wrap gap-1">
                    {sortedGroups.map(([text, count]) => (
                      <span
                        key={text}
                        className="px-2 py-0.5 rounded-full bg-red-900/20 text-red-400 text-[11px] border border-red-500/20"
                      >
                        {t("editor.fillerGroupChip", { text, count })}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Individual filler list */}
              {analysis.filler_indices.length > 0 && (
                <div className="space-y-1">
                  <p className="text-[10px] uppercase tracking-wider text-mid-gray/60">
                    {t("editor.clickToLocate")}
                  </p>
                  <div className="flex flex-wrap gap-1 max-h-20 overflow-y-auto">
                    {analysis.filler_indices.map((idx) => {
                      const w = words[idx];
                      if (!w) return null;
                      return (
                        <button
                          key={idx}
                          onClick={() => handleClickFiller(idx)}
                          className="px-1.5 py-0.5 rounded text-[11px] bg-red-900/10 text-red-300 hover:bg-red-900/30 transition-colors border border-red-500/10"
                          title={`${(w.start_us / 1_000_000).toFixed(1)}s`}
                        >
                          {w.text}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Duplicate word groups */}
              {sortedDuplicateGroups.length > 0 && (
                <div className="space-y-1">
                  <p className="text-[10px] uppercase tracking-wider text-mid-gray/60">
                    {t("editor.duplicateWords")}
                  </p>
                  <div className="flex flex-wrap gap-1">
                    {sortedDuplicateGroups.map(([text, count]) => (
                      <span
                        key={text}
                        className="px-2 py-0.5 rounded-full bg-orange-900/20 text-orange-400 text-[11px] border border-orange-500/20"
                      >
                        {t("editor.fillerGroupChip", { text, count })}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Individual duplicate list */}
              {analysis.duplicate_indices.length > 0 && (
                <div className="space-y-1">
                  <p className="text-[10px] uppercase tracking-wider text-mid-gray/60">
                    {t("editor.clickToLocate")}
                  </p>
                  <div className="flex flex-wrap gap-1 max-h-20 overflow-y-auto">
                    {analysis.duplicate_indices.map((idx) => {
                      const w = words[idx];
                      if (!w) return null;
                      return (
                        <button
                          key={idx}
                          onClick={() => handleClickFiller(idx)}
                          className="px-1.5 py-0.5 rounded text-[11px] bg-orange-900/10 text-orange-300 hover:bg-orange-900/30 transition-colors border border-orange-500/10"
                          title={`${(w.start_us / 1_000_000).toFixed(1)}s`}
                        >
                          {w.text}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Pause list */}
              {analysis.pauses.length > 0 && (
                <div className="space-y-1">
                  <p className="text-[10px] uppercase tracking-wider text-mid-gray/60">
                    {t("editor.pausesDetected")}
                  </p>
                  <div className="flex flex-wrap gap-1 max-h-20 overflow-y-auto">
                    {analysis.pauses.map((p, i) => {
                      const durationSec = (p.gap_duration_us / 1_000_000).toFixed(1);
                      const afterWord = words[p.after_word_index];
                      return (
                        <button
                          key={i}
                          onClick={() => handleClickFiller(p.after_word_index)}
                          className="px-1.5 py-0.5 rounded text-[11px] bg-yellow-900/10 text-yellow-300 hover:bg-yellow-900/30 transition-colors border border-yellow-500/10"
                        >
                          {afterWord
                            ? t("editor.pauseChipWithWord", { duration: durationSec, word: afterWord.text })
                            : t("editor.pauseChip", { duration: durationSec })}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default FillerDashboard;
