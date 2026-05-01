#!/usr/bin/env bun
/**
 * Feature state validator.
 *
 * Scans features/<slug>/ directories and validates:
 *   1. STATE.md exists and contains a recognized state
 *   2. Required artifacts exist for the declared state
 *   3. coverage.json is well-formed when present (AC IDs, verifier kinds)
 *   4. No orphan artifacts (e.g. coverage.json without PRD.md)
 *
 * Invocation:
 *   bun scripts/gate/check-feature-state.ts          # report-only
 *   bun scripts/gate/check-feature-state.ts --strict # exit 1 on violation
 *
 * Exit codes: 0 clean, 1 strict-mode violation, 2 internal error.
 */

import { readdir, readFile, stat } from "node:fs/promises";
import { join, relative } from "node:path";

const ROOT = process.cwd();
const STRICT = process.argv.includes("--strict");
const FEATURES_DIR = join(ROOT, "features");

/** Recognised feature states (canonical + observed legacy variants) */
const CANONICAL_STATES = [
  "defined",
  "planned",
  "executing",
  "reviewing",
  "shipped",
  "archived",
  "partial",
  "implemented", // legacy alias for shipped
] as const;

/** Artifacts required at each state (cumulative) */
const REQUIRED_AT_DEFINED = ["STATE.md", "REQUEST.md"];
const REQUIRED_AT_PLANNED = [
  ...REQUIRED_AT_DEFINED,
  "PRD.md",
  "BLUEPRINT.md",
  "coverage.json",
];

/** States that require planned-tier artifacts */
const PLANNED_AND_BEYOND = [
  "planned",
  "executing",
  "reviewing",
  "shipped",
  "partial",
  "implemented",
];

/** Valid verifier kinds in coverage.json */
const VALID_VERIFIER_KINDS = [
  "skill",
  "agent",
  "cargo-test",
  "script",
  "manual",
  "doc-section",
];

/** AC-NNN-x pattern */
const AC_PATTERN = /^AC-\d{3}-[a-z]$/;

type Violation = { slug: string; invariant: string; detail: string };

async function exists(p: string): Promise<boolean> {
  try {
    await stat(p);
    return true;
  } catch {
    return false;
  }
}

async function validateFeature(slug: string): Promise<Violation[]> {
  const dir = join(FEATURES_DIR, slug);
  const violations: Violation[] = [];

  // 1. STATE.md must exist
  const statePath = join(dir, "STATE.md");
  if (!(await exists(statePath))) {
    violations.push({
      slug,
      invariant: "state-exists",
      detail: "Missing STATE.md",
    });
    return violations;
  }

  const rawState = (await readFile(statePath, "utf8")).trim().toLowerCase();
  if (
    !CANONICAL_STATES.includes(rawState as (typeof CANONICAL_STATES)[number])
  ) {
    violations.push({
      slug,
      invariant: "state-recognized",
      detail: `STATE.md contains unrecognized state: "${rawState}"`,
    });
    return violations;
  }

  // 2. Required artifacts for declared state
  const requiredFiles = PLANNED_AND_BEYOND.includes(rawState)
    ? REQUIRED_AT_PLANNED
    : REQUIRED_AT_DEFINED;

  for (const file of requiredFiles) {
    if (!(await exists(join(dir, file)))) {
      violations.push({
        slug,
        invariant: "required-artifact",
        detail: `State "${rawState}" requires ${file} but it is missing`,
      });
    }
  }

  // 3. Validate coverage.json schema when present
  const coveragePath = join(dir, "coverage.json");
  if (await exists(coveragePath)) {
    try {
      const raw = await readFile(coveragePath, "utf8");
      const coverage = JSON.parse(raw);

      if (!coverage.feature || typeof coverage.feature !== "string") {
        violations.push({
          slug,
          invariant: "coverage-has-feature",
          detail: 'coverage.json missing or empty "feature" field',
        });
      }

      if (!coverage.acs || typeof coverage.acs !== "object") {
        violations.push({
          slug,
          invariant: "coverage-has-acs",
          detail: 'coverage.json missing or invalid "acs" object',
        });
      } else {
        for (const [acId, entry] of Object.entries(coverage.acs)) {
          if (!AC_PATTERN.test(acId)) {
            violations.push({
              slug,
              invariant: "coverage-ac-format",
              detail: `AC ID "${acId}" does not match pattern AC-NNN-x`,
            });
          }
          const e = entry as Record<string, unknown>;
          if (
            typeof e.kind === "string" &&
            !VALID_VERIFIER_KINDS.includes(e.kind)
          ) {
            violations.push({
              slug,
              invariant: "coverage-verifier-kind",
              detail: `AC "${acId}" has unknown verifier kind: "${e.kind}"`,
            });
          }
          if (
            e.kind === "manual" &&
            (!Array.isArray(e.steps) || e.steps.length === 0)
          ) {
            violations.push({
              slug,
              invariant: "coverage-manual-steps",
              detail: `AC "${acId}" is kind=manual but has no steps array`,
            });
          }
        }
      }
    } catch {
      violations.push({
        slug,
        invariant: "coverage-valid-json",
        detail: "coverage.json is not valid JSON",
      });
    }
  }

  // 4. Orphan check: coverage.json without PRD.md
  if ((await exists(coveragePath)) && !(await exists(join(dir, "PRD.md")))) {
    violations.push({
      slug,
      invariant: "no-orphan-coverage",
      detail: "coverage.json exists without PRD.md",
    });
  }

  return violations;
}

async function main(): Promise<number> {
  let entries: string[];
  try {
    const dirents = await readdir(FEATURES_DIR, { withFileTypes: true });
    entries = dirents
      .filter((d) => d.isDirectory() && !d.name.startsWith("."))
      .map((d) => d.name);
  } catch {
    console.log(
      `[feature-state] SKIP - features directory not found: ${relative(ROOT, FEATURES_DIR)}`,
    );
    return 0;
  }

  if (entries.length === 0) {
    console.log("[feature-state] SKIP - no feature directories found.");
    return 0;
  }

  const allViolations: Violation[] = [];
  for (const slug of entries) {
    allViolations.push(...(await validateFeature(slug)));
  }

  if (allViolations.length === 0) {
    console.log(
      `[feature-state] OK - ${entries.length} feature(s) pass all state/artifact invariants.`,
    );
    return 0;
  }

  const level = STRICT ? "FAIL" : "WARN";
  const exitCode = STRICT ? 1 : 0;

  console.error(
    `[feature-state] ${level} - ${allViolations.length} issue(s) across ${entries.length} feature(s):`,
  );
  for (const v of allViolations) {
    console.error(`  features/${v.slug}/ [${v.invariant}] ${v.detail}`);
  }
  console.error("\nSee docs/spec-driven.md for the feature state machine.");

  if (!STRICT) {
    console.error(
      "\n(Report-only mode. Pass --strict to fail on state violations.)",
    );
  }

  return exitCode;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[feature-state] internal error:", err);
    process.exit(2);
  });
