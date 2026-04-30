import { describe, it, expect } from "vitest";
import { mergeRangesUs, subtractRangesUs } from "./Waveform.util";
import type { Word } from "@/stores/editorStore";

function w(
  start_us: number,
  end_us: number,
  flags: { deleted?: boolean; silenced?: boolean } = {},
): Word {
  return {
    text: "x",
    start_us,
    end_us,
    deleted: flags.deleted ?? false,
    silenced: flags.silenced ?? false,
    confidence: 1.0,
    speaker_id: -1,
  } as Word;
}

describe("mergeRangesUs", () => {
  it("returns empty for no matches", () => {
    expect(mergeRangesUs([w(0, 100)], (x) => x.deleted)).toEqual([]);
  });

  it("collects a single matching word", () => {
    expect(
      mergeRangesUs([w(0, 100, { deleted: true })], (x) => x.deleted),
    ).toEqual([[0, 100]]);
  });

  it("merges two overlapping ranges into one swatch", () => {
    // Audio-truth case: a silence sentinel (50–200) overlaps a real
    // deleted word (0–100). The waveform must paint a single solid
    // span 0–200, not two translucent rectangles stacking.
    const words = [w(0, 100, { deleted: true }), w(50, 200, { deleted: true })];
    expect(mergeRangesUs(words, (x) => x.deleted)).toEqual([[0, 200]]);
  });

  it("merges adjacent (touching) ranges", () => {
    const words = [
      w(0, 100, { deleted: true }),
      w(100, 200, { deleted: true }),
    ];
    expect(mergeRangesUs(words, (x) => x.deleted)).toEqual([[0, 200]]);
  });

  it("keeps disjoint ranges separate", () => {
    const words = [
      w(0, 100, { deleted: true }),
      w(200, 300, { deleted: true }),
      w(500, 600, { deleted: true }),
    ];
    expect(mergeRangesUs(words, (x) => x.deleted)).toEqual([
      [0, 100],
      [200, 300],
      [500, 600],
    ]);
  });

  it("handles unsorted input", () => {
    const words = [
      w(500, 600, { deleted: true }),
      w(0, 100, { deleted: true }),
      w(200, 300, { deleted: true }),
    ];
    expect(mergeRangesUs(words, (x) => x.deleted)).toEqual([
      [0, 100],
      [200, 300],
      [500, 600],
    ]);
  });

  it("absorbs a fully-contained range", () => {
    const words = [
      w(0, 1000, { deleted: true }),
      w(100, 200, { deleted: true }),
    ];
    expect(mergeRangesUs(words, (x) => x.deleted)).toEqual([[0, 1000]]);
  });

  it("ignores zero/negative-duration entries", () => {
    const words = [
      w(0, 0, { deleted: true }),
      w(100, 50, { deleted: true }),
      w(200, 300, { deleted: true }),
    ];
    expect(mergeRangesUs(words, (x) => x.deleted)).toEqual([[200, 300]]);
  });
});

describe("subtractRangesUs", () => {
  it("returns base unchanged when forbidden is empty", () => {
    expect(subtractRangesUs([[0, 100]], [])).toEqual([[0, 100]]);
  });

  it("returns empty when base is empty", () => {
    expect(subtractRangesUs([], [[0, 100]])).toEqual([]);
  });

  it("removes a fully-overlapping forbidden range", () => {
    expect(subtractRangesUs([[100, 200]], [[0, 300]])).toEqual([]);
  });

  it("trims the leading edge", () => {
    expect(subtractRangesUs([[0, 200]], [[0, 100]])).toEqual([[100, 200]]);
  });

  it("trims the trailing edge", () => {
    expect(subtractRangesUs([[0, 200]], [[100, 200]])).toEqual([[0, 100]]);
  });

  it("punches a hole through the middle", () => {
    // Silenced overlay must not show under a deleted overlay — single
    // solid colour per pixel.
    expect(subtractRangesUs([[0, 300]], [[100, 200]])).toEqual([
      [0, 100],
      [200, 300],
    ]);
  });

  it("handles multiple forbidden ranges per base interval", () => {
    expect(
      subtractRangesUs(
        [[0, 1000]],
        [
          [100, 200],
          [400, 500],
          [800, 900],
        ],
      ),
    ).toEqual([
      [0, 100],
      [200, 400],
      [500, 800],
      [900, 1000],
    ]);
  });

  it("leaves base unchanged when forbidden lies outside", () => {
    expect(subtractRangesUs([[100, 200]], [[300, 400]])).toEqual([[100, 200]]);
  });
});
