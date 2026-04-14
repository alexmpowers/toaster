import React, { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { AudioLines, Trash2, VolumeX, ChevronDown, ChevronUp } from "lucide-react";
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
}

interface FillerDashboardProps {
  className?: string;
}

const FillerDashboard: React.FC<FillerDashboardProps> = ({ className = "" }) => {
  const { t } = useTranslation();
  const { words, setWords, selectWord } = useEditorStore();
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

  const handleDeleteFillers = useCallback(async () => {
    try {
      const count = await invoke<number>("delete_fillers", {});
      if (count > 0) {
        const updated = await invoke<typeof words>("editor_get_words", {});
        await setWords(updated);
      }
      setAnalysis(null);
    } catch (err) {
      console.error("Delete fillers failed:", err);
    }
  }, [words, setWords]);

  const handleSilencePauses = useCallback(async () => {
    try {
      const count = await invoke<number>("silence_pauses", {});
      if (count > 0) {
        const updated = await invoke<typeof words>("editor_get_words", {});
        await setWords(updated);
      }
      setAnalysis(null);
    } catch (err) {
      console.error("Silence pauses failed:", err);
    }
  }, [words, setWords]);

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

        {analysis && analysis.filler_count > 0 && (
          <button
            onClick={handleDeleteFillers}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-red-900/30 border border-red-500/30 rounded-lg text-xs text-red-400 hover:bg-red-900/50 transition-colors"
          >
            <Trash2 size={14} />
            {t("editor.deleteFillers", { count: analysis.filler_count })}
          </button>
        )}

        {analysis && analysis.pause_count > 0 && (
          <button
            onClick={handleSilencePauses}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-yellow-900/30 border border-yellow-500/30 rounded-lg text-xs text-yellow-400 hover:bg-yellow-900/50 transition-colors"
          >
            <VolumeX size={14} />
            {t("editor.silencePauses", { count: analysis.pause_count })}
          </button>
        )}
      </div>

      {/* Summary bar */}
      {analysis && (
        <div className="rounded-lg border border-mid-gray/20 bg-[#1E1E1E] overflow-hidden">
          <button
            onClick={() => setExpanded(!expanded)}
            className="w-full flex items-center justify-between px-3 py-2 text-xs text-mid-gray hover:bg-mid-gray/5 transition-colors"
          >
            <span>
              {t("editor.fillerResults", {
                fillers: analysis.filler_count,
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
                        "{text}" × {count}
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
                          {durationSec}s {afterWord ? `after "${afterWord.text}"` : ""}
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
