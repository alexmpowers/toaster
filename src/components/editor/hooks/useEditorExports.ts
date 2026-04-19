import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import {
  commands,
  type AllowedExportFormat,
  type AppSettings,
  type AudioExportFormat,
  type ExportFormat,
  type MediaInfo,
} from "@/bindings";
import { unwrapResult } from "@/components/editor/EditorView.util";

interface UseEditorExportsArgs {
  mediaInfo: MediaInfo | null;
  settings: AppSettings | null;
  burnCaptions: boolean;
}

/**
 * Owns every export-related piece of state + the three export handlers
 * (transcript / edited media / FFmpeg script) and the effect that keeps
 * `allowedFormats` in sync with the loaded media.
 *
 * Round-6 Phase D: the per-project format override was removed. The
 * backend now selects a default based on the source MediaType and the
 * two settings fields `export_format_video` / `export_format_audio`.
 */
export function useEditorExports({
  mediaInfo,
  settings,
  burnCaptions,
}: UseEditorExportsArgs) {
  const { t } = useTranslation();
  const [isExportingMedia, setIsExportingMedia] = useState(false);
  const [allowedFormats, setAllowedFormats] = useState<AllowedExportFormat[]>(
    [],
  );

  useEffect(() => {
    if (!mediaInfo) {
      setAllowedFormats([]);
      return;
    }
    const ext = mediaInfo.extension ?? "";
    let cancelled = false;
    (async () => {
      try {
        const result = await commands.listAllowedExportFormats(ext);
        if (!cancelled) setAllowedFormats(result);
      } catch (err) {
        console.error("Failed to list allowed export formats:", err);
        if (!cancelled) setAllowedFormats([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [mediaInfo]);

  const defaultExportFormat: AudioExportFormat =
    mediaInfo?.media_type === "Video"
      ? (settings?.export_format_video ?? "mp4")
      : (settings?.export_format_audio ?? "wav");

  const handleExport = useCallback(async (format: ExportFormat) => {
    const ext = format === "Srt" ? "srt" : format === "Vtt" ? "vtt" : "txt";
    try {
      const filePath = await save({
        filters: [{ name: format, extensions: [ext] }],
        defaultPath: `transcript.${ext}`,
      });
      if (!filePath) return;
      unwrapResult(
        await commands.exportTranscriptToFile(format, filePath, null, null),
      );
    } catch (err) {
      console.error("Export failed:", err);
    }
  }, []);

  const handleFFmpegScript = useCallback(async () => {
    if (!mediaInfo) return;
    try {
      const script = unwrapResult(
        await commands.generateFfmpegEditScript(mediaInfo.path),
      );
      await navigator.clipboard.writeText(script);
    } catch (err) {
      console.error("FFmpeg script generation failed:", err);
    }
  }, [mediaInfo]);

  const handleExportEditedMedia = useCallback(async () => {
    if (!mediaInfo) return;

    const allowedMatch = allowedFormats.find(
      (f) => f.format === defaultExportFormat,
    );
    const extension = (
      allowedMatch?.extension.replace(/^\./, "") ?? defaultExportFormat
    ).toLowerCase();
    const baseName = mediaInfo.file_name.replace(/\.[^/.]+$/, "");

    try {
      const filePath = await save({
        filters: [
          {
            name:
              mediaInfo.media_type === "Video"
                ? t("editor.editedVideo")
                : t("editor.editedAudio"),
            extensions: [extension],
          },
        ],
        defaultPath: `${baseName}-edited.${extension}`,
      });
      if (!filePath) return;
      setIsExportingMedia(true);
      unwrapResult(
        await commands.exportEditedMedia(
          mediaInfo.path,
          filePath,
          burnCaptions || null,
          null,
        ),
      );
    } catch (err) {
      console.error("Edited media export failed:", err);
    } finally {
      setIsExportingMedia(false);
    }
  }, [mediaInfo, t, burnCaptions, defaultExportFormat, allowedFormats]);

  return {
    handleExport,
    handleFFmpegScript,
    handleExportEditedMedia,
    isExportingMedia,
    allowedFormats,
    defaultExportFormat,
  };
}
