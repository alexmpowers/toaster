import React, { useCallback, useEffect, useRef, useState } from "react";
import { type Word } from "@/stores/editorStore";
import { mergeRangesUs, subtractRangesUs } from "./Waveform.util";

interface WaveformProps {
  audioUrl: string | null;
  currentTime: number;
  duration: number;
  onSeek: (time: number) => void;
  words?: Word[];
  selectedWordIndex?: number | null;
  className?: string;
}

const BAR_COUNT = 300;
const COARSE_BAR_COUNT = 180;
const LONG_MEDIA_COARSE_THRESHOLD_SECONDS = 20 * 60;
const BAR_GAP = 1;
// Canvas fillStyle strings. Brand-carrying values are read from the
// `--color-logo-primary` CSS var at draw time so a token change in
// App.css flows through without touching this file. CSS named colors
// are used as last-resort fallbacks (not hex literals) so the
// design-tokens drift gate stays clean.
const UNPLAYED_COLOR = "rgb(74, 74, 74)";
const DELETED_OVERLAY = "rgba(220, 38, 38, 0.25)";
const SILENCED_OVERLAY = "rgba(234, 179, 8, 0.15)";
const WORD_BOUNDARY_COLOR = "rgba(255, 255, 255, 0.08)";
const WAVEFORM_RETRY_DELAYS_MS = [120, 360];
const WAVEFORM_CACHE_MAX_ENTRIES = 12;

function readBrandColor(): string {
  if (typeof window === "undefined") return "goldenrod";
  const v = getComputedStyle(document.documentElement)
    .getPropertyValue("--color-logo-primary")
    .trim();
  return v || "goldenrod";
}

/**
 * Given a `#RRGGBB` hex (as returned by getComputedStyle), produce an
 * `rgba(r, g, b, a)` string for Canvas 2D. Returns the input unchanged
 * for non-hex inputs (e.g. CSS named color fallback).
 */
function withAlpha(hex: string, alpha: number): string {
  const m = /^#([0-9a-fA-F]{6})$/.exec(hex.trim());
  if (!m) return hex;
  const r = parseInt(m[1].slice(0, 2), 16);
  const g = parseInt(m[1].slice(2, 4), 16);
  const b = parseInt(m[1].slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

const waveformPeaksCache = new Map<string, number[]>();

const delay = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));

function downsamplePeaks(
  channelData: Float32Array,
  barCount: number,
): number[] {
  if (barCount <= 0 || channelData.length === 0) return [];

  const sampleCount = channelData.length;
  const peaks: number[] = [];
  for (let i = 0; i < barCount; i++) {
    let max = 0;
    const start = Math.floor((i / barCount) * sampleCount);
    const end = Math.max(
      start + 1,
      Math.floor(((i + 1) / barCount) * sampleCount),
    );
    for (let j = start; j < end; j++) {
      const abs = Math.abs(channelData[j]);
      if (abs > max) max = abs;
    }
    peaks.push(max);
  }
  const globalMax = Math.max(...peaks, 0.01);
  return peaks.map((p) => p / globalMax);
}

