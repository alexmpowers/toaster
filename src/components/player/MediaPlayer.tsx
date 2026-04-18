import React, { useCallback, useEffect, useRef, useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { usePlayerStore } from "@/stores/playerStore";
import { useEditorStore } from "@/stores/editorStore";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  DUAL_TRACK_DRIFT_THRESHOLD,
  DUAL_TRACK_SYNC_COOLDOWN_MS,
  getDeletedRanges,
  getDeletedRangesFromKeepSegments,
  editTimeToSourceTime,
  snapOutOfDeletedRange,
  type TimeSegment,
} from "@/lib/utils/timeline";
import { usePreviewCache } from "./usePreviewCache";
import PlaybackControls from "./PlaybackControls";
import CaptionOverlay from "./CaptionOverlay";

interface MediaPlayerProps {
  className?: string;
  onTimeUpdate?: (time: number) => void;
  captionsEnabled?: boolean;
}

/**
 * Fallback one-frame epsilon in seconds (≈20.8 µs at 48 kHz), used to step
 * playback just past the inclusive end-of-range sample when no audio sample
 * rate is known. Seeking to `range.end + ε` ensures the final sample of the
 * deleted word is not played.
 */
export const ONE_FRAME_EPSILON_FALLBACK = 1 / 48000;

/**
 * Pure helper — compute the next deleted-range boundary we need to skip past.
 *
 * Given the current playback time (in seconds), a set of deleted ranges, and
 * the current `playbackRate`, return the range we must skip next along with
 * the wall-clock delay (in ms) until playback will reach its start. If the
 * current time is already inside a range, `delayMs` is 0 and we should skip
 * immediately.
 *
 * Returns `null` when there is nothing more to skip (end of timeline).
 *
 * `deletedRanges` may be unsorted; the function picks the earliest-starting
 * range whose end is strictly after `currentTime`.
 */
export function computeNextDeletedSkip(
  currentTime: number,
  deletedRanges: ReadonlyArray<TimeSegment>,
  playbackRate: number,
): { range: TimeSegment; delayMs: number } | null {
  if (!(playbackRate > 0)) return null;
  let best: TimeSegment | null = null;
  for (const r of deletedRanges) {
    if (!(r.end > r.start)) continue;
    if (r.end <= currentTime) continue;
    if (!best || r.start < best.start) best = r;
  }
  if (!best) return null;
  const delaySec = Math.max(0, best.start - currentTime) / playbackRate;
  return { range: best, delayMs: delaySec * 1000 };
}

