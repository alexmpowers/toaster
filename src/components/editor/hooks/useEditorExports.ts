import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  commands,
  type AllowedExportFormat,
  type AudioExportFormat,
  type ExportFormat,
  type MediaInfo,
} from "@/bindings";
import { unwrapResult } from "@/components/editor/EditorView.util";

interface UseEditorExportsArgs {
  mediaInfo: MediaInfo | null;
  burnCaptions: boolean;
}

/**
 * Owns every export-related piece of state + the three export handlers
 * (transcript / edited media / FFmpeg script) and the effect that keeps
 * `allowedFormats` in sync with the loaded media.
 *
 * Round-8: the two persisted `export_format_video` /
 * `export_format_audio` settings were removed. Format choice is now a
 * per-project parameter carried by the `ExportMenu` popover — each
 * allowed format has its own menu row and passes its own format
 * override to `commands.exportEditedMedia`. `defaultExportFormat` is
 * still returned for fallback-path consumers (save-dialog extension
 * hint when caller did not specify a format explicitly).
 */
export function useEditorExports({
  mediaInfo,
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

  // Source-type fallback when the caller passes null — matches the
  // backend's own default-selection in `export_edited_media`.
  const defaultExportFormat: AudioExportFormat =
    mediaInfo?.media_type === "Video" ? "mp4" : "wav";

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
      toast.success(t("editor.ffmpegScriptCopied"));
    } catch (err) {
      console.error("FFmpeg script generation failed:", err);
      toast.error(t("editor.ffmpegScriptFailed"));
    }
  }, [mediaInfo, t]);

  const handleExportEditedMedia = useCallback(
    async (format: AudioExportFormat) => {
      if (!mediaInfo) return;

      const allowedMatch = allowedFormats.find((f) => f.format === format);
      const extension = (
        allowedMatch?.extension.replace(/^\./, "") ?? format
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
            format,
          ),
        );
      } catch (err) {
        console.error("Edited media export failed:", err);
      } finally {
        setIsExportingMedia(false);
      }
    },
    [mediaInfo, t, burnCaptions, allowedFormats],
  );

  return {
    handleExport,
    handleFFmpegScript,
    handleExportEditedMedia,
    isExportingMedia,
    allowedFormats,
    defaultExportFormat,
  };
}
