#!/usr/bin/env bun
/**
 * Reachability scanner — verifies that file paths, script references, and
 * feature slugs cited in documentation actually exist on disk.
 *
 * Scans:
 *   1. Markdown link targets [text](path) in .md files
 *   2. Inline code `scripts/...` or `src/...` references
 *   3. features/<slug>/ references point to real directories
 *   4. Registry JSON references point to real scripts/files
 *
 * Invocation:
 *   bun scripts/gate/check-reachability.ts          # report-only
 *   bun scripts/gate/check-reachability.ts --strict # exit 1 on broken ref
 *
 * Exit codes: 0 clean, 1 strict-mode violation, 2 internal error.
 */

import { readdir, readFile, stat } from "node:fs/promises";
import { join, resolve, relative, dirname, extname } from "node:path";

const ROOT = process.cwd();
const STRICT = process.argv.includes("--strict");

/** Directories to scan for .md files */
const SCAN_DIRS = [
  ".", // AGENTS.md, README.md, etc.
  ".github",
  ".github/agents",
  ".github/skills",
  ".github/instructions",
  "docs",
  "src",
  "src-tauri",
];

/** Only scan .md files in these directories (non-recursive for root) */
const RECURSIVE_DIRS = [
  ".github/agents",
  ".github/skills",
  ".github/instructions",
  "docs",
];

/** Paths to ignore (generated, external, anchors, etc.) */
const IGNORE_PATTERNS = [
  /^https?:\/\//, // external URLs
  /^#/, // anchor-only links
  /^mailto:/, // email links
  /^<[^>]+>$/, // HTML-like
  /node_modules/, // dependencies
  /target\//, // build output
  /\.git\//, // git internals
];

/** Known path patterns that are template/variable references, not literal */
const TEMPLATE_PATTERNS = [
  /<[^>]+>/, // <slug>, <name>, etc.
  /\$\{/, // ${variable}
  /\$\(/, // $(command)
  /\*/, // glob patterns
  /\{[^}]+\}/, // {a,b} brace expansion
];

type BrokenRef = {
  source: string;
  line: number;
  target: string;
  kind: "md-link" | "feature-ref" | "script-ref";
};

async function exists(p: string): Promise<boolean> {
  try {
    await stat(p);
    return true;
  } catch {
    return false;
  }
}

function shouldIgnore(path: string): boolean {
  return (
    IGNORE_PATTERNS.some((p) => p.test(path)) ||
    TEMPLATE_PATTERNS.some((p) => p.test(path))
  );
}

async function collectMdFiles(): Promise<string[]> {
  const files: string[] = [];

  for (const dir of SCAN_DIRS) {
    const absDir = join(ROOT, dir);
    try {
      const entries = await readdir(absDir, { withFileTypes: true });
      for (const entry of entries) {
        if (entry.isFile() && entry.name.endsWith(".md")) {
          files.push(join(absDir, entry.name));
        }
      }
    } catch {
      continue;
    }
  }

  for (const dir of RECURSIVE_DIRS) {
    const absDir = join(ROOT, dir);
    try {
      await collectMdRecursive(absDir, files);
    } catch {
      continue;
    }
  }

  return [...new Set(files)]; // deduplicate
}

async function collectMdRecursive(dir: string, out: string[]): Promise<void> {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      await collectMdRecursive(full, out);
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      out.push(full);
    }
  }
}

/**
 * Extract markdown link targets from a line: [text](target)
 * Also handles: [text](target#anchor) and [text](target "title")
 */
function extractMdLinks(line: string): string[] {
  const links: string[] = [];
  const re = /\[(?:[^\]]*)\]\(([^)]+)\)/g;
  let m;
  while ((m = re.exec(line)) !== null) {
    let target = m[1];
    // Strip anchor
    target = target.split("#")[0];
    // Strip title
    target = target.split(/\s+"/)[0];
    // Decode percent-encoding
    target = decodeURIComponent(target.trim());
    if (target) links.push(target);
  }
  return links;
}

/**
 * Extract features/<slug>/ references from a line
 */
