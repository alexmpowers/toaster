import React from "react";

interface ColorPickerProps {
  value: string;
  onChange: (color: string) => void;
  disabled?: boolean;
  label?: string;
}

export const ColorPicker: React.FC<ColorPickerProps> = ({ value, onChange, disabled, label }) => {
  // Extract the 6-char hex color (ignore alpha if present)
  const colorOnly = value.length > 7 ? value.slice(0, 7) : value;
  const alpha = value.length > 7 ? value.slice(7) : "";

  return (
    <div className="flex items-center gap-2">
      <input
        type="color"
        value={colorOnly}
        onChange={(e) => onChange(e.target.value + alpha)}
        disabled={disabled}
        className="w-8 h-8 rounded cursor-pointer border border-mid-gray/20 bg-transparent"
        aria-label={label}
      />
      <input
        type="text"
        value={value}
        onChange={(e) => {
          const v = e.target.value;
          if (/^#[0-9A-Fa-f]{0,8}$/.test(v)) onChange(v);
        }}
        disabled={disabled}
        className="w-28 px-2 py-1 text-xs rounded border border-mid-gray/20 bg-background text-text font-mono"
        placeholder="#FFFFFF"
        maxLength={9}
      />
    </div>
  );
};
