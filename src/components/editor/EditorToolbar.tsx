import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { Dropdown, type DropdownOption } from "@/components/ui/Dropdown";
import { useSettings } from "@/hooks/useSettings";
import type { LoudnessTarget, Word } from "@/bindings";

interface EditorToolbarProps {
  words: Word[];
  burnCaptions: boolean;
  onBurnCaptionsChange: (next: boolean) => void;
  normalizeAudio: boolean;
  onNormalizeAudioToggle: () => void;
}

const LOUDNESS_TARGETS: LoudnessTarget[] = [
  "off",
  "podcast_-16",
  "streaming_-14",
];

/**
 * Per-project export knobs. Shown alongside the editor when words are
 * loaded. Export triggers (SRT / VTT / Script / FFmpeg / edited media)
 * live in the header `<ExportMenu>` — this component owns only the
 * per-export knobs: burn captions, normalize audio, loudness target.
 *
 * Round-8: the default Video/Audio output-format dropdowns were
 * removed. Format selection now lives inside the header `ExportMenu`
 * (one row per allowed format for the loaded media type), matching
 * user feedback that format is a per-project concern, not a global
 * setting. The loudness target stayed here because it's a
 * per-project audio-render concern, not a format choice.
 */
const EditorToolbar: React.FC<EditorToolbarProps> = React.memo(
  ({
    words,
    burnCaptions,
    onBurnCaptionsChange,
    normalizeAudio,
    onNormalizeAudioToggle,
  }) => {
    const { t } = useTranslation();
    const { settings, updateSetting, isUpdating } = useSettings();
    const loudnessTarget: LoudnessTarget = settings?.loudness_target ?? "off";

    if (words.length === 0) return null;

    const loudnessOptions: DropdownOption[] = LOUDNESS_TARGETS.map((value) => ({
      value,
      label: t(`settings.export.loudness.options.${value}.label`),
    }));

    const handleLoudnessChange = (value: string) => {
      const next = value as LoudnessTarget;
      if (next === loudnessTarget) return;
      void updateSetting("loudness_target", next);
    };

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

          {normalizeAudio && (
            <SettingContainer
              title={t("settings.export.loudness.title")}
              description={t("settings.export.loudness.description")}
              grouped
              layout="horizontal"
            >
              <Dropdown
                options={loudnessOptions}
                selectedValue={loudnessTarget}
                onSelect={handleLoudnessChange}
                disabled={!settings || isUpdating("loudness_target")}
              />
            </SettingContainer>
          )}
        </div>
      </SettingsGroup>
    );
  },
);

EditorToolbar.displayName = "EditorToolbar";

export default EditorToolbar;
