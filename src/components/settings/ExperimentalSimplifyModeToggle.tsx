import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface ExperimentalSimplifyModeToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ExperimentalSimplifyModeToggle: React.FC<
  ExperimentalSimplifyModeToggleProps
> = React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const enabled = getSetting("experimental_simplify_mode") || false;

  return (
    <ToggleSwitch
      checked={enabled}
      onChange={(next) => updateSetting("experimental_simplify_mode", next)}
      isUpdating={isUpdating("experimental_simplify_mode")}
      label={t("settings.advanced.experimentalSimplifyModeToggle.label")}
      description={t("settings.advanced.experimentalSimplifyModeToggle.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
});
