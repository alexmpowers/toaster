/**
 * diagnose-fixture-diversity.ts
 *
 * Analyses .toaster project files and reports word-timing statistics,
 * gap distributions, and simulated keep-segment fragmentation at
 * different MAX_INTRA_SEGMENT_GAP_US thresholds.
 *
 * Usage:
 *   bun scripts/eval/diagnose-fixture-diversity.ts <path-to-.toaster>
 *   bun scripts/eval/diagnose-fixture-diversity.ts --all   # all .toaster files under eval/fixtures/
 */

import { readFileSync, readdirSync, existsSync } from "fs";
import { join, basename, resolve } from "path";

interface Word {
  text: string;
  start_us: number;
  end_us: number;
  deleted: boolean;
  silenced: boolean;
  confidence: number;
  speaker_id: number;
}

interface ToasterProject {
  version: string;
  name: string;
  words: Word[];
  settings: {
    filler_words: string[];
    pause_threshold_us: number;
    export_format: string;
  };
}

interface GapStats {
  count: number;
  min_us: number;
  max_us: number;
  mean_us: number;
  median_us: number;
  p75_us: number;
  p90_us: number;
  p95_us: number;
  stddev_us: number;
  histogram: Record<string, number>;
}

interface SegmentReport {
  threshold_us: number;
  threshold_label: string;
  segment_count: number;
  min_duration_us: number;
  max_duration_us: number;
  mean_duration_us: number;
  micro_segments: number; // segments < 150ms
  total_kept_us: number;
}

function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const idx = (p / 100) * (sorted.length - 1);
  const lo = Math.floor(idx);
  const hi = Math.ceil(idx);
  if (lo === hi) return sorted[lo];
  return sorted[lo] + (sorted[hi] - sorted[lo]) * (idx - lo);
}

function computeGapStats(gaps: number[]): GapStats {
  if (gaps.length === 0) {
    return {
      count: 0,
      min_us: 0,
      max_us: 0,
      mean_us: 0,
      median_us: 0,
      p75_us: 0,
      p90_us: 0,
      p95_us: 0,
      stddev_us: 0,
      histogram: {},
    };
  }

  const sorted = [...gaps].sort((a, b) => a - b);
  const mean = gaps.reduce((a, b) => a + b, 0) / gaps.length;
  const variance =
    gaps.reduce((sum, g) => sum + (g - mean) ** 2, 0) / gaps.length;

  // Histogram buckets (ms)
  const buckets = [0, 50, 100, 200, 300, 500, 1000, 2000, 5000, Infinity];
  const histogram: Record<string, number> = {};
  for (let i = 0; i < buckets.length - 1; i++) {
    const label =
      buckets[i + 1] === Infinity
        ? `${buckets[i]}ms+`
        : `${buckets[i]}-${buckets[i + 1]}ms`;
    histogram[label] = gaps.filter(
      (g) => g / 1000 >= buckets[i] && g / 1000 < buckets[i + 1]
    ).length;
  }

  return {
    count: gaps.length,
    min_us: sorted[0],
    max_us: sorted[sorted.length - 1],
    mean_us: Math.round(mean),
    median_us: Math.round(percentile(sorted, 50)),
    p75_us: Math.round(percentile(sorted, 75)),
    p90_us: Math.round(percentile(sorted, 90)),
    p95_us: Math.round(percentile(sorted, 95)),
    stddev_us: Math.round(Math.sqrt(variance)),
    histogram,
  };
}

