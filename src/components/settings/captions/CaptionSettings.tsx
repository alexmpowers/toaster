import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import type { CaptionProfile, CaptionProfileSet } from "@/bindings";
import { CaptionPreviewPane, type SampleKey } from "./CaptionProfileShared";
import { CaptionProfileForm } from "./CaptionProfileForm";
import type { CaptionMockOrientation } from "./CaptionMockFrame";
import { Dropdown } from "../../ui/Dropdown";
import { SettingContainer } from "../../ui/SettingContainer";

interface CaptionSettingsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const DEFAULT_DESKTOP: CaptionProfile = {
  font_size: 40,
  bg_color: "#000000B3",
  text_color: "#FFFFFF",
  position: 90,
  font_family: "Inter",
  radius_px: 0,
  padding_x_px: 12,
  padding_y_px: 4,
  max_width_percent: 90,
};

const DEFAULT_MOBILE: CaptionProfile = {
  font_size: 48,
  bg_color: "#000000B3",
  text_color: "#FFFFFF",
  position: 80,
  font_family: "Inter",
  radius_px: 8,
  padding_x_px: 14,
  padding_y_px: 6,
  max_width_percent: 80,
};

/**
 * Caption settings surface. Persistence remains dual-profile
 * (`AppSettings.caption_profiles.{desktop, mobile}`, Slice B of
 * `caption-profiles-persistence`) but the UI is unified behind a
 * single orientation control in the preview toolbar: Horizontal edits
 * the desktop profile, Vertical edits the mobile profile. Prior
 * Desktop|Mobile tab row was duplicative and has been removed.
 */
export const CaptionSettings: React.FC<CaptionSettingsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting } = useSettings();

    const profileSet = (getSetting("caption_profiles") as
      | CaptionProfileSet
      | undefined) ?? {
      desktop: DEFAULT_DESKTOP,
      mobile: DEFAULT_MOBILE,
    };

    const [previewOrientation, setPreviewOrientation] =
      useState<CaptionMockOrientation>("horizontal");
    const [selectedSampleKey, setSelectedSampleKey] =
      useState<SampleKey>("single");

    const isVertical = previewOrientation === "vertical";
    const activeProfile = isVertical ? profileSet.mobile : profileSet.desktop;

    const handleChange = (patch: Partial<CaptionProfile>) => {
      const merged: CaptionProfile = { ...activeProfile, ...patch };
      const next: CaptionProfileSet = {
        desktop: isVertical ? profileSet.desktop : merged,
        mobile: isVertical ? merged : profileSet.mobile,
      };
      updateSetting("caption_profiles", next);
    };

    const disabled = false;

    return (
      <div className="px-4 py-4 space-y-4">
        <CaptionPreviewPane
          profile={activeProfile}
          orientation={previewOrientation}
          selectedSampleKey={selectedSampleKey}
        />

        <SettingContainer
          title={t("settings.captions.preview.orientation.label")}
          description={t("settings.captions.preview.orientation.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <Dropdown
            selectedValue={previewOrientation}
            options={[
              {
                value: "horizontal",
                label: t("settings.captions.preview.orientation.horizontal"),
              },
              {
                value: "vertical",
                label: t("settings.captions.preview.orientation.vertical"),
              },
            ]}
            onSelect={(v) =>
              setPreviewOrientation(v as CaptionMockOrientation)
            }
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.captions.preview.sampleLegend")}
          description={t("settings.captions.preview.sampleDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <Dropdown
            selectedValue={selectedSampleKey}
            options={[
              {
                value: "single",
                label: t("settings.captions.preview.sample.label.single"),
              },
              {
                value: "multiLine",
                label: t("settings.captions.preview.sample.label.multiLine"),
              },
            ]}
            onSelect={(v) => setSelectedSampleKey(v as SampleKey)}
          />
        </SettingContainer>

        <CaptionProfileForm
          profile={activeProfile}
          onChange={handleChange}
          descriptionMode={descriptionMode}
          grouped={grouped}
          disabled={disabled}
        />
      </div>
    );
  },
);
