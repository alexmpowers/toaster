import { type Word } from "@/stores/editorStore";

/**
 * Merge overlapping or adjacent (touching) microsecond ranges drawn from
 * `words` for which `predicate(word)` returns true. Returns a sorted array
 * of `[start_us, end_us]` tuples with no overlaps.
 *
 * Used by the waveform overlay to paint each pixel of a deleted/silenced
 * region exactly once — a sentinel that overlaps a real word would
 * otherwise stack two translucent layers into a darker stripe.
 */
export function mergeRangesUs(
  words: readonly Word[],
  predicate: (w: Word) => boolean,
): Array<[number, number]> {
  const ranges: Array<[number, number]> = [];
  for (const w of words) {
    if (!predicate(w)) continue;
    if (w.end_us <= w.start_us) continue;
    ranges.push([w.start_us, w.end_us]);
  }
  if (ranges.length === 0) return ranges;
  ranges.sort((a, b) => a[0] - b[0] || a[1] - b[1]);

  const merged: Array<[number, number]> = [];
  let [curStart, curEnd] = ranges[0];
  for (let i = 1; i < ranges.length; i++) {
    const [s, e] = ranges[i];
    if (s <= curEnd) {
      if (e > curEnd) curEnd = e;
    } else {
      merged.push([curStart, curEnd]);
      curStart = s;
      curEnd = e;
    }
  }
  merged.push([curStart, curEnd]);
  return merged;
}

/**
 * Subtract `forbidden` ranges from `base` ranges, returning the surviving
 * sub-intervals in sorted order. Both inputs MUST already be sorted and
 * non-overlapping (e.g. produced by `mergeRangesUs`).
 *
 * Used so the waveform's silenced (yellow) overlay does not paint under a
 * deleted (red) overlay — keeping the cut a single solid swatch.
 */
export function subtractRangesUs(
  base: ReadonlyArray<readonly [number, number]>,
  forbidden: ReadonlyArray<readonly [number, number]>,
): Array<[number, number]> {
  if (base.length === 0) return [];
  if (forbidden.length === 0) return base.map(([s, e]) => [s, e]);

  const out: Array<[number, number]> = [];
  for (const [bStart, bEnd] of base) {
    let cursor = bStart;
    for (const [fStart, fEnd] of forbidden) {
      if (fEnd <= cursor) continue;
      if (fStart >= bEnd) break;
      if (fStart > cursor) out.push([cursor, Math.min(fStart, bEnd)]);
      cursor = Math.max(cursor, fEnd);
      if (cursor >= bEnd) break;
    }
    if (cursor < bEnd) out.push([cursor, bEnd]);
  }
  return out;
}
