import React from "react";
import { useTranslation } from "react-i18next";
import { type CleanupPreset } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { Dropdown, type DropdownOption } from "@/components/ui/Dropdown";
import { SettingContainer } from "@/components/ui/SettingContainer";

interface CleanupPresetSettingProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

const OPTIONS: CleanupPreset[] = ["Gentle", "Balanced", "Aggressive"];

export const CleanupPresetSetting: React.FC<CleanupPresetSettingProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { settings, updateSetting, isUpdating } = useSettings();
    const preset = settings?.cleanup_preset ?? "Balanced";
    const options: DropdownOption[] = OPTIONS.map((value) => ({
      value,
      label: t(`settings.advanced.cleanupPreset.options.${value}`),
    }));

    const handleSelect = (value: string) => {
      void updateSetting("cleanup_preset", value as CleanupPreset);
    };

    return (
      <SettingContainer
        title={t("settings.advanced.cleanupPreset.title")}
        description={t("settings.advanced.cleanupPreset.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown
          options={options}
          selectedValue={preset}
          onSelect={handleSelect}
          disabled={isUpdating("cleanup_preset")}
        />
      </SettingContainer>
    );
  },
);

CleanupPresetSetting.displayName = "CleanupPresetSetting";