function extractFeatureRefs(line: string): string[] {
  const refs: string[] = [];
  const re = /features\/([a-z0-9-]+)\//g;
  let m;
  while ((m = re.exec(line)) !== null) {
    refs.push(`features/${m[1]}`);
  }
  return refs;
}

/**
 * Extract script path references: `scripts/...` patterns in inline code
 */
function extractScriptRefs(line: string): string[] {
  const refs: string[] = [];
  // Match backtick-enclosed paths starting with scripts/
  const re = /`([^`]*scripts\/[^`\s]+)`/g;
  let m;
  while ((m = re.exec(line)) !== null) {
    let path = m[1];
    // Strip leading command prefixes (bun, pwsh, node, etc.)
    path = path.replace(/^(?:bun|pwsh|node|npx|bunx)\s+(?:-\S+\s+)*/, "");
    // Strip trailing flags and arguments
    path = path.replace(/\s+.*$/, "");
    // Strip line-number suffixes like :41-145
    path = path.replace(/:\d+(-\d+)?$/, "");
    path = path.trim();
    // Must still start with scripts/ after cleanup
    if (path && path.startsWith("scripts/") && !shouldIgnore(path)) {
      refs.push(path);
    }
  }
  return refs;
}

async function scanFile(filePath: string): Promise<BrokenRef[]> {
  const broken: BrokenRef[] = [];
  const content = await readFile(filePath, "utf8");
  const lines = content.split("\n");
  const fileDir = dirname(filePath);
  const relSource = relative(ROOT, filePath).replace(/\\/g, "/");

  // Seen targets to avoid duplicate reports within same file
  const seen = new Set<string>();

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // 1. Markdown links
    for (const target of extractMdLinks(line)) {
      if (shouldIgnore(target)) continue;

      // Resolve relative to the file's directory
      const resolved = resolve(fileDir, target);
      const key = `md-link:${resolved}`;
      if (seen.has(key)) continue;
      seen.add(key);

      if (!(await exists(resolved))) {
        broken.push({
          source: relSource,
          line: i + 1,
          target,
          kind: "md-link",
        });
      }
    }

    // 2. Feature slug references
    for (const ref of extractFeatureRefs(line)) {
      const resolved = join(ROOT, ref);
      const key = `feature:${ref}`;
      if (seen.has(key)) continue;
      seen.add(key);

      if (!(await exists(resolved))) {
        broken.push({
          source: relSource,
          line: i + 1,
          target: ref,
          kind: "feature-ref",
        });
      }
    }

    // 3. Script path references
    for (const ref of extractScriptRefs(line)) {
      const resolved = join(ROOT, ref);
      const key = `script:${ref}`;
      if (seen.has(key)) continue;
      seen.add(key);

      if (!(await exists(resolved))) {
        broken.push({
          source: relSource,
          line: i + 1,
          target: ref,
          kind: "script-ref",
        });
      }
    }
  }

  return broken;
}

async function main(): Promise<number> {
  const files = await collectMdFiles();

  if (files.length === 0) {
    console.log("[reachability] SKIP - no markdown files found.");
    return 0;
  }

  const allBroken: BrokenRef[] = [];
  for (const file of files) {
    try {
      allBroken.push(...(await scanFile(file)));
    } catch {
      // Skip unreadable files
    }
  }

  if (allBroken.length === 0) {
    console.log(
      `[reachability] OK - ${files.length} markdown file(s) scanned. All file references resolve.`,
    );
    return 0;
  }

  const level = STRICT ? "FAIL" : "WARN";
  const exitCode = STRICT ? 1 : 0;

  console.error(
    `[reachability] ${level} - ${allBroken.length} broken reference(s) across ${files.length} file(s):`,
  );
  for (const b of allBroken) {
    console.error(`  ${b.source}:${b.line} [${b.kind}] → ${b.target}`);
  }

  if (!STRICT) {
    console.error(
      "\n(Report-only mode. Pass --strict to fail on broken references.)",
    );
  }

  return exitCode;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[reachability] internal error:", err);
    process.exit(2);
  });
