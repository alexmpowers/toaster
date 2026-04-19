import React from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../../ui/SettingContainer";
import { Dropdown, type DropdownOption } from "../../ui/Dropdown";
import { useSettings } from "../../../hooks/useSettings";
import type { AudioExportFormat, LoudnessTarget } from "@/bindings";

const TARGETS: LoudnessTarget[] = ["off", "podcast_-16", "streaming_-14"];
// Video defaults: only Mp4 is a video container today.
const VIDEO_EXPORT_FORMATS: AudioExportFormat[] = ["mp4"];
// Audio-only defaults: the four audio variants, omitting Mp4.
const AUDIO_EXPORT_FORMATS: AudioExportFormat[] = ["mp3", "wav", "m4a", "opus"];

/**
 * Export group body for the Advanced page. Holds the default
 * Video-/Audio-output formats and the loudness target. The
 * page-level heading is intentionally absent — the parent
 * `SettingsGroup` in `AdvancedSettings.tsx` owns the group title.
 *
 * The loudness preflight panel was removed in Round 7 per user
 * feedback ("too confusing, get rid of it"). The backend
 * `loudness_preflight` command is retained for potential future
 * reuse but no UI surface invokes it.
 */
export const ExportGroup: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  const target: LoudnessTarget = settings?.loudness_target ?? "off";
  const exportFormatVideo: AudioExportFormat =
    settings?.export_format_video ?? "mp4";
  const exportFormatAudio: AudioExportFormat =
    settings?.export_format_audio ?? "wav";

  const handleTargetChange = (value: string) => {
    const next = value as LoudnessTarget;
    if (next === target) return;
    void updateSetting("loudness_target", next);
  };

  const handleFormatVideoChange = (value: string) => {
    const next = value as AudioExportFormat;
    if (next === exportFormatVideo) return;
    void updateSetting("export_format_video", next);
  };

  const handleFormatAudioChange = (value: string) => {
    const next = value as AudioExportFormat;
    if (next === exportFormatAudio) return;
    void updateSetting("export_format_audio", next);
  };

  const targetOptions: DropdownOption[] = TARGETS.map((value) => ({
    value,
    label: t(`settings.export.loudness.options.${value}.label`),
  }));

  const videoFormatOptions: DropdownOption[] = VIDEO_EXPORT_FORMATS.map(
    (value) => ({
      value,
      label: t(`settings.export.formatVideo.options.${value}.label`),
    }),
  );

  const audioFormatOptions: DropdownOption[] = AUDIO_EXPORT_FORMATS.map(
    (value) => ({
      value,
      label: t(`settings.export.formatAudio.options.${value}.label`),
    }),
  );

  return (
    <div className="space-y-1">
      <SettingContainer
        title={t("settings.export.formatVideo.label")}
        description={t("settings.export.formatVideo.description")}
        grouped
        layout="horizontal"
      >
        <Dropdown
          options={videoFormatOptions}
          selectedValue={exportFormatVideo}
          onSelect={handleFormatVideoChange}
          disabled={!settings || isUpdating("export_format_video")}
        />
      </SettingContainer>
      <SettingContainer
        title={t("settings.export.formatAudio.label")}
        description={t("settings.export.formatAudio.description")}
        grouped
        layout="horizontal"
      >
        <Dropdown
          options={audioFormatOptions}
          selectedValue={exportFormatAudio}
          onSelect={handleFormatAudioChange}
          disabled={!settings || isUpdating("export_format_audio")}
        />
      </SettingContainer>
      <SettingContainer
        title={t("settings.export.loudness.title")}
        description={t("settings.export.loudness.description")}
        grouped
        layout="horizontal"
      >
        <Dropdown
          options={targetOptions}
          selectedValue={target}
          onSelect={handleTargetChange}
          disabled={!settings || isUpdating("loudness_target")}
        />
      </SettingContainer>
    </div>
  );
};
