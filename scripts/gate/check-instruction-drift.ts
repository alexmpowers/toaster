#!/usr/bin/env bun
/**
 * Instruction-drift gate.
 *
 * AGENTS.md is the single source of truth for conventions, commands, and
 * thresholds. Other instruction files (.github/copilot-instructions.md,
 * README.md, docs/build.md, src/AGENTS.md, src-tauri/AGENTS.md) must not
 * contradict it. This gate extracts concrete checkable facts (stack
 * versions, command strings, numeric thresholds, paths) and flags
 * contradictions across files.
 *
 * Invocation:
 *   bun scripts/gate/check-instruction-drift.ts          # report-only
 *   bun scripts/gate/check-instruction-drift.ts --strict # exit 1 on drift
 *
 * Exit codes: 0 clean (or report-only drift), 1 strict-mode drift,
 * 2 internal error.
 */

import { readFile } from "node:fs/promises";
import { join } from "node:path";

const ROOT = process.cwd();
const STRICT = process.argv.includes("--strict");

// ---------------------------------------------------------------------------
// Files to compare
// ---------------------------------------------------------------------------

const CANONICAL = "AGENTS.md";

const MIRROR_FILES = [
  ".github/copilot-instructions.md",
  "README.md",
  "docs/build.md",
  "src/AGENTS.md",
  "src-tauri/AGENTS.md",
];

// ---------------------------------------------------------------------------
// Fact extractors — each returns { label, value, line } tuples from content
// ---------------------------------------------------------------------------

type Fact = { label: string; value: string; line: number };

function extractStackVersions(content: string): Fact[] {
  const facts: Fact[] = [];
  const lines = content.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];

    // Rust version: "Rust 1.82+" or "Rust 1.82"
    const rustMatch = l.match(/\bRust\s+([\d.]+\+?)/i);
    if (rustMatch)
      facts.push({ label: "rust-version", value: rustMatch[1], line: i + 1 });

    // Tauri version
    const tauriMatch = l.match(/\bTauri\s+([\d.]+x?)/i);
    if (tauriMatch)
      facts.push({ label: "tauri-version", value: tauriMatch[1], line: i + 1 });

    // React version
    const reactMatch = l.match(/\bReact\s+(\d+)/i);
    if (reactMatch)
      facts.push({ label: "react-version", value: reactMatch[1], line: i + 1 });

    // TypeScript version
    const tsMatch = l.match(/\bTypeScript\s+(\d+)/i);
    if (tsMatch)
      facts.push({
        label: "typescript-version",
        value: tsMatch[1],
        line: i + 1,
      });

    // Vite version
    const viteMatch = l.match(/\bVite\s+(\d+)/i);
    if (viteMatch)
      facts.push({ label: "vite-version", value: viteMatch[1], line: i + 1 });

    // Tailwind version
    const tailwindMatch = l.match(/\bTailwind\s+(?:CSS\s+)?(\d+)/i);
    if (tailwindMatch)
      facts.push({
        label: "tailwind-version",
        value: tailwindMatch[1],
        line: i + 1,
      });

    // Zustand version
    const zustandMatch = l.match(/\bZustand\s+(\d+)/i);
    if (zustandMatch)
      facts.push({
        label: "zustand-version",
        value: zustandMatch[1],
        line: i + 1,
      });

    // FFmpeg version
    const ffmpegMatch = l.match(/\bFFmpeg\s+(\d+)/i);
    if (ffmpegMatch)
      facts.push({
        label: "ffmpeg-version",
        value: ffmpegMatch[1],
        line: i + 1,
      });
  }
  return facts;
}

function extractNumericThresholds(content: string): Fact[] {
  const facts: Fact[] = [];
  const lines = content.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];

    // 800-line file cap
    if (/\b800[- ]line\b/i.test(l)) {
      facts.push({ label: "file-line-cap", value: "800", line: i + 1 });
    }

    // 20 locale files
    if (/\b(?:all\s+)?20\s+locale/i.test(l)) {
      facts.push({ label: "locale-count", value: "20", line: i + 1 });
    }
  }
  return facts;
}

function extractKeyCommands(content: string): Fact[] {
  const facts: Fact[] = [];
  const lines = content.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];

    // bun install command
    if (/bun install --frozen-lockfile/.test(l)) {
      facts.push({
        label: "bun-install-cmd",
        value: "bun install --frozen-lockfile",
        line: i + 1,
      });
    }

    // cargo tauri dev
    if (/cargo tauri dev/.test(l) && !/npm run tauri/.test(l)) {
      facts.push({
        label: "tauri-dev-cmd",
        value: "cargo tauri dev",
        line: i + 1,
      });
    }
  }
  return facts;
}

