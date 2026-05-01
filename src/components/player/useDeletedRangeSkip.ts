import { useEffect, useRef, type RefObject } from "react";
import type { TimeSegment } from "@/lib/utils/timeline";

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
function computeNextDeletedSkip(
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

interface UseDeletedRangeSkipParams {
  mediaRef: RefObject<HTMLMediaElement | null>;
  isPlaying: boolean;
  previewEdits: boolean;
  isPreviewCacheActive: boolean;
  isDualTrackVideoPreview: boolean;
  activeDeletedRanges: TimeSegment[];
  playbackRate: number;
  duration: number;
  seekVersion: number;
  /**
   * Shared with the RAF safety-net loop in MediaPlayer. The hook updates this
   * ref when it performs a scheduled skip so the RAF loop's monotonicity check
   * stays in sync.
   */
  lastSkipTargetRef: { current: number };
}

/**
 * Scheduled boundary-skip timer. Primary defense against deleted-range bleed:
 * we schedule a `setTimeout` to fire exactly when playback reaches the next
 * deleted range, then seek past its inclusive end. The RAF loop in MediaPlayer
 * remains as a safety fallback in case the timer is throttled or missed.
 */
export function useDeletedRangeSkip({
  mediaRef,
  isPlaying,
  previewEdits,
  isPreviewCacheActive,
  isDualTrackVideoPreview,
  activeDeletedRanges,
  playbackRate,
  duration,
  seekVersion,
  lastSkipTargetRef,
}: UseDeletedRangeSkipParams): void {
  const scheduledSkipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );

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
      const next = computeNextDeletedSkip(
        time,
        activeDeletedRanges,
        playbackRate,
      );
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
          Number.isFinite(elNow.duration) && elNow.duration > 0
            ? elNow.duration
            : duration;
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
    mediaRef,
    lastSkipTargetRef,
    isPlaying,
    previewEdits,
    isPreviewCacheActive,
    isDualTrackVideoPreview,
    activeDeletedRanges,
    playbackRate,
    duration,
    seekVersion,
  ]);
}
