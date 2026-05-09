import type { Word } from "@/stores/editorStore";

const SEARCH_WINDOW = 5;

export function findWordAtTimeUs(
  words: Word[],
  timeUs: number,
): number | null {
  if (words.length === 0 || timeUs < 0) {
    return null;
  }

  let lo = 0;
  let hi = words.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >>> 1;
    if (words[mid].start_us <= timeUs) {
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }

  for (let index = hi; index >= Math.max(0, hi - SEARCH_WINDOW); index--) {
    const word = words[index];
    if (!word.deleted && timeUs >= word.start_us && timeUs < word.end_us) {
      return index;
    }
  }

  return null;
}
