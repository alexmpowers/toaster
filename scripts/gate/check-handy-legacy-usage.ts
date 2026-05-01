#!/usr/bin/env bun
/**
 * Handy-legacy usage gate.
 *
 * Toaster was forked from Handy (a dictation app). Known-dead Handy-era
 * symbols, settings fields, commands, and i18n key prefixes must not
 * receive new code. This gate scans source files for references to
 * catalogued dead symbols and fails if any appear outside their original
 * declaration sites.
 *
 * The catalogue is derived from .github/skills/handy-legacy-pruning/SKILL.md.
 *
 * Invocation:
 *   bun scripts/gate/check-handy-legacy-usage.ts          # report-only
 *   bun scripts/gate/check-handy-legacy-usage.ts --strict # exit 1 on usage
 *
 * Exit codes: 0 clean (or report-only), 1 strict-mode hit, 2 internal error.
 */

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";

const ROOT = process.cwd();
const STRICT = process.argv.includes("--strict");

// ---------------------------------------------------------------------------
// Dead symbol catalogue — derived from handy-legacy-pruning/SKILL.md
// ---------------------------------------------------------------------------

/** Dead Rust symbols: commands, types, functions that only serve dictation */
const DEAD_RUST_SYMBOLS: Array<{ label: string; regex: RegExp }> = [
  {
    label: "dead command: open_recordings_folder",
    regex: /\bopen_recordings_folder\b/g,
  },
  {
    label: "dead type: ShortcutBinding (dictation shortcut)",
    regex: /\bShortcutBinding\b/g,
  },
  {
    label: "dead field: start_hidden (dictation tray mode)",
    regex: /\bstart_hidden\b/g,
  },
  {
    label: "dead default: default_start_hidden",
    regex: /\bdefault_start_hidden\b/g,
  },
  {
    label: "dead default: default_bindings",
    regex: /\bdefault_bindings\b/g,
  },
];

/** Dead i18n key prefixes — dictation-only UI groups */
const DEAD_I18N_PREFIXES: string[] = [
  "tray.",
  // settings.sound.outputDevice.* is LIVE (editor output device selector)
  "settings.sound.soundTheme",
  "settings.sound.muteWhileRecording",
  "settings.advanced.autoSubmit",
  "settings.advanced.pasteMethod",
  "settings.advanced.typingTool",
  "settings.advanced.clipboardHandling",
  "settings.advanced.startHidden",
  "settings.advanced.autostart",
  "settings.advanced.showTrayIcon",
  "settings.advanced.overlay",
  "settings.debug.soundTheme",
  "settings.debug.muteWhileRecording",
  "settings.debug.appendTrailingSpace",
  "settings.debug.pasteDelay",
  "settings.debug.recordingBuffer",
  "settings.debug.alwaysOnMicrophone",
  "settings.debug.keyboardImplementation",
];

// ---------------------------------------------------------------------------
// Allowlists — files where dead symbols exist by definition (declaration)
// ---------------------------------------------------------------------------

const RUST_DECLARATION_ALLOWLIST = new Set<string>([
  "src-tauri/src/commands/mod.rs", // open_recordings_folder definition
  "src-tauri/src/settings/types.rs", // ShortcutBinding, start_hidden definitions
  "src-tauri/src/settings/defaults.rs", // default_start_hidden, default_bindings definitions
  "src-tauri/src/lib.rs", // command registration + start_hidden usage
  "src-tauri/src/cli.rs", // CLI arg for start_hidden
  "src/bindings.ts", // specta-generated, reflects backend types
  "tests/app.spec.ts", // test fixture with start_hidden
]);

const I18N_DECLARATION_ALLOWLIST = new Set<string>([
  // locale files themselves: the dead keys live there until pruned
]);

// ---------------------------------------------------------------------------
// Scan logic
// ---------------------------------------------------------------------------

type Hit = {
  file: string;
  line: number;
  label: string;
  snippet: string;
};

async function walkDir(dir: string, extensions: string[]): Promise<string[]> {
  const entries = await readdir(dir, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (
        entry.name === "node_modules" ||
        entry.name === "target" ||
        entry.name === ".git" ||
        entry.name.startsWith(".")
      ) {
        continue;
      }
      files.push(...(await walkDir(full, extensions)));
      continue;
    }
    if (!entry.isFile()) continue;
    if (!extensions.some((ext) => entry.name.endsWith(ext))) continue;
    files.push(full);
  }
  return files;
}

function toRel(abs: string): string {
  return relative(ROOT, abs).replace(/\\/g, "/");
}

function lineOf(content: string, offset: number): number {
  return content.slice(0, offset).split("\n").length;
}

