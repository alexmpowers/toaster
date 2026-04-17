import React, { useMemo, useEffect, useRef, useCallback } from "react";
import type { Word } from "@/bindings";
import { commands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";

interface CaptionOverlayProps {
  currentTime: number;
  words: Word[];
  enabled: boolean;
}

interface CachedSegment {
  start_us: number;
  end_us: number;
  text: string;
}

/**
 * Parse a hex color string (e.g. #000000B3) into an rgba() CSS value.
 */
function hexToRgba(hex: string): string {
  const h = hex.replace("#", "");
  const r = parseInt(h.slice(0, 2), 16) || 0;
  const g = parseInt(h.slice(2, 4), 16) || 0;
  const b = parseInt(h.slice(4, 6), 16) || 0;
  const a = h.length > 6 ? parseInt(h.slice(6, 8), 16) / 255 : 1;
  return `rgba(${r},${g},${b},${a.toFixed(2)})`;
}

/**
 * Find the caption segment active at the given time via binary search.
 */
function findSegmentAtTime(
  segments: CachedSegment[],
  timeUs: number,
): string | null {
  let lo = 0;
  let hi = segments.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >>> 1;
    const seg = segments[mid];
    if (timeUs < seg.start_us) {
      hi = mid - 1;
    } else if (timeUs > seg.end_us) {
      lo = mid + 1;
    } else {
      return seg.text;
    }
  }
  return null;
}

const CaptionOverlay: React.FC<CaptionOverlayProps> = ({
  currentTime,
  words,
  enabled,
}) => {
  const getSetting = useSettingsStore((s) => s.getSetting);
  const segmentsRef = useRef<CachedSegment[]>([]);

  // Stable identity for word list to avoid spurious fetches
  const wordsFingerprint = useMemo(() => {
    return words
      .map((w) => `${w.text}|${w.deleted}|${w.silenced}`)
      .join(",");
  }, [words]);

  const fetchSegments = useCallback(async () => {
    try {
      const segments = await commands.getCaptionSegments();
      segmentsRef.current = segments;
    } catch {
      // Command not available yet (e.g. during startup)
    }
  }, []);

  useEffect(() => {
    if (enabled && words.length > 0) {
      fetchSegments();
    } else {
      segmentsRef.current = [];
    }
  }, [enabled, wordsFingerprint, fetchSegments]);

  const fontSize = (getSetting("caption_font_size") as number) ?? 24;
  const bgColor = (getSetting("caption_bg_color") as string) ?? "#000000B3";
  const textColor = (getSetting("caption_text_color") as string) ?? "#FFFFFF";
  const position = (getSetting("caption_position") as number) ?? 90;

  const captionText = useMemo(() => {
    if (!enabled || segmentsRef.current.length === 0) return null;
    const currentTimeUs = currentTime * 1_000_000;
    return findSegmentAtTime(segmentsRef.current, currentTimeUs);
  }, [enabled, currentTime]);

  if (!captionText) return null;

  const bottomPercent = `${100 - position}%`;

  return (
    <div
      style={{
        position: "absolute",
        bottom: bottomPercent,
        left: "50%",
        transform: "translateX(-50%)",
        background: hexToRgba(bgColor),
        color: textColor,
        padding: "4px 12px",
        borderRadius: "4px",
        fontSize: `${fontSize}px`,
        pointerEvents: "none",
        maxWidth: "90%",
        textAlign: "center",
        whiteSpace: "pre-wrap",
      }}
    >
      {captionText}
    </div>
  );
};

export default CaptionOverlay;