const MediaPlayer: React.FC<MediaPlayerProps> = ({
  className = "",
  onTimeUpdate,
  captionsEnabled = false,
}) => {
  const { t } = useTranslation();
  const mediaRef = useRef<HTMLVideoElement & HTMLAudioElement>(null);

  const {
    mediaUrl,
    mediaType,
    isPlaying,
    currentTime,
    duration,
    volume,
    playbackRate,
    seekVersion,
    seekTarget,
    setPlaying,
    setCurrentTime,
    setDuration,
    setVolume,
    setPlaybackRate,
  } = usePlayerStore();

  const words = useEditorStore((s) => s.words);
  const timingContract = useEditorStore((s) => s.timingContract);
  const experimentalSimplifyMode = useSettingsStore(
    (s) => s.settings?.experimental_simplify_mode ?? false,
  );

  const {
    previewAudioRef,
    previewEdits,
    setPreviewEdits,
    previewCacheState,
    setPreviewCacheState,
    previewAudioUrl,
    setPreviewAudioReady,
    usePreviewCache: isPreviewCacheActive,
    previewToggleLabel,
    previewCacheModeLabel,
    hasVideoPreviewCandidate,
    isDualTrackVideoPreview,
    primarySrc,
    activePlaybackSrc,
    handlePreviewCanPlay,
    handlePreviewAudioError,
  } = usePreviewCache({
    mediaUrl,
    mediaType,
    words,
    timingContract,
    experimentalSimplifyMode,
  });

  const [backendDeletedRanges, setBackendDeletedRanges] = useState<TimeSegment[] | null>(null);
  const [backendKeepSegments, setBackendKeepSegments] = useState<TimeSegment[]>([]);
  const backendFetchSeq = useRef(0);
  const lastSkipTargetRef = useRef(0);
  const lastObservedTimeRef = useRef(0);
  /** Real-clock timestamp (ms) of the last drift correction applied to the video element */
  const lastVideoSyncTimeRef = useRef(0);
  /** Tracks latest activeDeletedRanges for the play/pause effect without adding it to deps. */
  const activeDeletedRangesRef = useRef<TimeSegment[]>([]);
  /** Tracks whether fallback/live-skip mode is active for the play/pause effect. */
  const fallbackSkipModeRef = useRef(false);

  // Memoize deleted ranges so they aren't rebuilt every frame
  const deletedRanges = useMemo(() => getDeletedRanges(words, duration), [words, duration]);
  const activeDeletedRanges = backendDeletedRanges ?? deletedRanges;

  // Keep refs current so the play/pause effect can read them without adding
  // them to its dependency array (which would restart playback on every edit).
  useEffect(() => {
    activeDeletedRangesRef.current = activeDeletedRanges;
  }, [activeDeletedRanges]);
  useEffect(() => {
    fallbackSkipModeRef.current = previewEdits && !isPreviewCacheActive;
  }, [previewEdits, isPreviewCacheActive]);

  useEffect(() => {
    let isCancelled = false;
    const seq = ++backendFetchSeq.current;

    if (words.length === 0) {
      setBackendDeletedRanges([]);
      setBackendKeepSegments([]);
      return;
    }

    if (timingContract) {
      const keepSegments =
        timingContract.keep_segments?.length > 0
          ? timingContract.keep_segments
          : timingContract.quantized_keep_segments ?? [];
      setBackendDeletedRanges(getDeletedRangesFromKeepSegments(words, keepSegments));
      const normalized = keepSegments
        .map((s) => ({ start: s.start_us / 1_000_000, end: s.end_us / 1_000_000 }))
        .filter((s) => s.end > s.start)
        .sort((a, b) => a.start - b.start);
      setBackendKeepSegments(normalized);

      if (!timingContract.keep_segments_valid && timingContract.warning) {
        console.warn(
          `[timing-contract] revision=${timingContract.timeline_revision} warning=${timingContract.warning}`,
        );
      }
      return;
    }

    const refreshKeepSegments = async () => {
      try {
        const result = await commands.getKeepSegments();
        if (isCancelled || seq !== backendFetchSeq.current) return;
        if (result.status === "ok") {
          setBackendDeletedRanges(getDeletedRangesFromKeepSegments(words, result.data));
          const normalized = result.data
            .map((s) => ({ start: s.start_us / 1_000_000, end: s.end_us / 1_000_000 }))
            .filter((s) => s.end > s.start)
            .sort((a, b) => a.start - b.start);
          setBackendKeepSegments(normalized);
          return;
        }
      } catch {
        // Fallback to local deleted-ranges heuristic below
      }

      if (!isCancelled && seq === backendFetchSeq.current) {
        setBackendDeletedRanges(null);
        setBackendKeepSegments([]);
      }
    };

    void refreshKeepSegments();
    return () => {
      isCancelled = true;
    };
  }, [words, timingContract]);

  // Sync seek requests from the store to the media element(s)
  const lastSeekVersion = useRef(0);
  const pendingSeekRef = useRef<{ version: number; target: number } | null>(null);
  const seekFlushRafRef = useRef<number | null>(null);
  const lastAppliedSeekRef = useRef<{ target: number; ts: number } | null>(null);
  const seekContextRef = useRef<{
    dualTrack: boolean;
    keepSegments: TimeSegment[];
  }>({ dualTrack: false, keepSegments: [] });

  useEffect(() => {
    seekContextRef.current = {
      dualTrack: isDualTrackVideoPreview,
      keepSegments: backendKeepSegments,
    };
  }, [isDualTrackVideoPreview, backendKeepSegments]);

  useEffect(() => {
    const mediaEl = mediaRef.current;
    if (!mediaEl || seekVersion === lastSeekVersion.current) return;
    lastSeekVersion.current = seekVersion;

    // Latest-wins seek queue: coalesce multiple seek intents into one frame.
    pendingSeekRef.current = { version: seekVersion, target: seekTarget };

    if (seekFlushRafRef.current !== null) {
      return;
    }

    seekFlushRafRef.current = requestAnimationFrame(() => {
      seekFlushRafRef.current = null;
      const pending = pendingSeekRef.current;
      pendingSeekRef.current = null;
      if (!pending) return;

      const now = performance.now();
      const lastApplied = lastAppliedSeekRef.current;
      if (lastApplied && Math.abs(lastApplied.target - pending.target) < 0.0005 && now - lastApplied.ts < 30) {
        return;
      }

      const mediaNow = mediaRef.current;
      if (!mediaNow) return;
      const context = seekContextRef.current;

      if (context.dualTrack) {
        // Preview audio is the edit-time authority. Video follows mapped source time.
        const previewEl = previewAudioRef.current;
        if (previewEl) previewEl.currentTime = pending.target;
        const sourceTime =
          context.keepSegments.length > 0
            ? editTimeToSourceTime(pending.target, context.keepSegments)
            : pending.target;
        mediaNow.currentTime = sourceTime;
        lastVideoSyncTimeRef.current = now;

        // Verify seeks landed within tolerance after the next frame
        const tolerance = 0.05;
        requestAnimationFrame(() => {
          if (previewEl && Math.abs(previewEl.currentTime - pending.target) > tolerance) {
            previewEl.currentTime = pending.target;
          }
          if (mediaNow && Math.abs(mediaNow.currentTime - sourceTime) > tolerance) {
            mediaNow.currentTime = sourceTime;
          }
        });
      } else {
        mediaNow.currentTime = pending.target;
      }

      lastAppliedSeekRef.current = { target: pending.target, ts: now };
    });
  }, [seekVersion, seekTarget]);

  useEffect(() => {
    return () => {
      if (seekFlushRafRef.current !== null) {
        cancelAnimationFrame(seekFlushRafRef.current);
        seekFlushRafRef.current = null;
      }
      pendingSeekRef.current = null;
    };
  }, []);

  // When playback mode/source switches, reset playback position to 0
  const prevPlaybackKeyRef = useRef<string | null>(null);
  useEffect(() => {
    const playbackKey = isDualTrackVideoPreview
      ? `dual:${previewAudioUrl ?? "none"}`
      : `single:${primarySrc ?? "none"}`;
    if (playbackKey === prevPlaybackKeyRef.current) return;
    const wasSet = prevPlaybackKeyRef.current !== null;
    prevPlaybackKeyRef.current = playbackKey;
    if (!wasSet) return; // initial mount — do nothing
    const mediaEl = mediaRef.current;
    if (mediaEl) mediaEl.currentTime = 0;
    const previewEl = previewAudioRef.current;
    if (previewEl) previewEl.currentTime = 0;
    setCurrentTime(0);
  }, [isDualTrackVideoPreview, previewAudioUrl, primarySrc, setCurrentTime]);

  // Sync volume and playback rate to the element
  useEffect(() => {
    const mediaEl = mediaRef.current;
    if (mediaEl) {
      mediaEl.volume = isDualTrackVideoPreview ? 0 : volume;
      mediaEl.muted = isDualTrackVideoPreview;
    }
    const previewEl = previewAudioRef.current;
    if (previewEl) {
      previewEl.volume = volume;
    }
  }, [volume, isDualTrackVideoPreview]);

  useEffect(() => {
    const mediaEl = mediaRef.current;
    if (mediaEl) mediaEl.playbackRate = playbackRate;
    const previewEl = previewAudioRef.current;
    if (previewEl) previewEl.playbackRate = playbackRate;
  }, [playbackRate]);

  // Play/pause sync
  useEffect(() => {
    const mediaEl = mediaRef.current;
    if (!mediaEl || !activePlaybackSrc) return;
    const previewEl = previewAudioRef.current;
    let cancelled = false;

    const playWithFallback = async () => {
      if (isDualTrackVideoPreview && previewEl) {
        try {
          await mediaEl.play();
        } catch {
          if (!cancelled) setPlaying(false);
          return;
        }

        try {
          await previewEl.play();
          if (!cancelled) setPlaying(true);
        } catch (error) {
          if (cancelled) return;
          console.warn("Preview audio failed to start; falling back to single-track playback:", error);
          setPreviewCacheState("error");
          setPreviewAudioReady(false);
          previewEl.pause();
          mediaEl.muted = false;
          mediaEl.volume = volume;
          if (mediaEl.paused) {
            mediaEl.play().catch(() => setPlaying(false));
          } else {
            setPlaying(true);
          }
        }
        return;
      }

      // Pre-play snap: if currentTime is inside a deleted range in fallback/live-skip
      // mode, seek to the next kept boundary before calling play() to prevent startup
      // leakage of deleted audio. Uses exclusive-end semantics (range.end + ε) so the
      // final sample of the deleted word is skipped rather than played.
      if (fallbackSkipModeRef.current && activeDeletedRangesRef.current.length > 0) {
        const snapped = snapOutOfDeletedRange(mediaEl.currentTime, activeDeletedRangesRef.current);
        if (snapped !== mediaEl.currentTime) {
          mediaEl.currentTime = snapped + ONE_FRAME_EPSILON_FALLBACK;
        }
      }

      mediaEl.play()
        .then(() => {
          if (!cancelled) setPlaying(true);
        })
        .catch(() => {
          if (!cancelled) setPlaying(false);
        });
    };

    if (isPlaying) {
      void playWithFallback();
    } else {
      mediaEl.pause();
      previewEl?.pause();
    }

    return () => {
      cancelled = true;
    };
  }, [isPlaying, activePlaybackSrc, isDualTrackVideoPreview, setPlaying, volume]);

  // Scheduled boundary-skip timer. Primary defense against deleted-range bleed:
  // we schedule a `setTimeout` to fire exactly when playback reaches the next
  // deleted range, then seek past its inclusive end. The RAF loop below
  // remains as a safety fallback in case the timer is throttled or missed.
  const scheduledSkipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    // Scheduled skip is the fallback/live-skip path only: when preview edits
    // are on but no cached preview is available, and we're not in dual-track
    // video-preview mode (which uses the preview audio element + keep
    // segments for authoritative time).
    if (
      !isPlaying ||
      !previewEdits ||
      isPreviewCacheActive ||
      isDualTrackVideoPreview ||
      activeDeletedRanges.length === 0 ||
      !(playbackRate > 0)
    ) {
      if (scheduledSkipTimerRef.current !== null) {
        clearTimeout(scheduledSkipTimerRef.current);
        scheduledSkipTimerRef.current = null;
      }
      return;
    }

    let cancelled = false;
    const END_EPSILON = 0.005;

    const scheduleNextSkip = () => {
      if (cancelled) return;
      const el = mediaRef.current;
      if (!el) return;
      const time = el.currentTime;
      const next = computeNextDeletedSkip(time, activeDeletedRanges, playbackRate);
      if (!next) return;

      const fire = () => {
        scheduledSkipTimerRef.current = null;
        if (cancelled) return;
        const elNow = mediaRef.current;
        if (!elNow) return;

        // Re-check current time: the timer may have fired a few ms early/late.
        // Only skip if we're inside or past the start of the range.
        const timeNow = elNow.currentTime;
        if (timeNow >= next.range.end) {
          // Already past it (e.g. a user seek landed beyond). Just reschedule.
          scheduleNextSkip();
          return;
        }

        // Exclusive-end semantics: seek to `range.end + ε` so the final sample
        // of the deleted word is skipped rather than played. We prefer the
        // true audio sample rate if the media element exposes one; otherwise
        // fall back to 1/48000 (~20.8 µs).
        const elAny = elNow as HTMLMediaElement & { mozSampleRate?: number };
        const sr =
          typeof elAny.mozSampleRate === "number" && elAny.mozSampleRate > 0
            ? elAny.mozSampleRate
            : 0;
        const epsilon = sr > 0 ? 1 / sr : ONE_FRAME_EPSILON_FALLBACK;

        const mediaDuration =
          Number.isFinite(elNow.duration) && elNow.duration > 0 ? elNow.duration : duration;
        const maxSeekTarget =
          Number.isFinite(mediaDuration) && mediaDuration > 0
            ? Math.max(0, mediaDuration - END_EPSILON)
            : Number.POSITIVE_INFINITY;

        const rawTarget = next.range.end + epsilon;
        const finalTarget = Math.min(rawTarget, maxSeekTarget);

        if (finalTarget > timeNow) {
          lastSkipTargetRef.current = finalTarget;
          elNow.currentTime = finalTarget;
        }
        scheduleNextSkip();
      };

      if (next.delayMs <= 1) {
        // Already at/inside the range — skip on the next microtask so we
        // don't recurse synchronously beyond the stack.
        scheduledSkipTimerRef.current = setTimeout(fire, 0);
      } else {
        scheduledSkipTimerRef.current = setTimeout(fire, next.delayMs);
      }
    };

    scheduleNextSkip();

    return () => {
      cancelled = true;
      if (scheduledSkipTimerRef.current !== null) {
        clearTimeout(scheduledSkipTimerRef.current);
        scheduledSkipTimerRef.current = null;
      }
    };
  }, [
    isPlaying,
    previewEdits,
    isPreviewCacheActive,
    isDualTrackVideoPreview,
    activeDeletedRanges,
    playbackRate,
    duration,
    seekVersion,
  ]);

  // RAF-based playback loop: polls ~60fps for precise deleted-segment skipping
  // instead of relying on the ~4Hz onTimeUpdate event. With the scheduled
  // skip above as the primary defense, this loop serves as a safety net for
  // throttled timers and also drives `setCurrentTime` updates.
  const rafRef = useRef<number>(0);
  const lastFallbackSkipAtRef = useRef(0);
  const lastVideoSyncTargetRef = useRef(0);
  useEffect(() => {
    if (!isPlaying) {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = 0;
      }
      return;
    }

    const tick = () => {
      const el = isDualTrackVideoPreview ? previewAudioRef.current : mediaRef.current;
      if (!el) return;
      const time = el.currentTime;
      if (time + 0.05 < lastObservedTimeRef.current) {
        lastSkipTargetRef.current = 0;
        lastFallbackSkipAtRef.current = 0;
      }
      lastObservedTimeRef.current = time;
      const END_EPSILON = 0.005; // 5ms
      const nowTick = performance.now();
      const mediaDuration =
        Number.isFinite(el.duration) && el.duration > 0 ? el.duration : duration;
      const maxSeekTarget =
        Number.isFinite(mediaDuration) && mediaDuration > 0
          ? Math.max(0, mediaDuration - END_EPSILON)
          : Number.POSITIVE_INFINITY;

      // Skip deleted segments when preview edits is on but no cached preview is available.
      // This is the safety-net path: the scheduled-skip effect above normally handles
      // boundary skips precisely. RAF catches anything the timer misses (e.g. throttled
      // background tab). Debounce is kept short (10 ms) so back-to-back short deletions
      // don't bleed, while still preventing thrash if the element's currentTime lags.
      if (previewEdits && !isPreviewCacheActive && activeDeletedRanges.length > 0) {
        for (const range of activeDeletedRanges) {
          if (time >= range.start && time < range.end) {
            // Exclusive-end semantics: land at `range.end + ε` so the final sample
            // of the deleted word is skipped rather than played.
            const elAny = el as HTMLMediaElement & { mozSampleRate?: number };
            const sr =
              typeof elAny.mozSampleRate === "number" && elAny.mozSampleRate > 0
                ? elAny.mozSampleRate
                : 0;
            const epsilon = sr > 0 ? 1 / sr : ONE_FRAME_EPSILON_FALLBACK;
            const rawTarget = Math.min(range.end + epsilon, maxSeekTarget);
            const monotonicTarget = Math.max(rawTarget, lastSkipTargetRef.current + epsilon);
            const finalTarget = Math.min(monotonicTarget, maxSeekTarget);
            if (finalTarget > time + epsilon && nowTick - lastFallbackSkipAtRef.current > 10) {
              lastFallbackSkipAtRef.current = nowTick;
              lastSkipTargetRef.current = finalTarget;
              el.currentTime = finalTarget;
              // Don't update store yet — next frame will read the new position
              rafRef.current = requestAnimationFrame(tick);
              return;
            }
            break;
          }
        }
      }

      // Dual-track: keep video element in sync with preview audio (the time authority).
      // Preview audio plays the edit timeline; video must show the matching source frame.
      // Only correct when drift exceeds the threshold and the cooldown has elapsed, to
      // avoid jitter from constant micro-corrections.
      if (isDualTrackVideoPreview && backendKeepSegments.length > 0) {
        const videoEl = mediaRef.current;
        if (videoEl) {
          const targetSourceTime = editTimeToSourceTime(time, backendKeepSegments);
          const drift = Math.abs(videoEl.currentTime - targetSourceTime);
          const now = performance.now();
          if (
            drift > DUAL_TRACK_DRIFT_THRESHOLD &&
            now - lastVideoSyncTimeRef.current > DUAL_TRACK_SYNC_COOLDOWN_MS
          ) {
            videoEl.currentTime = targetSourceTime;
            lastVideoSyncTimeRef.current = now;
            lastVideoSyncTargetRef.current = targetSourceTime;
          }
        }
      }

      setCurrentTime(time);
      onTimeUpdate?.(time);
      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = 0;
      }
    };
  }, [isPlaying, previewEdits, isPreviewCacheActive, activeDeletedRanges, duration, setCurrentTime, onTimeUpdate, isDualTrackVideoPreview, backendKeepSegments]);

  // Fallback onTimeUpdate for when paused (seek bar scrubbing, etc.)
  const handleTimeUpdate = useCallback(() => {
    if (isPlaying) return; // RAF loop handles this during playback
    const el = isDualTrackVideoPreview ? previewAudioRef.current : mediaRef.current;
    if (!el) return;
    setCurrentTime(el.currentTime);
    onTimeUpdate?.(el.currentTime);
  }, [isPlaying, setCurrentTime, onTimeUpdate, isDualTrackVideoPreview]);

  const handleLoadedMetadata = useCallback((e: React.SyntheticEvent<HTMLVideoElement | HTMLAudioElement>) => {
    const targetEl = e.currentTarget;
    setDuration(targetEl.duration);
    targetEl.playbackRate = playbackRate;

    if (targetEl === previewAudioRef.current) {
      setPreviewAudioReady(true);
      targetEl.volume = volume;
      return;
    }

    if (isDualTrackVideoPreview) {
      targetEl.volume = 0;
      targetEl.muted = true;
      return;
    }

    targetEl.volume = volume;
    targetEl.muted = false;
  }, [setDuration, volume, playbackRate, isDualTrackVideoPreview]);

  const handlePlay= useCallback(() => setPlaying(true), [setPlaying]);
  const handlePause = useCallback(() => setPlaying(false), [setPlaying]);

  const togglePlay = useCallback(() => {
    setPlaying(!isPlaying);
  }, [isPlaying, setPlaying]);

  const handleRestart = useCallback(() => {
    const mediaEl = mediaRef.current;
    if (mediaEl) mediaEl.currentTime = 0;
    const previewEl = previewAudioRef.current;
    if (previewEl) previewEl.currentTime = 0;
  }, []);

  const handleRewind = useCallback(() => {
    const skipSeconds = 5;
    if (isDualTrackVideoPreview) {
      const previewEl = previewAudioRef.current;
      if (previewEl) previewEl.currentTime = Math.max(0, previewEl.currentTime - skipSeconds);
    } else {
      const mediaEl = mediaRef.current;
      if (mediaEl) mediaEl.currentTime = Math.max(0, mediaEl.currentTime - skipSeconds);
    }
  }, [isDualTrackVideoPreview]);

  const toggleMute = useCallback(() => {
    setVolume(volume === 0 ? 1 : 0);
  }, [volume, setVolume]);

  const handleSeekBarChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const editTime = parseFloat(e.target.value);
      if (isDualTrackVideoPreview) {
        // Seek preview audio (edit-time authority) and video (source time) independently
        const previewEl = previewAudioRef.current;
        if (previewEl) previewEl.currentTime = editTime;
        const mediaEl = mediaRef.current;
        if (mediaEl) {
          const sourceTime =
            backendKeepSegments.length > 0
              ? editTimeToSourceTime(editTime, backendKeepSegments)
              : editTime;
          mediaEl.currentTime = sourceTime;
          lastVideoSyncTimeRef.current = performance.now();
        }
      } else {
        const mediaEl = mediaRef.current;
        if (mediaEl) mediaEl.currentTime = editTime;
      }
      setCurrentTime(editTime);
      onTimeUpdate?.(editTime);
    },
    [setCurrentTime, onTimeUpdate, isDualTrackVideoPreview, backendKeepSegments],
  );

  const handleVolumeChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setVolume(parseFloat(e.target.value));
    },
    [setVolume],
  );

  const handleRateChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      setPlaybackRate(parseFloat(e.target.value));
    },
    [setPlaybackRate],
  );

  if (!mediaUrl || !mediaType) {
    return (
      <div
        className={`flex items-center justify-center bg-neutral-900 text-neutral-500 rounded-lg p-8 ${className}`}
      >
        {t("player.noMedia")}
      </div>
    );
  }

  const MediaTag = mediaType === "video" ? "video" : "audio";
  const showVideoDisplay = mediaType === "video";

  return (
    <div className={`flex flex-col bg-neutral-900 rounded-lg ${className}`}>
      {/* Primary media element — video src remains stable even when preview cache is active */}
      <div className="relative">
        <MediaTag
          ref={mediaRef}
          src={primarySrc ?? undefined}
          onTimeUpdate={handleTimeUpdate}
          onLoadedMetadata={handleLoadedMetadata}
          onPlay={handlePlay}
          onPause={handlePause}
          onEnded={handlePause}
          className={
            showVideoDisplay
              ? "w-full rounded-t-lg bg-black"
              : "hidden"
          }
          preload="metadata"
        />
        {showVideoDisplay && (
          <CaptionOverlay
            currentTime={currentTime}
            words={words}
            enabled={captionsEnabled}
            videoRef={mediaRef as React.RefObject<HTMLVideoElement | null>}
          />
        )}
      </div>
      {hasVideoPreviewCandidate && (
        <audio
          ref={previewAudioRef}
          src={previewAudioUrl ?? undefined}
          onLoadedMetadata={handleLoadedMetadata}
          onCanPlay={handlePreviewCanPlay}
          onTimeUpdate={handleTimeUpdate}
          onError={handlePreviewAudioError}
          onEnded={handlePause}
          className="hidden"
          preload="metadata"
        />
      )}

      {/* Controls */}
      <PlaybackControls
        currentTime={currentTime}
        duration={duration}
        isPlaying={isPlaying}
        volume={volume}
        playbackRate={playbackRate}
        previewEdits={previewEdits}
        previewCacheState={previewCacheState}
        previewToggleLabel={previewToggleLabel}
        previewCacheModeLabel={previewCacheModeLabel}
        hasWords={words.length > 0}
        onTogglePlay={togglePlay}
        onRestart={handleRestart}
        onRewind={handleRewind}
        onSeekBarChange={handleSeekBarChange}
        onToggleMute={toggleMute}
        onVolumeChange={handleVolumeChange}
        onRateChange={handleRateChange}
        onTogglePreviewEdits={() => setPreviewEdits(!previewEdits)}
      />
    </div>
  );
};

export default MediaPlayer;
