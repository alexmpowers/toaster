import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, Download } from "lucide-react";
import type { ExportFormat, MediaType } from "@/bindings";

interface ExportMenuProps {
  mediaType: MediaType | null;
  disabled?: boolean;
  isExportingMedia?: boolean;
  onExportEditedMedia: () => void;
  onExportTranscript: (format: ExportFormat) => void;
  onExportFFmpegScript: () => void;
}

/**
 * Single export entry-point for the editor. Replaces the previous
 * four-location export UI (header [Export] button + EditorToolbar
 * SRT/VTT/Script buttons + FFmpeg script button).
 *
 * Popover lists every available export path; the trigger button is
 * always [Export ▼] so users have exactly one place to look.
 */
const ExportMenu: React.FC<ExportMenuProps> = ({
  mediaType,
  disabled,
  isExportingMedia,
  onExportEditedMedia,
  onExportTranscript,
  onExportFFmpegScript,
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      if (!containerRef.current) return;
      if (!containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocClick);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const dispatch = (action: () => void) => {
    setOpen(false);
    action();
  };

  const editedLabel =
    mediaType === "Video"
      ? t("editor.exportMenu.editedVideo")
      : t("editor.exportMenu.editedAudio");

  return (
    <div ref={containerRef} className="relative inline-flex">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        disabled={disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        className={`flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
          open
            ? "bg-logo-primary/20 text-text"
            : "bg-mid-gray/10 text-text/60 hover:bg-mid-gray/20"
        }`}
      >
        <Download className="w-3.5 h-3.5" />
        <span>
          {isExportingMedia
            ? t("editor.exporting")
            : t("editor.exportMenu.trigger")}
        </span>
        <ChevronDown
          className={`w-3.5 h-3.5 transition-transform ${
            open ? "rotate-180" : ""
          }`}
        />
      </button>
      {open && (
        <div
          role="menu"
          className="absolute top-full right-0 mt-1 w-56 bg-background border border-mid-gray/80 rounded-lg shadow-lg z-50 overflow-hidden"
        >
          <MenuItem
            label={editedLabel}
            disabled={isExportingMedia || !mediaType}
            onClick={() => dispatch(onExportEditedMedia)}
          />
          <div className="border-t border-mid-gray/20" />
          <MenuItem
            label={t("editor.exportMenu.transcriptSrt")}
            onClick={() => dispatch(() => onExportTranscript("Srt"))}
          />
          <MenuItem
            label={t("editor.exportMenu.transcriptVtt")}
            onClick={() => dispatch(() => onExportTranscript("Vtt"))}
          />
          <MenuItem
            label={t("editor.exportMenu.transcriptScript")}
            onClick={() => dispatch(() => onExportTranscript("Script"))}
          />
          <div className="border-t border-mid-gray/20" />
          <MenuItem
            label={t("editor.exportMenu.ffmpegScript")}
            onClick={() => dispatch(onExportFFmpegScript)}
          />
        </div>
      )}
    </div>
  );
};

interface MenuItemProps {
  label: string;
  disabled?: boolean;
  onClick: () => void;
}

const MenuItem: React.FC<MenuItemProps> = ({ label, disabled, onClick }) => (
  <button
    type="button"
    role="menuitem"
    onClick={onClick}
    disabled={disabled}
    className="w-full px-3 py-1.5 text-sm text-left text-text transition-colors hover:bg-mid-gray/10 disabled:opacity-50 disabled:cursor-not-allowed"
  >
    {label}
  </button>
);

export default ExportMenu;
