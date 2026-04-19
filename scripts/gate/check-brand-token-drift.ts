#!/usr/bin/env bun
/**
 * Design-token drift gate — detect hex color literals outside the single
 * source of truth (`src/App.css @theme` block).
 *
 * Rationale: `docs/design-tokens.md` + `src/AGENTS.md` forbid hex literals
 * in component code. All brand/theme colors must route through CSS vars
 * (`var(--color-logo-primary)`) or Tailwind utilities (`bg-logo-primary`).
 * Every hex literal outside the `@theme` block is a drift site — it bypasses
 * light/dark theming and cannot be changed from one place.
 *
 * Scope: `src` tree, extensions ts/tsx/css. The token-declaration block in
 * `src/App.css` is the only allowed home for hex literals. `App.css` is
 * processed with a scoped carve-out (inside `@theme ...`).
 *
 * Existing drift that is known and grandfathered lives in `ALLOWLIST`. Do
 * not extend it without a referenced justification; prefer adding a token
 * or migrating the call site.
 *
 * Invocation:
 *   bun scripts/gate/check-brand-token-drift.ts          # report-only
 *   bun scripts/gate/check-brand-token-drift.ts --strict # CI mode, exit 1 on drift
 *
 * Exit codes: 0 clean (or report-only drift), 1 strict-mode drift, 2 internal error.
 */

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";

const ROOT = process.cwd();
const SRC = join(ROOT, "src");

// Hex literals of lengths 3, 4, 6, or 8. The leading `#` must NOT be
// followed by another hex char (prevents eating into longer tokens).
const HEX_REGEX = /#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})\b/g;

// Drift sites that predate this gate and haven't been migrated yet.
// Each entry MUST be justified. Prefer migration over allowlist growth.
const ALLOWLIST = new Set<string>([
  // App.css IS the source of truth for tokens. The `@theme` block is
  // carved out below, but the dark-theme `:root` override and any
  // legacy comments also legitimately contain hex literals.
  "src/App.css",
  // CaptionProfileShared / CaptionMockFrame / CaptionSettings embed
  // colors in profile data structures that serialize to user settings
  // (hex strings are the storage format). Different concern from
  // UI drift — flag separately if ever.
  "src/components/settings/captions/CaptionProfileShared.tsx",
  "src/components/settings/captions/CaptionMockFrame.tsx",
  "src/components/settings/captions/CaptionSettings.tsx",
  // TranscriptEditor carries a SPEAKER_COLORS palette of 8 distinct
  // hues for diarization. Intentional variety, not brand drift.
  "src/components/editor/TranscriptEditor.tsx",
]);

// Neutral hex literals that are NOT brand colors — pure black/white, alpha
// patterns used by shadows/rings. These can legitimately appear outside
// tokens because they describe absolute opacity values, not brand intent.
const NEUTRAL_HEXES = new Set<string>([
  "#000",
  "#000000",
  "#fff",
  "#ffffff",
  "#0f0f0f", // matches token but bare hex fine for SVG defaults
]);

type Violation = {
  file: string;
  line: number;
  hex: string;
  snippet: string;
};

async function walk(dir: string): Promise<string[]> {
  const entries = await readdir(dir, { withFileTypes: true });
  const results: string[] = [];
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...(await walk(full)));
    } else if (entry.isFile() && /\.(tsx?|jsx?|css)$/.test(entry.name)) {
      results.push(full);
    }
  }
  return results;
}

function lineOf(content: string, offset: number): number {
  return content.slice(0, offset).split("\n").length;
}

/**
 * Strip the `@theme { ... }` block from App.css content so hexes
 * declared there (the token declarations themselves) don't count as drift.
 */
function stripThemeBlock(content: string, rel: string): string {
  if (rel !== "src/App.css") return content;
  // Replace every `@theme { ... }` block with equivalent whitespace so
  // line offsets stay stable for any remaining violations.
  return content.replace(/@theme\s*\{[^}]*\}/g, (block) =>
    block.replace(/[^\n]/g, " "),
  );
}

async function main(): Promise<number> {
  const files = await walk(SRC);
  const violations: Violation[] = [];

  for (const file of files) {
    const rel = relative(ROOT, file).replace(/\\/g, "/");
    if (ALLOWLIST.has(rel)) continue;

    const raw = await readFile(file, "utf8");
    const content = stripThemeBlock(raw, rel);
    for (const m of content.matchAll(HEX_REGEX)) {
      const hex = m[0].toLowerCase();
      if (NEUTRAL_HEXES.has(hex)) continue;
      violations.push({
        file: rel,
        line: lineOf(content, m.index ?? 0),
        hex: m[0],
        snippet: content
          .slice(Math.max(0, (m.index ?? 0) - 20), (m.index ?? 0) + 40)
          .replace(/\s+/g, " "),
      });
    }
  }

  if (violations.length === 0) {
    console.log(
      "[design-tokens] OK — no hex color literals outside src/App.css @theme.",
    );
    return 0;
  }

  const strict = process.argv.includes("--strict");
  const verb = strict ? "FAIL" : "WARN";
  const exitCode = strict ? 1 : 0;
  console.error(
    `[design-tokens] ${verb} — ${violations.length} hex color literal(s) found outside tokens:`,
  );
  for (const v of violations) {
    console.error(`  ${v.file}:${v.line}  ${v.hex}   …${v.snippet}…`);
  }
  console.error(
    "\nFix: declare the color in src/App.css @theme and reference it via var(--color-*) " +
      "or a Tailwind utility (bg-logo-primary, text-logo-primary, etc.). " +
      "See docs/design-tokens.md for the full token table.",
  );
  if (!strict) {
    console.error(
      "\n(Report-only mode. Pass --strict to turn drift into a hard CI failure.)",
    );
  }
  return exitCode;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[design-tokens] internal error:", err);
    process.exit(2);
  });
