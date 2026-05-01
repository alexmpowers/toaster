#!/usr/bin/env bun
/**
 * Backend-authority drift gate.
 *
 * Toaster's architecture requires keep-segment/time-mapping/caption-layout
 * authority to live in backend managers, with frontend code limited to
 * consuming backend outputs.
 *
 * This gate scans frontend source files and reports suspicious authority
 * keywords outside a small allowlist of approved projection modules.
 *
 * Invocation:
 *   bun scripts/gate/check-backend-authority-drift.ts          # report-only
 *   bun scripts/gate/check-backend-authority-drift.ts --strict # exit 1 on drift
 */

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";

const ROOT = process.cwd();
const SRC = join(ROOT, "src");
const STRICT = process.argv.includes("--strict");

const EXCLUDED_EXACT = new Set<string>(["src/bindings.ts"]);

const ALLOWED_PATTERNS: RegExp[] = [
  /^src\/lib\/utils\/timeline\.ts$/,
  /^src\/components\/player\/MediaPlayer\.tsx$/,
  /^src\/components\/player\/useTimingContract\.ts$/,
  /^src\/stores\/editorStore\.ts$/,
  /^src\/components\/editor\/EditorView\.tsx$/,
  /^src\/components\/editor\/TranscriptEditor\.tsx$/,
];

const SUSPICIOUS_PATTERNS: Array<{ label: string; regex: RegExp }> = [
  {
    label: "keep-segment terminology",
    regex: /\bkeep[_-]?segments?\b/gi,
  },
  {
    label: "time-mapping terminology",
    regex: /\btime[_-]?mapping\b|\btime[_-]?map\b|\bmapTime\b/gi,
  },
  {
    label: "caption-layout derivation identifier",
    regex:
      /\b(?:compute|derive|build|get)[A-Za-z0-9_]*CaptionLayout\b|\bcaption[_-]?layout[_-]?(?:calc|compute|derive|build|get)\b/gi,
  },
  {
    label: "word-grouping terminology",
    regex: /\bgroupWords\b|\bword[_-]?group(?:ing)?\b/gi,
  },
  {
    label: "authoritative keep-segment identifier",
    regex: /\bcanonical_keep_segments_for_media\b/gi,
  },
];

type Hit = {
  file: string;
  line: number;
  label: string;
  snippet: string;
};

async function walk(dir: string): Promise<string[]> {
  const entries = await readdir(dir, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name.startsWith(".")) {
        continue;
      }
      files.push(...(await walk(full)));
      continue;
    }
    if (!entry.isFile()) continue;
    if (!/\.(ts|tsx)$/.test(entry.name)) continue;
    if (entry.name.endsWith(".test.ts") || entry.name.endsWith(".test.tsx")) {
      continue;
    }
    files.push(full);
  }
  return files;
}

function toRel(abs: string): string {
  return relative(ROOT, abs).replace(/\\/g, "/");
}

function isAllowedFile(rel: string): boolean {
  if (EXCLUDED_EXACT.has(rel)) return true;
  return ALLOWED_PATTERNS.some((pattern) => pattern.test(rel));
}

function lineOf(content: string, offset: number): number {
  return content.slice(0, offset).split("\n").length;
}

async function main(): Promise<number> {
  const files = await walk(SRC);
  const hits: Hit[] = [];

  for (const file of files) {
    const rel = toRel(file);
    if (isAllowedFile(rel)) continue;

    const content = await readFile(file, "utf8");
    for (const pattern of SUSPICIOUS_PATTERNS) {
      for (const match of content.matchAll(pattern.regex)) {
        const idx = match.index ?? 0;
        hits.push({
          file: rel,
          line: lineOf(content, idx),
          label: pattern.label,
          snippet: content
            .slice(Math.max(0, idx - 20), idx + 60)
            .replace(/\s+/g, " "),
        });
      }
    }
  }

  if (hits.length === 0) {
    console.log(
      "[backend-authority] OK - no suspicious backend-authority keywords outside approved projection modules.",
    );
    return 0;
  }

  const level = STRICT ? "FAIL" : "WARN";
  const exitCode = STRICT ? 1 : 0;

  console.error(
    `[backend-authority] ${level} - ${hits.length} suspicious match(es) outside approved files:`,
  );
  for (const hit of hits) {
    console.error(
      `  ${hit.file}:${hit.line} [${hit.label}] ...${hit.snippet}...`,
    );
  }
  console.error(
    "\nIf this is legitimate frontend projection, add a narrowly-scoped allowlist entry with justification.",
  );
  console.error(
    "If this is duplicate authority logic, move computation to src-tauri/src/managers and consume via bindings.",
  );

  if (!STRICT) {
    console.error(
      "\n(Report-only mode. Pass --strict to fail on backend-authority drift.)",
    );
  }

  return exitCode;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[backend-authority] internal error:", err);
    process.exit(2);
  });
