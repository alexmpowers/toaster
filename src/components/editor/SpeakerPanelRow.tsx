import React from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import type { SpeakerInfo } from "@/stores/editorStore";
import { getSpeakerColor } from "./speakerColors";
import { formatDurationSeconds } from "./speakerPanelUtils";

interface SpeakerPanelRowProps {
  speaker: SpeakerInfo;
  editingSpeakerId: number | null;
  draftName: string;
  onDraftNameChange: (value: string) => void;
  onBeginRename: (speaker: SpeakerInfo) => void;
  onCommitRename: () => void;
  onCancelRename: () => void;
}

const SpeakerPanelRow: React.FC<SpeakerPanelRowProps> = ({
  speaker,
  editingSpeakerId,
  draftName,
  onDraftNameChange,
  onBeginRename,
  onCommitRename,
  onCancelRename,
}) => {
  const { t } = useTranslation();
  const displayName = speaker.name || t("editor.speaker", { id: speaker.id + 1 });

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-mid-gray/20 bg-background px-3 py-2">
      <div className="flex min-w-0 items-center gap-2">
        <span
          className="inline-block h-2.5 w-2.5 shrink-0 rounded-full"
          style={{ backgroundColor: getSpeakerColor(speaker.id) }}
        />
        {editingSpeakerId === speaker.id ? (
          <Input
            value={draftName}
            onChange={(event) => onDraftNameChange(event.target.value)}
            onBlur={() => void onCommitRename()}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void onCommitRename();
              }
              if (event.key === "Escape") {
                onCancelRename();
              }
            }}
            autoFocus
            aria-label={t("editor.speakerName")}
            className="min-w-[180px]"
          />
        ) : (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => onBeginRename(speaker)}
            className="!px-1 !py-0.5 text-sm font-medium"
            title={t("editor.renameSpeaker")}
          >
            {displayName}
          </Button>
        )}
      </div>
      <div className="text-xs text-mid-gray">
        {t("editor.speakerWords", { count: speaker.word_count })} ·{" "}
        {t("editor.speakerDuration", {
          duration: formatDurationSeconds(speaker.total_duration_us),
        })}
      </div>
    </div>
  );
};

export default SpeakerPanelRow;