function simulateKeepSegments(
  words: Word[],
  thresholdUs: number
): SegmentReport {
  const MIN_KEEP_SEGMENT_US = 150_000;
  const kept = words.filter((w) => !w.deleted);

  if (kept.length === 0) {
    return {
      threshold_us: thresholdUs,
      threshold_label: `${thresholdUs / 1000}ms`,
      segment_count: 0,
      min_duration_us: 0,
      max_duration_us: 0,
      mean_duration_us: 0,
      micro_segments: 0,
      total_kept_us: 0,
    };
  }

  // Build segments using same logic as editor/mod.rs get_keep_segments
  // (simplified — no overlap/forbidden logic, just gap-based splitting)
  const segments: [number, number][] = [];
  let segStart = kept[0].start_us;
  let segEnd = kept[0].end_us;

  for (let i = 1; i < kept.length; i++) {
    const gap = kept[i].start_us - segEnd;

    // Check if any deleted word sits between prev kept word and this one
    const deletedBetween = words.some(
      (w) => w.deleted && w.start_us >= segEnd && w.end_us <= kept[i].start_us
    );

    const splitRequired = deletedBetween || gap > thresholdUs;
    if (splitRequired) {
      if (segEnd > segStart) {
        segments.push([segStart, segEnd]);
      }
      segStart = kept[i].start_us;
    }
    segEnd = Math.max(segEnd, kept[i].end_us);
  }
  if (segEnd > segStart) {
    segments.push([segStart, segEnd]);
  }

  const durations = segments.map(([s, e]) => e - s);
  const total = durations.reduce((a, b) => a + b, 0);

  return {
    threshold_us: thresholdUs,
    threshold_label: `${thresholdUs / 1000}ms`,
    segment_count: segments.length,
    min_duration_us: Math.min(...durations),
    max_duration_us: Math.max(...durations),
    mean_duration_us: Math.round(total / durations.length),
    micro_segments: durations.filter((d) => d < MIN_KEEP_SEGMENT_US).length,
    total_kept_us: total,
  };
}

