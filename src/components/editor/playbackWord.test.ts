import { describe, expect, it } from "vitest";
import type { Word } from "@/stores/editorStore";
import { findWordAtTimeUs } from "./playbackWord";

function word(
  text: string,
  start_us: number,
  end_us: number,
  deleted = false,
): Word {
  return {
    text,
    start_us,
    end_us,
    deleted,
    silenced: false,
    confidence: 1,
    speaker_id: -1,
  } as Word;
}

describe("findWordAtTimeUs", () => {
  it("returns the active word when time lands inside its range", () => {
    const words = [
      word("alpha", 0, 200_000),
      word("beta", 200_000, 450_000),
      word("gamma", 450_000, 700_000),
    ];

    expect(findWordAtTimeUs(words, 300_000)).toBe(1);
  });

  it("skips deleted words and falls back to null inside removed ranges", () => {
    const words = [
      word("alpha", 0, 200_000),
      word("beta", 200_000, 450_000, true),
      word("gamma", 450_000, 700_000),
    ];

    expect(findWordAtTimeUs(words, 300_000)).toBeNull();
  });

  it("returns null for gaps between words", () => {
    const words = [word("alpha", 0, 200_000), word("gamma", 450_000, 700_000)];

    expect(findWordAtTimeUs(words, 300_000)).toBeNull();
  });
});
