import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { ExportGroup } from "@/components/settings/advanced/ExportGroup";
import type { Word } from "@/bindings";

interface EditorToolbarProps {
  words: Word[];
  burnCaptions: boolean;
  onBurnCaptionsChange: (next: boolean) => void;
  normalizeAudio: boolean;
  onNormalizeAudioToggle: () => void;
}

/**
 * Export settings panel. Shown alongside the editor when words are
 * loaded. Export triggers (SRT / VTT / Script / FFmpeg / edited media)
 * live in the header `<ExportMenu>` — this component owns only the
 * per-export knobs: burn captions, normalize audio, loudness target +
 * preflight. Default format selection lives in Settings → Advanced →
 * Export (Round-6 Phase D).
 */
const EditorToolbar: React.FC<EditorToolbarProps> = React.memo(({
  words,
  burnCaptions,
  onBurnCaptionsChange,
  normalizeAudio,
  onNormalizeAudioToggle,
}) => {
  const { t } = useTranslation();

  if (words.length === 0) return null;

  return (
    <SettingsGroup title={t("editor.sections.exportSettings")}>
      <div className="space-y-1">
        <ToggleSwitch
          checked={burnCaptions}
          onChange={onBurnCaptionsChange}
          label={t("editor.addCaptions")}
          description={t("editor.addCaptionsDescription")}
          grouped
        />

        <ToggleSwitch
          checked={normalizeAudio}
          onChange={onNormalizeAudioToggle}
          label={t("editor.normalizeAudio")}
          description={t("editor.normalizeAudioDescription")}
          grouped
        />

        <ExportGroup />
      </div>
    </SettingsGroup>
  );
});

EditorToolbar.displayName = "EditorToolbar";

export default EditorToolbar;
