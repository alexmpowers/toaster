#!/usr/bin/env bun
/**
 * Transcription adapter schema validator.
 *
 * Validates NormalizedTranscriptionResult fixture JSONs against the
 * canonical contract defined in transcription-adapter-contract/SKILL.md.
 *
 * Invariants checked:
 *   1. Monotonic non-overlap: words[i].end_us <= words[i+1].start_us
 *   2. No zero-duration: end_us > start_us for every word
 *   3. Non-speech tokens stripped: no [MUSIC], <silence>, etc.
 *   4. No equal-duration synthesis: adjacent words must not all have
 *      identical durations (within 1µs) unless authoritative=true
 *   5. Sample-rate truth: input_sample_rate_hz > 0
 *   6. Language reported: language is a non-empty string
 *
 * Invocation:
 *   bun scripts/gate/check-adapter-schema.ts          # report-only
 *   bun scripts/gate/check-adapter-schema.ts --strict # exit 1 on violation
 *
 * Exit codes: 0 clean, 1 strict-mode violation, 2 internal error.
 */

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";

const ROOT = process.cwd();
const STRICT = process.argv.includes("--strict");
const FIXTURES_ROOT = join(
  ROOT,
  "src-tauri",
  "tests",
  "fixtures",
  "parity",
  "backend_outputs",
);

// Non-speech tokens that must not appear in word text
const NON_SPEECH_PATTERNS = [
  /^\[.*\]$/, // [MUSIC], [LAUGHTER], etc.
  /^<.*>$/, // <silence>, <eot>, etc.
  /^\(.*\)$/, // (inaudible), (laughter), etc.
  /^[♪♫]+$/, // music symbols
  /^\.{3,}$/, // bare ellipsis
];

type Violation = {
  file: string;
  invariant: string;
  detail: string;
};

interface Word {
  text: string;
  start_us: number;
  end_us: number;
  confidence: number | null;
}

interface TranscriptionResult {
  words: Word[];
  language: string;
  word_timestamps_authoritative: boolean;
  input_sample_rate_hz: number;
}

function validateResult(
  file: string,
  result: TranscriptionResult,
): Violation[] {
  const violations: Violation[] = [];
  const { words } = result;

  // Invariant 1: Monotonic non-overlap
  for (let i = 0; i < words.length - 1; i++) {
    if (words[i].end_us > words[i + 1].start_us) {
      violations.push({
        file,
        invariant: "monotonic-non-overlap",
        detail: `words[${i}].end_us (${words[i].end_us}) > words[${i + 1}].start_us (${words[i + 1].start_us})`,
      });
    }
  }

  // Invariant 2: No zero-duration
  for (let i = 0; i < words.length; i++) {
    if (words[i].end_us <= words[i].start_us) {
      violations.push({
        file,
        invariant: "no-zero-duration",
        detail: `words[${i}] "${words[i].text}": end_us (${words[i].end_us}) <= start_us (${words[i].start_us})`,
      });
    }
  }

  // Invariant 3: Non-speech tokens stripped
  for (let i = 0; i < words.length; i++) {
    const text = words[i].text.trim();
    for (const pattern of NON_SPEECH_PATTERNS) {
      if (pattern.test(text)) {
        violations.push({
          file,
          invariant: "non-speech-stripped",
          detail: `words[${i}].text = "${text}" matches non-speech pattern ${pattern}`,
        });
        break;
      }
    }
  }

  // Invariant 4: No equal-duration synthesis (only if non-authoritative)
  if (!result.word_timestamps_authoritative && words.length >= 3) {
    const durations = words.map((w) => w.end_us - w.start_us);
    const allEqual = durations.every((d) => Math.abs(d - durations[0]) <= 1);
    if (allEqual) {
      violations.push({
        file,
        invariant: "no-equal-duration-synthesis",
        detail: `All ${words.length} words have identical duration ${durations[0]}µs — likely synthesized`,
      });
    }
  }

  // Invariant 5: Sample-rate truth
  if (!result.input_sample_rate_hz || result.input_sample_rate_hz <= 0) {
    violations.push({
      file,
      invariant: "sample-rate-truth",
      detail: `input_sample_rate_hz = ${result.input_sample_rate_hz}`,
    });
  }

  // Invariant 6: Language reported
  if (!result.language || result.language.trim() === "") {
    violations.push({
      file,
      invariant: "language-reported",
      detail: `language is empty or missing`,
    });
  }

  return violations;
}

async function main(): Promise<number> {
  let backends: string[];
  try {
    const entries = await readdir(FIXTURES_ROOT, { withFileTypes: true });
    backends = entries.filter((e) => e.isDirectory()).map((e) => e.name);
  } catch {
    console.log(
      `[adapter-schema] SKIP - fixtures directory not found: ${relative(ROOT, FIXTURES_ROOT)}`,
    );
    return 0;
  }

  if (backends.length === 0) {
    console.log("[adapter-schema] SKIP - no backend output directories found.");
    return 0;
  }

  const allViolations: Violation[] = [];
  let filesChecked = 0;

  for (const backend of backends) {
    const backendDir = join(FIXTURES_ROOT, backend);
    let files: string[];
    try {
      const entries = await readdir(backendDir);
      files = entries.filter((f) => f.endsWith(".result.json"));
    } catch {
      continue;
    }

    for (const file of files) {
      const absPath = join(backendDir, file);
      const relPath = relative(ROOT, absPath).replace(/\\/g, "/");
      let content: string;
      try {
        content = await readFile(absPath, "utf8");
      } catch {
        continue;
      }

      let result: TranscriptionResult;
      try {
        result = JSON.parse(content);
      } catch {
        allViolations.push({
          file: relPath,
          invariant: "valid-json",
          detail: "File is not valid JSON",
        });
        continue;
      }

      if (!result.words || !Array.isArray(result.words)) {
        allViolations.push({
          file: relPath,
          invariant: "has-words-array",
          detail: "Missing or invalid 'words' array",
        });
        continue;
      }

      filesChecked++;
      allViolations.push(...validateResult(relPath, result));
    }
  }

  if (allViolations.length === 0) {
    console.log(
      `[adapter-schema] OK - ${filesChecked} fixture(s) across ${backends.length} backend(s) pass all 6 invariants.`,
    );
    return 0;
  }

  const level = STRICT ? "FAIL" : "WARN";
  const exitCode = STRICT ? 1 : 0;

  console.error(
    `[adapter-schema] ${level} - ${allViolations.length} violation(s) in ${filesChecked} fixture(s):`,
  );
  for (const v of allViolations) {
    console.error(`  ${v.file} [${v.invariant}] ${v.detail}`);
  }
  console.error(
    "\nSee .github/skills/transcription-adapter-contract/SKILL.md for invariant definitions.",
  );

  if (!STRICT) {
    console.error(
      "\n(Report-only mode. Pass --strict to fail on schema violations.)",
    );
  }

  return exitCode;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[adapter-schema] internal error:", err);
    process.exit(2);
  });
