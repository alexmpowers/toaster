import { describe, it, expect } from "vitest";
import {
  editTimeToSourceTime,
  sourceTimeToEditTime,
  getDeletedRanges,
  getDeletedRangesFromKeepSegments,
  snapOutOfDeletedRange,
  type TimeSegment,
} from "./timeline";
import type { Word } from "@/stores/editorStore";

// Helper to create a minimal Word for testing
function makeWord(
  start_us: number,
  end_us: number,
  deleted = false,
): Word {
  return {
    text: "test",
    start_us,
    end_us,
    deleted,
    silenced: false,
    confidence: 1.0,
    speaker_id: -1,
  } as Word;
}

// ── editTimeToSourceTime ─────────────────────────────────────────────────────

describe("editTimeToSourceTime", () => {
  it("returns identity when keepSegments is empty", () => {
    expect(editTimeToSourceTime(5.0, [])).toBe(5.0);
  });

  it("maps within a single keep-segment", () => {
    const segs: TimeSegment[] = [{ start: 2.0, end: 6.0 }];
    // editTime 1.0 → 1.0 seconds into the segment → source 3.0
    expect(editTimeToSourceTime(1.0, segs)).toBe(3.0);
  });

  it("maps across multiple keep-segments", () => {
    // keep [0,2] and [4,6] → 4 seconds of content
    const segs: TimeSegment[] = [
      { start: 0.0, end: 2.0 },
      { start: 4.0, end: 6.0 },
    ];
    // editTime 0.0 → source 0.0
    expect(editTimeToSourceTime(0.0, segs)).toBe(0.0);
    // editTime 1.0 → source 1.0 (within first segment)
    expect(editTimeToSourceTime(1.0, segs)).toBe(1.0);
    // editTime 2.0 → starts second segment → source 4.0
    expect(editTimeToSourceTime(2.0, segs)).toBe(4.0);
    // editTime 3.0 → 1 second into second segment → source 5.0
    expect(editTimeToSourceTime(3.0, segs)).toBe(5.0);
  });

  it("clamps to end of last segment when editTime exceeds total duration", () => {
    const segs: TimeSegment[] = [
      { start: 0.0, end: 2.0 },
      { start: 4.0, end: 6.0 },
    ];
    // Total keep duration = 4s, so editTime=10 → clamp to end of last segment
    expect(editTimeToSourceTime(10.0, segs)).toBe(6.0);
  });

  it("is monotonically non-decreasing", () => {
    const segs: TimeSegment[] = [
      { start: 1.0, end: 3.0 },
      { start: 5.0, end: 8.0 },
      { start: 10.0, end: 12.0 },
    ];
    let prev = -Infinity;
    for (let t = 0; t <= 8; t += 0.1) {
      const mapped = editTimeToSourceTime(t, segs);
      expect(mapped).toBeGreaterThanOrEqual(prev);
      prev = mapped;
    }
  });
});

// ── sourceTimeToEditTime ─────────────────────────────────────────────────────

describe("sourceTimeToEditTime", () => {
  it("returns identity when keepSegments is empty", () => {
    expect(sourceTimeToEditTime(5.0, [])).toBe(5.0);
  });

  it("maps within a single keep-segment", () => {
    const segs: TimeSegment[] = [{ start: 2.0, end: 6.0 }];
    // source 3.0 → 1 second into segment → edit 1.0
    expect(sourceTimeToEditTime(3.0, segs)).toBe(1.0);
  });

  it("snaps forward when sourceTime is in a deleted region", () => {
    const segs: TimeSegment[] = [
      { start: 0.0, end: 2.0 },
      { start: 4.0, end: 6.0 },
    ];
    // source 3.0 is in deleted region [2,4] → snap to start of next keep = editTime 2.0
    expect(sourceTimeToEditTime(3.0, segs)).toBe(2.0);
  });

  it("maps across multiple keep-segments correctly", () => {
    const segs: TimeSegment[] = [
      { start: 0.0, end: 2.0 },
      { start: 4.0, end: 6.0 },
    ];
    expect(sourceTimeToEditTime(0.0, segs)).toBe(0.0);
    expect(sourceTimeToEditTime(1.0, segs)).toBe(1.0);
    expect(sourceTimeToEditTime(4.0, segs)).toBe(2.0);
    expect(sourceTimeToEditTime(5.0, segs)).toBe(3.0);
  });

  it("result never exceeds total keep duration", () => {
    const segs: TimeSegment[] = [
      { start: 0.0, end: 2.0 },
      { start: 4.0, end: 6.0 },
    ];
    // Total keep = 4s. Source 100.0 → should not exceed 4.0
    expect(sourceTimeToEditTime(100.0, segs)).toBe(4.0);
  });

  it("result is always >= 0", () => {
    const segs: TimeSegment[] = [{ start: 5.0, end: 10.0 }];
    // source before any segment
    expect(sourceTimeToEditTime(0.0, segs)).toBe(0.0);
    expect(sourceTimeToEditTime(3.0, segs)).toBe(0.0);
  });
});

// ── Round-trip consistency ───────────────────────────────────────────────────

