import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown, type DropdownOption } from "@/components/ui/Dropdown";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { useSettings } from "@/hooks/useSettings";
import type { LoudnessTarget, Word } from "@/bindings";

interface EditorToolbarProps {
  words: Word[];
  burnCaptions: boolean;
  onBurnCaptionsChange: (next: boolean) => void;
  normalizeAudio: boolean;
  onNormalizeAudioToggle: () => void;
}

const LOUDNESS_TARGETS: LoudnessTarget[] = ["off", "podcast_-16", "streaming_-14"];

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
      if (next !== loudnessTarget) {
        void updateSetting("loudness_target", next);
      }
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
