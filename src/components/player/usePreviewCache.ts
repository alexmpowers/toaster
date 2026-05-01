import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { commands } from "@/bindings";
import type { Word, TimingContractSnapshot } from "@/stores/editorStore";

interface CachedPreviewMetadata {
  generationToken: string;
  sourceMediaFingerprint: string | null;
  editVersion: string;
}

type PreviewCacheMode = "building" | "ready" | "fallback";

interface PlaybackAudioContract {
  selected_output_device: string;
  selected_output_device_available: boolean;
  preferred_output_sample_rate: number;
  detected_output_sample_rate: number | null;
  normalized_output_sample_rate: number;
  mismatch_detected: boolean;
}

interface UsePreviewCacheParams {
  mediaUrl: string | null;
  mediaType: "video" | "audio" | null;
  words: Word[];
  timingContract: TimingContractSnapshot | null;
}

export function usePreviewCache({
  mediaUrl,
  mediaType,
  words,
  timingContract,
}: UsePreviewCacheParams) {
  const { t } = useTranslation();
  const previewAudioRef = useRef<HTMLAudioElement>(null);
  const [previewEdits, setPreviewEdits] = useState(true);

  const [previewCacheState, setPreviewCacheState] = useState<
    "idle" | "loading" | "ready" | "error"
  >("idle");
  const [previewAudioUrl, setPreviewAudioUrl] = useState<string | null>(null);
  const [previewAudioReady, setPreviewAudioReady] = useState(false);
  const previewRenderTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const previewInvalidationTimerRef = useRef<ReturnType<
    typeof setTimeout
  > | null>(null);
  const previewRenderSeq = useRef(0);
  const previewMetadataRef = useRef<CachedPreviewMetadata | null>(null);

  const hasPreviewAudio = !!previewAudioUrl;
  const usePreviewCacheFlag =
    previewEdits && hasPreviewAudio && previewCacheState === "ready";
  const previewCacheMode: PreviewCacheMode =
    !previewEdits || previewCacheState === "error"
      ? "fallback"
      : previewCacheState === "loading" ||
          (hasPreviewAudio && !previewAudioReady)
        ? "building"
        : usePreviewCacheFlag
          ? "ready"
          : "fallback";
  const previewToggleLabel = previewEdits
    ? t("player.previewEditsOn")
    : t("player.previewEditsOff");
  const previewCacheModeLabel =
    previewCacheMode === "building"
      ? t("player.cacheModeBuilding")
      : previewCacheMode === "ready"
        ? t("player.cacheModeReady")
        : t("player.cacheModeFallback");

  const hasVideoPreviewCandidate = mediaType === "video" && usePreviewCacheFlag;
  const isDualTrackVideoPreview = hasVideoPreviewCandidate && previewAudioReady;
  const primarySrc =
    mediaType === "video"
      ? mediaUrl
      : usePreviewCacheFlag
        ? previewAudioUrl
        : mediaUrl;
  const activePlaybackSrc = isDualTrackVideoPreview
    ? previewAudioUrl
    : primarySrc;

  const schedulePreviewInvalidation = useCallback(
    (stalePreview: CachedPreviewMetadata | null, reason: string) => {
      if (!stalePreview?.generationToken) {
        return;
      }
      if (previewInvalidationTimerRef.current) {
        clearTimeout(previewInvalidationTimerRef.current);
      }
      previewInvalidationTimerRef.current = setTimeout(() => {
        void invoke("invalidate_temp_preview_cache", {
          generationToken: stalePreview.generationToken,
          sourceMediaFingerprint: stalePreview.sourceMediaFingerprint,
          reason,
        }).catch((error) => {
          console.warn("Failed to invalidate preview cache:", error);
        });
      }, 250);
    },
    [],
  );

  const resetPreviewCache = useCallback(
    (reason: string, invalidateBackend: boolean) => {
      if (previewRenderTimerRef.current) {
        clearTimeout(previewRenderTimerRef.current);
        previewRenderTimerRef.current = null;
      }
      if (previewInvalidationTimerRef.current) {
        clearTimeout(previewInvalidationTimerRef.current);
        previewInvalidationTimerRef.current = null;
      }

      previewRenderSeq.current += 1;
      const stalePreview = previewMetadataRef.current;
      previewMetadataRef.current = null;
      setPreviewAudioUrl(null);
      setPreviewAudioReady(false);
      setPreviewCacheState("idle");

      if (!invalidateBackend || !stalePreview?.generationToken) {
        return;
      }

      schedulePreviewInvalidation(stalePreview, reason);
    },
    [schedulePreviewInvalidation],
  );

  // Cleanup timers on unmount
  useEffect(() => {
    return () => {
      if (previewRenderTimerRef.current) {
        clearTimeout(previewRenderTimerRef.current);
      }
      if (previewInvalidationTimerRef.current) {
        clearTimeout(previewInvalidationTimerRef.current);
      }
    };
  }, []);

  // Reset preview cache on media change
  const previousPreviewLifecycleRef = useRef<{
    mediaUrl: string | null;
    words: Word[];
  } | null>(null);
  useEffect(() => {
    const previous = previousPreviewLifecycleRef.current;
    previousPreviewLifecycleRef.current = { mediaUrl, words };

    if (!previous) {
      return;
    }

    if (previous.mediaUrl !== mediaUrl) {
      resetPreviewCache("media-change", true);
      return;
    }
  }, [mediaUrl, resetPreviewCache, words]);

  // Normalize playback audio contract
  useEffect(() => {
    if (!mediaUrl) return;
    let cancelled = false;

    void (async () => {
      try {
        const contract = await invoke<PlaybackAudioContract>(
          "normalize_playback_audio_contract",
        );
        if (cancelled) return;
        if (contract.mismatch_detected) {
          console.warn(
            `[audio-contract] sample rate normalized ${contract.preferred_output_sample_rate}Hz -> ${contract.normalized_output_sample_rate}Hz on ${contract.selected_output_device}`,
          );
        }
        if (!contract.selected_output_device_available) {
          console.warn(
            "[audio-contract] selected output device missing; fell back to default output device",
          );
        }
      } catch (error) {
        if (!cancelled) {
          console.warn("Failed to normalize playback audio contract:", error);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [mediaUrl]);

  // Debounced preview cache generation
  useEffect(() => {
    if (!previewEdits || words.length === 0) {
      const reason = previewEdits ? "empty-transcript" : "preview-disabled";
      resetPreviewCache(reason, true);
      return;
    }

    setPreviewCacheState("loading");

    if (previewRenderTimerRef.current) {
      clearTimeout(previewRenderTimerRef.current);
    }

    const seq = ++previewRenderSeq.current;

    previewRenderTimerRef.current = setTimeout(() => {
      void (async () => {
        try {
          const result = await commands.renderTempPreviewAudio();
          if (seq !== previewRenderSeq.current) return;
          if (result.status !== "ok") {
            previewMetadataRef.current = null;
            setPreviewCacheState("error");
            return;
          }
          const meta = result.data;
          if (meta.status === "ready" && meta.preview_url_safe_path) {
            const stalePreview = previewMetadataRef.current;
            previewMetadataRef.current = {
              generationToken: meta.generation_token,
              sourceMediaFingerprint: meta.source_media_fingerprint,
              editVersion: meta.edit_version,
            };
            setPreviewAudioReady(false);
            setPreviewAudioUrl(convertFileSrc(meta.preview_url_safe_path));
            setPreviewCacheState("ready");
            if (
              stalePreview?.generationToken &&
              stalePreview.generationToken !== meta.generation_token
            ) {
              schedulePreviewInvalidation(stalePreview, "preview-replaced");
            }
          } else {
            previewMetadataRef.current = null;
            setPreviewCacheState("error");
          }
        } catch {
          if (seq === previewRenderSeq.current) {
            previewMetadataRef.current = null;
            setPreviewCacheState("error");
          }
        }
      })();
    }, 500);

    return () => {
      if (previewRenderTimerRef.current) {
        clearTimeout(previewRenderTimerRef.current);
        previewRenderTimerRef.current = null;
      }
    };
  }, [
    previewEdits,
    resetPreviewCache,
    schedulePreviewInvalidation,
    timingContract?.timeline_revision,
    words,
  ]);

  const handlePreviewCanPlay = useCallback(() => {
    setPreviewAudioReady(true);
  }, []);

  const handlePreviewAudioError = useCallback(() => {
    setPreviewAudioReady(false);
    setPreviewCacheState("error");
  }, []);

  return {
    previewAudioRef,
    previewEdits,
    setPreviewEdits,
    previewCacheState,
    setPreviewCacheState,
    previewAudioUrl,
    previewAudioReady,
    setPreviewAudioReady,
    usePreviewCache: usePreviewCacheFlag,
    previewCacheMode,
    previewToggleLabel,
    previewCacheModeLabel,
    hasVideoPreviewCandidate,
    isDualTrackVideoPreview,
    primarySrc,
    activePlaybackSrc,
    handlePreviewCanPlay,
    handlePreviewAudioError,
  };
}
