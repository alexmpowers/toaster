import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown, type DropdownOption } from "@/components/ui/Dropdown";
import type { AllowedExportFormat, AudioExportFormat } from "@/bindings";

interface ExportFormatPickerProps {
  /**
   * Current per-export override. `null` means "use Advanced-settings default".
   */
  value: AudioExportFormat | null;
  onChange: (next: AudioExportFormat | null) => void;
  /** Source-compatible options returned by backend `list_allowed_export_formats`. */
  options: AllowedExportFormat[];
  /** Effective default from Advanced settings; used as the placeholder when value is null. */
  defaultFormat: AudioExportFormat;
  disabled?: boolean;
}

const USE_DEFAULT_VALUE = "__use_default__";

/**
 * Per-project export-format override control for the Editor Export
 * button. Backend is the single source of truth for which formats are
 * valid given the source media extension (AC-003-a). When the user
 * selects "Use default", we store null and defer to
 * `settings.export_format`.
 *
 * PRD: features/edit-export-format-override/PRD.md (AC-001-*, AC-004-*).
 */
const ExportFormatPicker: React.FC<ExportFormatPickerProps> = ({
  value,
  onChange,
  options,
  defaultFormat,
  disabled,
}) => {
  const { t } = useTranslation();

  const defaultLabel = t(`editor.exportFormat.format${capitalize(defaultFormat)}` as const, {
    defaultValue: defaultFormat.toUpperCase(),
  });

  const dropdownOptions: DropdownOption[] = [
    {
      value: USE_DEFAULT_VALUE,
      label: t("editor.exportFormat.useDefault", { format: defaultLabel }),
    },
    ...options.map((opt) => ({
      value: opt.format,
      label: t(`editor.exportFormat.format${capitalize(opt.format)}` as const, {
        defaultValue: opt.format.toUpperCase(),
      }),
    })),
  ];

  const handleSelect = (next: string) => {
    if (next === USE_DEFAULT_VALUE) {
      onChange(null);
      return;
    }
    onChange(next as AudioExportFormat);
  };

  return (
    <span title={t("editor.exportFormat.tooltip")} aria-label={t("editor.exportFormat.label")}>
      <Dropdown
        options={dropdownOptions}
        selectedValue={value ?? USE_DEFAULT_VALUE}
        onSelect={handleSelect}
        disabled={disabled || options.length === 0}
      />
    </span>
  );
};

function capitalize<T extends string>(v: T): Capitalize<T> {
  return (v.charAt(0).toUpperCase() + v.slice(1)) as Capitalize<T>;
}

export default ExportFormatPicker;