describe("editTime ↔ sourceTime round-trip", () => {
  it("editToSource then sourceToEdit returns original for kept positions", () => {
    const segs: TimeSegment[] = [
      { start: 1.0, end: 3.0 },
      { start: 5.0, end: 8.0 },
    ];
    // Total keep = 5s
    for (const editTime of [0, 0.5, 1.0, 2.0, 3.5, 4.9]) {
      const source = editTimeToSourceTime(editTime, segs);
      const backToEdit = sourceTimeToEditTime(source, segs);
      expect(backToEdit).toBeCloseTo(editTime, 10);
    }
  });
});

// ── snapOutOfDeletedRange ────────────────────────────────────────────────────

describe("snapOutOfDeletedRange", () => {
  const deleted: TimeSegment[] = [
    { start: 2.0, end: 4.0 },
    { start: 7.0, end: 9.0 },
  ];

  it("returns time unchanged when outside deleted ranges", () => {
    expect(snapOutOfDeletedRange(1.0, deleted)).toBe(1.0);
    expect(snapOutOfDeletedRange(5.0, deleted)).toBe(5.0);
    expect(snapOutOfDeletedRange(10.0, deleted)).toBe(10.0);
  });

  it("snaps to end of range when inside a deleted range", () => {
    expect(snapOutOfDeletedRange(2.5, deleted)).toBe(4.0);
    expect(snapOutOfDeletedRange(7.0, deleted)).toBe(9.0);
    expect(snapOutOfDeletedRange(8.5, deleted)).toBe(9.0);
  });

  it("returns time unchanged when at the end boundary (half-open)", () => {
    expect(snapOutOfDeletedRange(4.0, deleted)).toBe(4.0);
    expect(snapOutOfDeletedRange(9.0, deleted)).toBe(9.0);
  });

  it("handles empty ranges", () => {
    expect(snapOutOfDeletedRange(5.0, [])).toBe(5.0);
  });
});

// ── getDeletedRanges ─────────────────────────────────────────────────────────

describe("getDeletedRanges", () => {
  it("returns empty array when no words are deleted", () => {
    const words = [
      makeWord(0, 1_000_000),
      makeWord(1_000_000, 2_000_000),
    ];
    expect(getDeletedRanges(words, 3.0)).toEqual([]);
  });

  it("creates a range for a single deleted word", () => {
    const words = [
      makeWord(0, 1_000_000),
      makeWord(1_000_000, 2_000_000, true),
      makeWord(2_000_000, 3_000_000),
    ];
    const ranges = getDeletedRanges(words, 3.0);
    expect(ranges).toHaveLength(1);
    // With 10ms crossfade pad
    expect(ranges[0].start).toBeCloseTo(0.99, 2);
    expect(ranges[0].end).toBeCloseTo(2.01, 2);
  });

  it("merges adjacent deleted words within 50ms gap", () => {
    const words = [
      makeWord(1_000_000, 2_000_000, true),
      makeWord(2_020_000, 3_000_000, true), // 20ms gap — should merge
    ];
    const ranges = getDeletedRanges(words, 5.0);
    expect(ranges).toHaveLength(1);
  });

  it("does NOT merge deleted words with gap > 50ms", () => {
    const words = [
      makeWord(1_000_000, 2_000_000, true),
      makeWord(2_100_000, 3_000_000, true), // 100ms gap — separate ranges
    ];
    const ranges = getDeletedRanges(words, 5.0);
    expect(ranges).toHaveLength(2);
  });

  it("clamps ranges to duration", () => {
    const words = [
      makeWord(0, 500_000, true),
    ];
    const ranges = getDeletedRanges(words, 0.3);
    expect(ranges).toHaveLength(1);
    expect(ranges[0].end).toBeLessThanOrEqual(0.3);
  });
});

// ── getDeletedRangesFromKeepSegments ─────────────────────────────────────────

describe("getDeletedRangesFromKeepSegments", () => {
  it("returns empty when no words", () => {
    const segs = [{ start_us: 0, end_us: 1_000_000 }];
    expect(getDeletedRangesFromKeepSegments([], segs)).toEqual([]);
  });

  it("returns empty when keepSegments cover entire transcript", () => {
    const words = [
      makeWord(0, 1_000_000),
      makeWord(1_000_000, 2_000_000),
    ];
    const segs = [{ start_us: 0, end_us: 2_000_000 }];
    expect(getDeletedRangesFromKeepSegments(words, segs)).toEqual([]);
  });

  it("identifies gap between keep-segments as deleted", () => {
    const words = [
      makeWord(0, 1_000_000),
      makeWord(1_000_000, 2_000_000),
      makeWord(2_000_000, 3_000_000),
    ];
    const segs = [
      { start_us: 0, end_us: 1_000_000 },
      { start_us: 2_000_000, end_us: 3_000_000 },
    ];
    const ranges = getDeletedRangesFromKeepSegments(words, segs);
    expect(ranges).toHaveLength(1);
    expect(ranges[0].start).toBeCloseTo(1.0, 5);
    expect(ranges[0].end).toBeCloseTo(2.0, 5);
  });

  it("identifies trailing region as deleted when keep stops before transcript end", () => {
    const words = [
      makeWord(0, 1_000_000),
      makeWord(1_000_000, 2_000_000),
      makeWord(2_000_000, 3_000_000),
    ];
    const segs = [{ start_us: 0, end_us: 1_000_000 }];
    const ranges = getDeletedRangesFromKeepSegments(words, segs);
    expect(ranges).toHaveLength(1);
    expect(ranges[0].start).toBeCloseTo(1.0, 5);
    expect(ranges[0].end).toBeCloseTo(3.0, 5);
  });
});
