import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { SettingContainer } from "../ui/SettingContainer";
import { ColorPicker } from "../ui/ColorPicker";
import { Select } from "../ui/Select";
import type { CaptionFontFamily } from "@/bindings";

interface CaptionSettingsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

interface SliderWithInputProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  suffix: string;
  onChange: (value: number) => void;
  disabled?: boolean;
}

const SliderWithInput: React.FC<SliderWithInputProps> = ({
  value,
  min,
  max,
  step = 1,
  suffix,
  onChange,
  disabled,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [editValue, setEditValue] = useState(String(value));
  // Local drag state to avoid calling onChange (updateSetting) on every pixel
  const [isDragging, setIsDragging] = useState(false);
  const [localValue, setLocalValue] = useState(value);

  // Sync local value when external value changes (but not during drag)
  React.useEffect(() => {
    if (!isDragging) {
      setLocalValue(value);
    }
  }, [value, isDragging]);

  const handleCommit = () => {
    const parsed = parseInt(editValue);
    if (!isNaN(parsed)) {
      onChange(Math.min(max, Math.max(min, parsed)));
    }
    setIsEditing(false);
  };

  const handleSliderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = parseInt(e.target.value);
    setLocalValue(newValue);
    // Don't call onChange during drag - wait for release
  };

  const handleDragStart = () => {
    setIsDragging(true);
  };

  const handleDragEnd = () => {
    setIsDragging(false);
    // Only call onChange (updateSetting) on release
    if (localValue !== value) {
      onChange(localValue);
    }
  };

  const displayValue = isDragging ? localValue : value;
  const pct = ((displayValue - min) / (max - min)) * 100;

  return (
    <div className="flex items-center gap-3">
      <div className="relative w-40 h-6 flex items-center">
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={displayValue}
          onChange={handleSliderChange}
          onMouseDown={handleDragStart}
          onMouseUp={handleDragEnd}
          onMouseLeave={(e) => {
            // Commit if mouse leaves while dragging (button still pressed)
            if (isDragging && e.buttons === 0) handleDragEnd();
          }}
          onTouchStart={handleDragStart}
          onTouchEnd={handleDragEnd}
          disabled={disabled}
          className="w-full cursor-pointer outline-none appearance-none"
          style={{
            background: `linear-gradient(to right, #E8A838 0%, #E8A838 ${pct}%, #d1d5db ${pct}%, #d1d5db 100%)`,
            cursor: disabled ? "not-allowed" : "pointer",
            width: "100%",
            height: "6px",
            WebkitAppearance: "none",
            borderRadius: "3px",
          }}
        />
      </div>
      {isEditing ? (
        <input
          type="text"
          inputMode="numeric"
          value={editValue}
          onChange={(e) => {
            const v = e.target.value;
            if (/^\d*$/.test(v)) setEditValue(v);
          }}
          onBlur={handleCommit}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleCommit();
            if (e.key === "Escape") setIsEditing(false);
          }}
          autoFocus
          className="w-14 px-2 py-0.5 text-xs rounded border border-mid-gray/30 bg-background text-text font-mono text-right"
        />
      ) : (
        <span
          className="text-xs text-text/70 w-12 text-right font-mono cursor-pointer hover:text-accent transition-colors"
          onDoubleClick={() => {
            setEditValue(String(displayValue));
            setIsEditing(true);
          }}
          title="Double-click to edit"
        >
          {displayValue}{suffix}
        </span>
      )}
    </div>
  );
};

