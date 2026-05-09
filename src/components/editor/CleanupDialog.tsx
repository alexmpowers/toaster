import React, { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Eye, Sparkles, Wand2 } from "lucide-react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/Button";
import { useEditorStore } from "@/stores/editorStore";

type CleanupPreset = "Gentle" | "Balanced" | "Aggressive";
type CleanupHighlightType = "filler" | "duplicate" | "pause" | "cleanup";

type CleanupActionType =
  | "DeleteFiller"
  | "DeleteDuplicate"
  | "SilencePause"
  | "TrimPause"
  | "RemoveSilence";

interface CleanupAction {
  word_index: number;
  word_text: string;
  action: CleanupActionType;
  start_us: number;
  end_us: number;
}

interface CleanupPlan {
  source_revision: number;
  actions: CleanupAction[];
  filler_count: number;
  duplicate_count: number;
  pause_count: number;
  trim_count: number;
  silence_count: number;
  total_affected: number;
  estimated_duration_saved_us: number;
  passes: number;
}

interface CleanupDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

const PRESETS: readonly CleanupPreset[] = ["Gentle", "Balanced", "Aggressive"];

function getHighlightType(actions: CleanupAction[]): CleanupHighlightType {
  const kinds = new Set(actions.map((action) => action.action));
  if (kinds.size === 1) {
    if (kinds.has("DeleteFiller")) {
      return "filler";
    }
    if (kinds.has("DeleteDuplicate")) {
      return "duplicate";
    }
    if (kinds.has("SilencePause")) {
      return "pause";
    }
  }
  return "cleanup";
}

const CleanupDialog: React.FC<CleanupDialogProps> = ({ isOpen, onClose }) => {
  const { t } = useTranslation();
  const [preset, setPreset] = useState<CleanupPreset>("Balanced");
  const [plan, setPlan] = useState<CleanupPlan | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const refreshFromBackend = useEditorStore(
    (state) => state.refreshFromBackend,
  );
  const setHighlightedIndices = useEditorStore(
    (state) => state.setHighlightedIndices,
  );
  const clearHighlights = useEditorStore((state) => state.clearHighlights);

  const presetDescription = useMemo(() => {
    const descriptions: Record<CleanupPreset, string[]> = {
      Gentle: [t("editor.cleanupFillers")],
      Balanced: [
        t("editor.cleanupFillers"),
        t("editor.cleanupDuplicates"),
        t("editor.cleanupPauses"),
      ],
      Aggressive: [
        t("editor.cleanupFillers"),
        t("editor.cleanupDuplicates"),
        t("editor.cleanupPauses"),
        t("editor.removeSilence.button"),
      ],
    };
    return descriptions[preset].join(" · ");
  }, [preset, t]);

  const resultItems = useMemo(() => {
    if (!plan) {
      return [];
    }

    return [
      {
        count: plan.filler_count,
        label: t("editor.cleanupFillers"),
      },
      {
        count: plan.duplicate_count,
        label: t("editor.cleanupDuplicates"),
      },
      {
        count: plan.pause_count + plan.trim_count,
        label: t("editor.cleanupPauses"),
      },
      {
        count: plan.silence_count,
        label: t("editor.removeSilence.button"),
      },
    ].filter((item) => item.count > 0);
  }, [plan, t]);

  const handlePreview = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await invoke<CleanupPlan>("preview_cleanup", { preset });
      setPlan(result);
      const indices = Array.from(
        new Set(result.actions.map((action) => action.word_index)),
      );
      if (indices.length > 0) {
        setHighlightedIndices(indices, getHighlightType(result.actions));
      } else {
        clearHighlights();
      }
    } catch (error) {
      console.error("Cleanup preview failed:", error);
      toast.error(t("editor.cleanup.failed"));
    } finally {
      setIsLoading(false);
    }
  }, [clearHighlights, preset, setHighlightedIndices, t]);

  const handleApply = useCallback(async () => {
    setIsLoading(true);
    try {
      if (!plan) {
        return;
      }
      const result = await invoke<CleanupPlan>("apply_cleanup", { plan });
      if (result.total_affected === 0) {
        toast.info(t("editor.cleanupNothingFound"));
        return;
      }

      await refreshFromBackend();
      clearHighlights();
      setPlan(null);
      toast.success(
        t("editor.cleanupAffected", { count: result.total_affected }),
      );
      onClose();
    } catch (error) {
      console.error("Cleanup apply failed:", error);
      toast.error(t("editor.cleanup.failed"));
    } finally {
      setIsLoading(false);
    }
  }, [clearHighlights, onClose, plan, refreshFromBackend, t]);

  useEffect(() => {
    if (!isOpen) {
      setPlan(null);
      clearHighlights();
    }
  }, [clearHighlights, isOpen]);

  if (!isOpen) {
    return null;
  }

  const durationSavedSeconds = plan
    ? (plan.estimated_duration_saved_us / 1_000_000).toFixed(1)
    : "0.0";

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-mid-gray/20 bg-background-ui/10 p-3">
      <div className="flex items-center gap-2 text-sm font-medium text-text">
        <Sparkles className="h-4 w-4 text-logo-primary" />
        <span>{t("editor.cleanupTitle")}</span>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs text-mid-gray">
          {t("editor.cleanupPreset")}
        </span>
        {PRESETS.map((value) => (
          <Button
            key={value}
            type="button"
            variant={preset === value ? "primary-soft" : "ghost"}
            size="sm"
            onClick={() => {
              setPreset(value);
              setPlan(null);
              clearHighlights();
            }}
          >
            {t(`editor.cleanup${value}`)}
          </Button>
        ))}
      </div>

      <p className="text-xs text-mid-gray">{presetDescription}</p>

      {plan ? (
        <div className="rounded-lg border border-mid-gray/20 bg-background p-3 text-xs text-mid-gray">
          {plan.total_affected > 0 ? (
            <div className="flex flex-col gap-1">
              <span>
                {t("editor.cleanupAffected", { count: plan.total_affected })}
              </span>
              {resultItems.map((item) => (
                <span key={item.label}>
                  {item.count} · {item.label}
                </span>
              ))}
              <span>
                {t("editor.cleanupDurationSaved", {
                  duration: durationSavedSeconds,
                })}
              </span>
            </div>
          ) : (
            <span>{t("editor.cleanupNothingFound")}</span>
          )}
        </div>
      ) : null}

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="secondary"
          size="sm"
          onClick={handlePreview}
          disabled={isLoading}
          className="inline-flex items-center gap-1.5"
        >
          <Eye className="h-3.5 w-3.5" />
          {t("editor.cleanupPreview")}
        </Button>
        <Button
          type="button"
          variant="primary-soft"
          size="sm"
          onClick={handleApply}
          disabled={isLoading || !plan || plan.total_affected === 0}
          className="inline-flex items-center gap-1.5"
        >
          <Wand2 className="h-3.5 w-3.5" />
          {t("editor.cleanupApply")}
        </Button>
        <Button type="button" variant="ghost" size="sm" onClick={onClose}>
          {t("editor.close")}
        </Button>
      </div>
    </div>
  );
};

export default CleanupDialog;