function extractKeyPolicies(content: string): Fact[] {
  const facts: Fact[] = [];
  const lines = content.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];

    // bindings.ts is specta-generated
    if (
      /bindings\.ts.*specta[- ]generated|specta[- ]generated.*bindings\.ts/i.test(
        l,
      )
    ) {
      facts.push({
        label: "bindings-generated",
        value: "specta-generated",
        line: i + 1,
      });
    }

    // Microsecond timestamps
    if (
      /\btimestamps?\b.*\bmicroseconds?\b|\bmicroseconds?\b.*\btimestamps?\b/i.test(
        l,
      )
    ) {
      facts.push({
        label: "timestamp-unit",
        value: "microseconds",
        line: i + 1,
      });
    }

    // Local-only inference
    if (/local[- ]only\s+inference/i.test(l)) {
      facts.push({
        label: "inference-policy",
        value: "local-only",
        line: i + 1,
      });
    }
  }
  return facts;
}

const ALL_EXTRACTORS = [
  extractStackVersions,
  extractNumericThresholds,
  extractKeyCommands,
  extractKeyPolicies,
];

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

type FileFactMap = Map<string, Fact[]>;
type Contradiction = {
  label: string;
  canonical: { file: string; line: number; value: string };
  mirror: { file: string; line: number; value: string };
};

async function loadFacts(file: string): Promise<Fact[]> {
  const absPath = join(ROOT, file);
  let content: string;
  try {
    content = await readFile(absPath, "utf8");
  } catch (err) {
    console.error(
      `[instruction-drift] WARN: failed to read ${absPath}: ${(err as Error).message}`,
    );
    return [];
  }
  if (!content.trim()) {
    console.error(
      `[instruction-drift] WARN: ${absPath} is empty (${content.length} chars)`,
    );
  }
  const facts: Fact[] = [];
  for (const extractor of ALL_EXTRACTORS) {
    facts.push(...extractor(content));
  }
  return facts;
}

async function main(): Promise<number> {
  // Load canonical facts
  const canonicalFacts = await loadFacts(CANONICAL);
  if (canonicalFacts.length === 0) {
    console.error(
      `[instruction-drift] ERROR: No facts extracted from ${CANONICAL}`,
    );
    return 2;
  }

  // Build canonical map: label → first occurrence
  const canonicalMap = new Map<string, Fact>();
  for (const fact of canonicalFacts) {
    if (!canonicalMap.has(fact.label)) {
      canonicalMap.set(fact.label, fact);
    }
  }

  // Load mirror file facts
  const mirrorFactMap: FileFactMap = new Map();
  for (const file of MIRROR_FILES) {
    const facts = await loadFacts(file);
    if (facts.length > 0) {
      mirrorFactMap.set(file, facts);
    }
  }

  // Find contradictions
  const contradictions: Contradiction[] = [];
  for (const [file, facts] of mirrorFactMap) {
    for (const fact of facts) {
      const canonical = canonicalMap.get(fact.label);
      if (!canonical) continue; // no canonical claim to contradict
      if (canonical.value !== fact.value) {
        contradictions.push({
          label: fact.label,
          canonical: {
            file: CANONICAL,
            line: canonical.line,
            value: canonical.value,
          },
          mirror: { file, line: fact.line, value: fact.value },
        });
      }
    }
  }

  // Also check for internal contradictions within AGENTS.md (same label, different value)
  const canonicalByLabel = new Map<string, Fact[]>();
  for (const fact of canonicalFacts) {
    const existing = canonicalByLabel.get(fact.label) ?? [];
    existing.push(fact);
    canonicalByLabel.set(fact.label, existing);
  }
  for (const [label, facts] of canonicalByLabel) {
    const distinct = new Set(facts.map((f) => f.value));
    if (distinct.size > 1) {
      const sorted = facts.sort((a, b) => a.line - b.line);
      for (let i = 1; i < sorted.length; i++) {
        if (sorted[i].value !== sorted[0].value) {
          contradictions.push({
            label,
            canonical: {
              file: CANONICAL,
              line: sorted[0].line,
              value: sorted[0].value,
            },
            mirror: {
              file: CANONICAL,
              line: sorted[i].line,
              value: sorted[i].value,
            },
          });
        }
      }
    }
  }

  // Report
  if (contradictions.length === 0) {
    const mirrorCount = mirrorFactMap.size;
    const factCount = canonicalFacts.length;
    console.log(
      `[instruction-drift] OK - ${factCount} canonical facts checked across ${mirrorCount} mirror files. No contradictions.`,
    );
    return 0;
  }

  const level = STRICT ? "FAIL" : "WARN";
  const exitCode = STRICT ? 1 : 0;

  console.error(
    `[instruction-drift] ${level} - ${contradictions.length} contradiction(s) found:`,
  );
  for (const c of contradictions) {
    console.error(`\n  ${c.label}:`);
    console.error(
      `    ${c.canonical.file}:${c.canonical.line} says "${c.canonical.value}"`,
    );
    console.error(
      `    ${c.mirror.file}:${c.mirror.line} says "${c.mirror.value}"`,
    );
    console.error(`    Canonical (AGENTS.md) should win.`);
  }

  if (!STRICT) {
    console.error(
      "\n(Report-only mode. Pass --strict to fail on instruction drift.)",
    );
  }

  return exitCode;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[instruction-drift] internal error:", err);
    process.exit(2);
  });
