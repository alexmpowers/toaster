import React from "react";
import { useTranslation } from "react-i18next";
import { ModelUnloadTimeoutSetting } from "../ModelUnloadTimeout";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { HistoryLimit } from "../HistoryLimit";
import { ExperimentalToggle } from "../ExperimentalToggle";
import { useSettings } from "../../../hooks/useSettings";
import { AccelerationSelector } from "../AccelerationSelector";
import { DiscardWords } from "../DiscardWords";
import { AllowWords } from "../AllowWords";
import { CaptionSettings } from "../CaptionSettings";
import { ExperimentalSimplifyModeToggle } from "../ExperimentalSimplifyModeToggle";

export const AdvancedSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const experimentalEnabled = getSetting("experimental_enabled") || false;

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.advanced.groups.app")}>
        <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped={true} />
        <ExperimentalToggle descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.groups.transcription")}>
        <DiscardWords descriptionMode="tooltip" grouped />
        <AllowWords descriptionMode="tooltip" grouped />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.groups.captionSettings")}>
        <CaptionSettings descriptionMode="tooltip" grouped />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.groups.history")}>
        <HistoryLimit descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      {experimentalEnabled && (
        <SettingsGroup title={t("settings.advanced.groups.experimental")}>
          <ExperimentalSimplifyModeToggle
            descriptionMode="tooltip"
            grouped={true}
          />
          <AccelerationSelector descriptionMode="tooltip" grouped={true} />
        </SettingsGroup>
      )}
    </div>
  );
};