export const CaptionSettings: React.FC<CaptionSettingsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const position = (getSetting("caption_position") as number) ?? 90;
    const fontSize = (getSetting("caption_font_size") as number) ?? 24;
    const bgColorHex = (getSetting("caption_bg_color") as string) ?? "#000000B3";
    const textColor = (getSetting("caption_text_color") as string) ?? "#FFFFFF";
    const fontFamily =
      (getSetting("caption_font_family") as CaptionFontFamily) ?? "Inter";
    const radiusPx = (getSetting("caption_radius_px") as number) ?? 8;
    const paddingX = (getSetting("caption_padding_x_px") as number) ?? 16;
    const paddingY = (getSetting("caption_padding_y_px") as number) ?? 8;
    const maxWidth = (getSetting("caption_max_width_percent") as number) ?? 80;

    // Extract transparency from bg color hex alpha (last 2 chars)
    const bgColorBase = bgColorHex.slice(0, 7);
    const bgAlphaHex = bgColorHex.length > 7 ? bgColorHex.slice(7, 9) : "FF";
    const bgTransparency = Math.round((parseInt(bgAlphaHex, 16) / 255) * 100);

    const handleTransparencyChange = (pct: number) => {
      const alpha = Math.round((pct / 100) * 255)
        .toString(16)
        .padStart(2, "0")
        .toUpperCase();
      updateSetting("caption_bg_color", bgColorBase + alpha);
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.captionSettings.position")}
          description={t("settings.advanced.captionSettings.positionDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <SliderWithInput
            value={position}
            min={0}
            max={100}
            suffix="%"
            onChange={(v) => updateSetting("caption_position", v)}
            disabled={isUpdating("caption_position")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.fontSize")}
          description={t("settings.advanced.captionSettings.fontSizeDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <SliderWithInput
            value={fontSize}
            min={12}
            max={72}
            suffix="px"
            onChange={(v) => updateSetting("caption_font_size", v)}
            disabled={isUpdating("caption_font_size")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.bgTransparency")}
          description={t("settings.advanced.captionSettings.bgTransparencyDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <SliderWithInput
            value={bgTransparency}
            min={0}
            max={100}
            suffix="%"
            onChange={handleTransparencyChange}
            disabled={isUpdating("caption_bg_color")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.bgColor")}
          description={t("settings.advanced.captionSettings.bgColorDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <ColorPicker
            value={bgColorBase}
            onChange={(color) => {
              const alpha = bgColorHex.length > 7 ? bgColorHex.slice(7, 9) : "B3";
              updateSetting("caption_bg_color", color + alpha);
            }}
            disabled={isUpdating("caption_bg_color")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.textColor")}
          description={t("settings.advanced.captionSettings.textColorDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <ColorPicker
            value={textColor}
            onChange={(color) => updateSetting("caption_text_color", color)}
            disabled={isUpdating("caption_text_color")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.fontFamily")}
          description={t("settings.advanced.captionSettings.fontFamilyDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <Select
            value={fontFamily}
            options={[
              { value: "Inter", label: t("settings.advanced.captionSettings.fontInter") },
              { value: "Roboto", label: t("settings.advanced.captionSettings.fontRoboto") },
              { value: "SystemUi", label: t("settings.advanced.captionSettings.fontSystemUi") },
            ]}
            onChange={(v) => {
              if (v) updateSetting("caption_font_family", v as CaptionFontFamily);
            }}
            disabled={isUpdating("caption_font_family")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.cornerRadius")}
          description={t("settings.advanced.captionSettings.cornerRadiusDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <SliderWithInput
            value={radiusPx}
            min={0}
            max={48}
            suffix="px"
            onChange={(v) => updateSetting("caption_radius_px", v)}
            disabled={isUpdating("caption_radius_px")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.paddingHorizontal")}
          description={t("settings.advanced.captionSettings.paddingHorizontalDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <SliderWithInput
            value={paddingX}
            min={0}
            max={64}
            suffix="px"
            onChange={(v) => updateSetting("caption_padding_x_px", v)}
            disabled={isUpdating("caption_padding_x_px")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.paddingVertical")}
          description={t("settings.advanced.captionSettings.paddingVerticalDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <SliderWithInput
            value={paddingY}
            min={0}
            max={32}
            suffix="px"
            onChange={(v) => updateSetting("caption_padding_y_px", v)}
            disabled={isUpdating("caption_padding_y_px")}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.advanced.captionSettings.maxWidth")}
          description={t("settings.advanced.captionSettings.maxWidthDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <SliderWithInput
            value={maxWidth}
            min={20}
            max={100}
            suffix="%"
            onChange={(v) => updateSetting("caption_max_width_percent", v)}
            disabled={isUpdating("caption_max_width_percent")}
          />
        </SettingContainer>
      </>
    );
  },
);