function analyzeProject(path: string): void {
  const raw = readFileSync(path, "utf-8");
  const project: ToasterProject = JSON.parse(raw);

  const words = project.words;
  const kept = words.filter((w) => !w.deleted);
  const deleted = words.filter((w) => w.deleted);

  const totalDurationUs =
    words.length > 0 ? words[words.length - 1].end_us - words[0].start_us : 0;

  console.log(`\n${"=".repeat(72)}`);
  console.log(`FIXTURE: ${project.name}`);
  console.log(`File:    ${path}`);
  console.log(`${"=".repeat(72)}`);

  // Basic word stats
  console.log(`\n--- Word Statistics ---`);
  console.log(`  Total words:    ${words.length}`);
  console.log(`  Kept words:     ${kept.length}`);
  console.log(`  Deleted words:  ${deleted.length}`);
  console.log(
    `  Total span:     ${(totalDurationUs / 1_000_000).toFixed(2)}s`
  );
  console.log(
    `  Word density:   ${(kept.length / (totalDurationUs / 1_000_000)).toFixed(1)} words/sec`
  );

  // Word duration stats
  const wordDurations = kept.map((w) => w.end_us - w.start_us);
  const sortedDurations = [...wordDurations].sort((a, b) => a - b);
  console.log(`\n--- Word Duration Distribution ---`);
  console.log(
    `  Min:    ${(sortedDurations[0] / 1000).toFixed(1)}ms`
  );
  console.log(
    `  Median: ${(percentile(sortedDurations, 50) / 1000).toFixed(1)}ms`
  );
  console.log(
    `  Mean:   ${(wordDurations.reduce((a, b) => a + b, 0) / wordDurations.length / 1000).toFixed(1)}ms`
  );
  console.log(
    `  Max:    ${(sortedDurations[sortedDurations.length - 1] / 1000).toFixed(1)}ms`
  );

  // Gap analysis (between consecutive kept words)
  const gaps: number[] = [];
  for (let i = 1; i < kept.length; i++) {
    const gap = kept[i].start_us - kept[i - 1].end_us;
    if (gap > 0) gaps.push(gap);
  }

  const gapStats = computeGapStats(gaps);
  console.log(`\n--- Inter-Word Gap Distribution (kept words only) ---`);
  console.log(`  Count:  ${gapStats.count} gaps`);
  console.log(`  Min:    ${(gapStats.min_us / 1000).toFixed(1)}ms`);
  console.log(`  Median: ${(gapStats.median_us / 1000).toFixed(1)}ms`);
  console.log(`  Mean:   ${(gapStats.mean_us / 1000).toFixed(1)}ms`);
  console.log(`  P75:    ${(gapStats.p75_us / 1000).toFixed(1)}ms`);
  console.log(`  P90:    ${(gapStats.p90_us / 1000).toFixed(1)}ms`);
  console.log(`  P95:    ${(gapStats.p95_us / 1000).toFixed(1)}ms`);
  console.log(`  Max:    ${(gapStats.max_us / 1000).toFixed(1)}ms`);
  console.log(`  StdDev: ${(gapStats.stddev_us / 1000).toFixed(1)}ms`);

  console.log(`\n  Gap histogram:`);
  for (const [bucket, count] of Object.entries(gapStats.histogram)) {
    if (count > 0) {
      const bar = "█".repeat(Math.min(count, 40));
      console.log(`    ${bucket.padEnd(14)} ${String(count).padStart(3)} ${bar}`);
    }
  }

  // Gaps > 200ms (the current threshold)
  const gapsOver200 = gaps.filter((g) => g > 200_000);
  console.log(
    `\n  Gaps > 200ms (current threshold): ${gapsOver200.length} / ${gaps.length} (${((gapsOver200.length / Math.max(gaps.length, 1)) * 100).toFixed(1)}%)`
  );

  // Simulate keep-segments at different thresholds
  const thresholds = [
    100_000, 200_000, 300_000, 500_000, 750_000, 1_000_000, 2_000_000,
  ];
  console.log(`\n--- Keep-Segment Simulation (varying MAX_INTRA_SEGMENT_GAP_US) ---`);
  console.log(
    `  ${"Threshold".padEnd(12)} ${"Segments".padStart(8)} ${"MicroSegs".padStart(9)} ${"MinDur".padStart(10)} ${"MaxDur".padStart(10)} ${"MeanDur".padStart(10)} ${"TotalKept".padStart(12)}`
  );
  console.log(`  ${"-".repeat(71)}`);

  for (const t of thresholds) {
    const report = simulateKeepSegments(words, t);
    const marker = t === 200_000 ? " ← CURRENT" : "";
    console.log(
      `  ${report.threshold_label.padEnd(12)} ${String(report.segment_count).padStart(8)} ${String(report.micro_segments).padStart(9)} ${(report.min_duration_us / 1000).toFixed(0).padStart(8)}ms ${(report.max_duration_us / 1000).toFixed(0).padStart(8)}ms ${(report.mean_duration_us / 1000).toFixed(0).padStart(8)}ms ${(report.total_kept_us / 1_000_000).toFixed(2).padStart(10)}s${marker}`
    );
  }

  // Suggest adaptive threshold
  const suggestedThreshold = Math.round(
    gapStats.median_us + 2 * gapStats.stddev_us
  );
  const suggested = simulateKeepSegments(words, suggestedThreshold);
  console.log(
    `\n  Suggested adaptive (median + 2σ = ${(suggestedThreshold / 1000).toFixed(0)}ms):`
  );
  console.log(
    `    → ${suggested.segment_count} segments, ${suggested.micro_segments} micro-segments`
  );

  // Settings info
  console.log(`\n--- Project Settings ---`);
  console.log(
    `  pause_threshold_us: ${project.settings.pause_threshold_us} (${(project.settings.pause_threshold_us / 1_000_000).toFixed(1)}s)`
  );
  console.log(
    `  filler_words:       [${project.settings.filler_words.join(", ")}]`
  );
}

// --- Main ---
const args = process.argv.slice(2);
const repoRoot = resolve(join(import.meta.dir, "..", ".."));

if (args.length === 0 || args[0] === "--help") {
  console.log(`Usage:
  bun scripts/eval/diagnose-fixture-diversity.ts <path-to-.toaster>
  bun scripts/eval/diagnose-fixture-diversity.ts --all`);
  process.exit(0);
}

if (args[0] === "--all") {
  const fixtureDir = join(repoRoot, "eval", "fixtures");
  const toasterFiles = readdirSync(fixtureDir).filter((f) =>
    f.endsWith(".toaster")
  );
  if (toasterFiles.length === 0) {
    console.log("No .toaster files found in eval/fixtures/");
    process.exit(1);
  }
  for (const f of toasterFiles) {
    analyzeProject(join(fixtureDir, f));
  }
} else {
  const path = resolve(args[0]);
  if (!existsSync(path)) {
    console.error(`File not found: ${path}`);
    process.exit(1);
  }
  analyzeProject(path);
}

console.log(`\n${"=".repeat(72)}`);
console.log("Done.");