const Waveform: React.FC<WaveformProps> = ({
  audioUrl,
  currentTime,
  duration,
  onSeek,
  words = [],
  selectedWordIndex = null,
  className = "",
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [peaks, setPeaks] = useState<number[]>([]);
  const [canvasWidth, setCanvasWidth] = useState(0);
  const canvasHeight = 64;

  // Decode audio and extract waveform peaks
  useEffect(() => {
    if (!audioUrl) {
      setPeaks([]);
      return;
    }

    const targetBarCount =
      duration > LONG_MEDIA_COARSE_THRESHOLD_SECONDS
        ? COARSE_BAR_COUNT
        : BAR_COUNT;
    const cacheKey = `${audioUrl}::${targetBarCount}`;
    const cached = waveformPeaksCache.get(cacheKey);
    if (cached) {
      setPeaks(cached);
      return;
    }

    let cancelled = false;
    const controller = new AbortController();

    const loadAudio = async () => {
      for (
        let attempt = 0;
        attempt <= WAVEFORM_RETRY_DELAYS_MS.length;
        attempt++
      ) {
        try {
          const response = await fetch(audioUrl, {
            signal: controller.signal,
            cache: "force-cache",
          });
          if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
          }

          const arrayBuffer = await response.arrayBuffer();
          const audioCtx = new AudioContext();
          let extracted: number[] = [];
          try {
            const audioBuffer = await audioCtx.decodeAudioData(arrayBuffer);
            const channelData = audioBuffer.getChannelData(0);
            extracted = downsamplePeaks(channelData, targetBarCount);
          } finally {
            await audioCtx.close().catch(() => undefined);
          }

          if (cancelled || controller.signal.aborted) return;
          waveformPeaksCache.set(cacheKey, extracted);
          while (waveformPeaksCache.size > WAVEFORM_CACHE_MAX_ENTRIES) {
            const oldestKey = waveformPeaksCache.keys().next().value;
            if (!oldestKey) break;
            waveformPeaksCache.delete(oldestKey);
          }
          setPeaks(extracted);
          return;
        } catch (err) {
          if (cancelled || controller.signal.aborted) return;
          if (attempt < WAVEFORM_RETRY_DELAYS_MS.length) {
            await delay(WAVEFORM_RETRY_DELAYS_MS[attempt]);
            continue;
          }
          console.error("Failed to decode audio for waveform:", err);
          setPeaks([]);
        }
      }
    };

    loadAudio();
    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [audioUrl, duration]);

  // Observe container resize to keep canvas responsive
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setCanvasWidth(entry.contentRect.width);
      }
    });
    observer.observe(container);
    setCanvasWidth(container.clientWidth);

    return () => observer.disconnect();
  }, []);

  // Draw waveform with overlays
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || peaks.length === 0 || canvasWidth === 0) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = canvasWidth * dpr;
    canvas.height = canvasHeight * dpr;
    ctx.scale(dpr, dpr);

    ctx.clearRect(0, 0, canvasWidth, canvasHeight);

    // Resolve brand-coloured draw styles once per draw (token flows from
    // `--color-logo-primary` — see docs/design-tokens.md).
    const playedColor = readBrandColor();
    const selectedWordColor = withAlpha(playedColor, 0.3);

    const barWidth = Math.max(
      1,
      (canvasWidth - (peaks.length - 1) * BAR_GAP) / peaks.length,
    );
    const progress = duration > 0 ? currentTime / duration : 0;
    const playedBars = Math.floor(progress * peaks.length);

    const midY = canvasHeight / 2;
    const maxBarHeight = canvasHeight * 0.8;

    // Draw bars
    for (let i = 0; i < peaks.length; i++) {
      const x = i * (barWidth + BAR_GAP);
      const barH = Math.max(2, peaks[i] * maxBarHeight);
      ctx.fillStyle = i < playedBars ? playedColor : UNPLAYED_COLOR;
      ctx.fillRect(x, midY - barH / 2, barWidth, barH);
    }

    // Draw edit overlays (deleted/silenced regions). Merge overlapping
    // ranges first so a sentinel that overlaps a real word does not
    // stack two translucent layers into a darker stripe — a single
    // solid swatch across the merged span instead.
    if (duration > 0 && words.length > 0) {
      const deletedRanges = mergeRangesUs(words, (w) => w.deleted);
      const silencedRanges = subtractRangesUs(
        mergeRangesUs(words, (w) => w.silenced && !w.deleted),
        deletedRanges,
      );

      ctx.fillStyle = SILENCED_OVERLAY;
      for (const [startUs, endUs] of silencedRanges) {
        const startX = (startUs / 1_000_000 / duration) * canvasWidth;
        const endX = (endUs / 1_000_000 / duration) * canvasWidth;
        ctx.fillRect(startX, 0, Math.max(1, endX - startX), canvasHeight);
      }

      ctx.fillStyle = DELETED_OVERLAY;
      for (const [startUs, endUs] of deletedRanges) {
        const startX = (startUs / 1_000_000 / duration) * canvasWidth;
        const endX = (endUs / 1_000_000 / duration) * canvasWidth;
        ctx.fillRect(startX, 0, Math.max(1, endX - startX), canvasHeight);
      }

      // Draw selected word highlight
      if (selectedWordIndex !== null && words[selectedWordIndex]) {
        const sw = words[selectedWordIndex];
        const startX = (sw.start_us / 1_000_000 / duration) * canvasWidth;
        const endX = (sw.end_us / 1_000_000 / duration) * canvasWidth;
        ctx.fillStyle = selectedWordColor;
        ctx.fillRect(startX, 0, Math.max(2, endX - startX), canvasHeight);
      }

      // Draw word boundary lines (subtle)
      ctx.strokeStyle = WORD_BOUNDARY_COLOR;
      ctx.lineWidth = 1;
      for (const word of words) {
        if (word.deleted) continue;
        const x = (word.start_us / 1_000_000 / duration) * canvasWidth;
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, canvasHeight);
        ctx.stroke();
      }
    }

    // Draw playhead
    if (duration > 0) {
      const playheadX = progress * canvasWidth;
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(playheadX, 0);
      ctx.lineTo(playheadX, canvasHeight);
      ctx.stroke();
    }
  }, [peaks, currentTime, duration, canvasWidth, words, selectedWordIndex]);

  const handleClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas || duration <= 0) return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const ratio = Math.max(0, Math.min(1, x / rect.width));
      onSeek(ratio * duration);
    },
    [duration, onSeek],
  );

  if (!audioUrl) return null;

  return (
    <div ref={containerRef} className={`w-full ${className}`}>
      <canvas
        ref={canvasRef}
        onClick={handleClick}
        className="w-full cursor-pointer rounded"
        style={{ height: canvasHeight }}
      />
    </div>
  );
};

export default Waveform;
