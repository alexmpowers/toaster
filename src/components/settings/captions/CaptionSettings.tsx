import React, { useState } from "react";
import { useSettings } from "../../../hooks/useSettings";
import type { CaptionProfile, CaptionProfileSet } from "@/bindings";
import { CaptionPreviewPane } from "./CaptionProfileShared";
import { CaptionProfileForm } from "./CaptionProfileForm";
import type { CaptionMockOrientation } from "./CaptionMockFrame";

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
    const { getSetting, updateSetting } = useSettings();

    const profileSet = (getSetting("caption_profiles") as
      | CaptionProfileSet
      | undefined) ?? {
      desktop: DEFAULT_DESKTOP,
      mobile: DEFAULT_MOBILE,
    };

    const [previewOrientation, setPreviewOrientation] =
      useState<CaptionMockOrientation>("horizontal");

    const isVertical = previewOrientation === "vertical";
    const activeProfile = isVertical ? profileSet.mobile : profileSet.desktop;

    // Round-8: `isVertical` still drives which profile we patch (desktop vs
    // mobile). Helper kept for readability when handleChange reads it below.

    const handleChange = (patch: Partial<CaptionProfile>) => {
      const merged: CaptionProfile = { ...activeProfile, ...patch };
      const next: CaptionProfileSet = {
        desktop: isVertical ? profileSet.desktop : merged,
        mobile: isVertical ? merged : profileSet.mobile,
      };
      updateSetting("caption_profiles", next);
    };

    // Round-8: intentionally do NOT thread `isUpdating("caption_profiles")`
    // into the form. Caption-profile writes are idempotent and fire on
    // every slider tick; pulsing the `disabled` prop true→false on each
    // write was causing the font-family Select to visibly flash/flicker
    // even when the user was only dragging a neighbouring slider.
    const disabled = false;

    return (
      <div className="px-4 py-4 space-y-4">
        <CaptionPreviewPane
          profile={activeProfile}
          orientation={previewOrientation}
          onOrientationChange={setPreviewOrientation}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />

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
