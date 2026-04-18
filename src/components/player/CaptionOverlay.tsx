import React, { useEffect, useMemo, useState } from "react";
import type { CaptionBlock, Rgba, Word } from "@/bindings";
import { commands } from "@/bindings";

interface CaptionOverlayProps {
  currentTime: number;
  words: Word[];
  enabled: boolean;
  videoRef?: React.RefObject<HTMLVideoElement | null>;
}

/**
 * Find the block active at the given time via binary search.
 */
function findBlockAtTime(
  blocks: CaptionBlock[],
  timeUs: number,
): CaptionBlock | null {
  let lo = 0;
  let hi = blocks.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >>> 1;
    const b = blocks[mid];
    if (timeUs < b.start_us) {
      hi = mid - 1;
    } else if (timeUs > b.end_us) {
      lo = mid + 1;
    } else {
      return b;
    }
  }
  return null;
}

function rgbaToCss(c: Rgba): string {
  return `rgba(${c.r},${c.g},${c.b},${(c.a / 255).toFixed(3)})`;
}

/**
 * Caption overlay that consumes the authoritative `CaptionBlock`
 * stream from the backend. Geometry is in video pixels; we scale to
 * the rendered `<video>` element so the preview visually matches the
 * burned-in export pixel-for-pixel (same font, same wrap, same
 * rounded-corner pill, same padding). See
 * `managers/captions/layout.rs` for the single source of truth.
 */
const CaptionOverlay: React.FC<CaptionOverlayProps> = ({
  currentTime,
  words,
  enabled,
  videoRef,
}) => {
  const [blocks, setBlocks] = useState<CaptionBlock[]>([]);
  const [renderedSize, setRenderedSize] = useState<{ w: number; h: number }>({
    w: 0,
    h: 0,
  });

  const wordsFingerprint = useMemo(
    () => words.map((w) => `${w.text}|${w.deleted}|${w.silenced}`).join(","),
    [words],
  );

  // Refetch blocks when the word list changes.
  useEffect(() => {
    let cancelled = false;
    if (!enabled || words.length === 0) {
      setBlocks([]);
      return;
    }
    commands
      .getCaptionBlocks("Source")
      .then((next) => {
        if (!cancelled) setBlocks(next);
      })
      .catch(() => {
        // Command may fail during startup if media isn't loaded yet.
      });
    return () => {
      cancelled = true;
    };
  }, [enabled, wordsFingerprint, words.length]);

  // Track the rendered `<video>` size so we can scale video-pixel
  // geometry into CSS pixels. Falls back to the layout's frame size
  // (1:1 scale) when no video ref is provided.
  useEffect(() => {
    const video = videoRef?.current;
    if (!video) return;
    const update = () => {
      const rect = video.getBoundingClientRect();
      setRenderedSize({ w: rect.width, h: rect.height });
    };
    update();
    const obs = new ResizeObserver(update);
    obs.observe(video);
    return () => obs.disconnect();
  }, [videoRef]);

  const block = useMemo(() => {
    if (!enabled || blocks.length === 0) return null;
    return findBlockAtTime(blocks, currentTime * 1_000_000);
  }, [enabled, currentTime, blocks]);

  if (!block) return null;

  // Scale: video-pixel → CSS-pixel. If we don't know the rendered
  // size yet, render at 1:1 (still correct geometry).
  const scale =
    renderedSize.h > 0 ? renderedSize.h / block.frame_height : 1;

  const boxStyle: React.CSSProperties = {
    position: "absolute",
    left: "50%",
    bottom: `${block.margin_v_px * scale}px`,
    transform: "translateX(-50%)",
    background: rgbaToCss(block.background),
    color: rgbaToCss(block.text_color),
    fontFamily: block.font_css,
    fontSize: `${block.font_size_px * scale}px`,
    lineHeight: `${block.line_height_px * scale}px`,
    padding: `${block.padding_y_px * scale}px ${block.padding_x_px * scale}px`,
    borderRadius: `${block.radius_px * scale}px`,
    pointerEvents: "none",
    textAlign: "center",
    whiteSpace: "pre",
  };

  return (
    <div style={boxStyle}>
      {block.lines.map((line, i) => (
        <div key={i}>{line}</div>
      ))}
    </div>
  );
};

export default CaptionOverlay;