async function scanRustAndTs(): Promise<Hit[]> {
  const hits: Hit[] = [];
  const dirs = [join(ROOT, "src-tauri", "src"), join(ROOT, "src")];
  const extensions = [".rs", ".ts", ".tsx"];

  for (const dir of dirs) {
    let files: string[];
    try {
      files = await walkDir(dir, extensions);
    } catch {
      continue;
    }

    for (const file of files) {
      const rel = toRel(file);
      if (RUST_DECLARATION_ALLOWLIST.has(rel)) continue;

      const content = await readFile(file, "utf8");
      for (const sym of DEAD_RUST_SYMBOLS) {
        for (const match of content.matchAll(sym.regex)) {
          const idx = match.index ?? 0;
          // Skip if in a comment
          const lineStart = content.lastIndexOf("\n", idx) + 1;
          const lineText = content.slice(lineStart, content.indexOf("\n", idx));
          if (/^\s*\/\//.test(lineText) || /^\s*\*/.test(lineText)) continue;

          hits.push({
            file: rel,
            line: lineOf(content, idx),
            label: sym.label,
            snippet: content
              .slice(Math.max(0, idx - 15), idx + 50)
              .replace(/\s+/g, " ")
              .trim(),
          });
        }
      }
    }
  }
  return hits;
}

async function scanI18nKeys(): Promise<Hit[]> {
  const hits: Hit[] = [];
  const i18nDir = join(ROOT, "src", "i18n", "locales");

  let localeDirs: string[];
  try {
    const entries = await readdir(i18nDir, { withFileTypes: true });
    localeDirs = entries
      .filter((e) => e.isDirectory())
      .map((e) => join(i18nDir, e.name, "translation.json"));
  } catch {
    return hits;
  }

  // Only check the English file for new key additions — if a dead prefix
  // key is in en, it's in all 20 locales (i18n parity gate ensures that).
  const enFile = join(i18nDir, "en", "translation.json");
  let content: string;
  try {
    content = await readFile(enFile, "utf8");
  } catch {
    return hits;
  }

  // Flatten JSON keys and check against dead prefixes
  let json: Record<string, unknown>;
  try {
    json = JSON.parse(content);
  } catch {
    return hits;
  }

  function flattenKeys(obj: Record<string, unknown>, prefix: string): string[] {
    const keys: string[] = [];
    for (const [k, v] of Object.entries(obj)) {
      const full = prefix ? `${prefix}.${k}` : k;
      if (typeof v === "object" && v !== null && !Array.isArray(v)) {
        keys.push(...flattenKeys(v as Record<string, unknown>, full));
      } else {
        keys.push(full);
      }
    }
    return keys;
  }

  const allKeys = flattenKeys(json, "");
  for (const key of allKeys) {
    for (const deadPrefix of DEAD_I18N_PREFIXES) {
      if (key.startsWith(deadPrefix)) {
        // Find line number in the file
        const keyParts = key.split(".");
        const lastPart = keyParts[keyParts.length - 1];
        const keyIdx = content.indexOf(`"${lastPart}"`);
        const line = keyIdx >= 0 ? lineOf(content, keyIdx) : 0;

        hits.push({
          file: `src/i18n/locales/en/translation.json`,
          line,
          label: `dead i18n prefix: ${deadPrefix}*`,
          snippet: key,
        });
        break; // one prefix match per key
      }
    }
  }

  return hits;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main(): Promise<number> {
  const [rustHits, i18nHits] = await Promise.all([
    scanRustAndTs(),
    scanI18nKeys(),
  ]);
  const allHits = [...rustHits, ...i18nHits];

  if (allHits.length === 0) {
    console.log(
      "[handy-legacy] OK - no new usage of known-dead Handy-era symbols outside declaration sites.",
    );
    return 0;
  }

  const level = STRICT ? "FAIL" : "WARN";
  const exitCode = STRICT ? 1 : 0;

  console.error(
    `[handy-legacy] ${level} - ${allHits.length} dead-symbol reference(s) outside allowed sites:`,
  );

  // Group by label for readability
  const byLabel = new Map<string, Hit[]>();
  for (const hit of allHits) {
    const list = byLabel.get(hit.label) ?? [];
    list.push(hit);
    byLabel.set(hit.label, list);
  }

  for (const [label, hits] of byLabel) {
    console.error(
      `\n  ${label} (${hits.length} hit${hits.length > 1 ? "s" : ""}):`,
    );
    for (const hit of hits) {
      console.error(`    ${hit.file}:${hit.line} ...${hit.snippet}...`);
    }
  }

  console.error(
    "\nDead Handy-era symbols must not receive new code. See .github/skills/handy-legacy-pruning/SKILL.md.",
  );
  console.error(
    "If the symbol is still live for editor use, add it to the allowlist with justification.",
  );

  if (!STRICT) {
    console.error(
      "\n(Report-only mode. Pass --strict to fail on handy-legacy usage.)",
    );
  }

  return exitCode;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[handy-legacy] internal error:", err);
    process.exit(2);
  });
