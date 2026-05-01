#!/usr/bin/env bun
/**
 * Task context auto-writer.
 *
 * Generates `features/<slug>/tasks/<task-id>/context.md` briefings from
 * the existing PRD, BLUEPRINT, coverage.json, and tasks.sql artifacts.
 *
 * Usage:
 *   bun scripts/feature/generate-task-context.ts <slug>              # all tasks
 *   bun scripts/feature/generate-task-context.ts <slug> <task-id>    # one task
 *   bun scripts/feature/generate-task-context.ts <slug> --dry-run    # preview
 *
 * Each context.md is a curated-for-fresh-subagent briefing containing:
 *   - Relevant R-IDs and their PRD acceptance criteria
 *   - Blueprint decisions for those R-IDs
 *   - Verifiers from coverage.json
 *   - File references from task description
 *   - Dependency chain from tasks.sql
 */

import { mkdir, readFile, writeFile, stat } from "node:fs/promises";
import { join, dirname } from "node:path";

const ROOT = process.cwd();
const args = process.argv.slice(2);

if (args.length === 0 || args.includes("--help")) {
  console.log(
    "Usage: bun scripts/feature/generate-task-context.ts <slug> [task-id] [--dry-run]",
  );
  process.exit(0);
}

const slug = args[0];
const DRY_RUN = args.includes("--dry-run");
const singleTask = args.find((a) => a !== slug && !a.startsWith("--"));
const featureDir = join(ROOT, "features", slug);

async function exists(p: string): Promise<boolean> {
  try {
    await stat(p);
    return true;
  } catch {
    return false;
  }
}

// ── Parse tasks.sql ───────────────────────────────────────────────────

interface Task {
  id: string;
  title: string;
  description: string;
}

interface TaskDep {
  todoId: string;
  dependsOn: string;
}

function parseTasks(sql: string): { tasks: Task[]; deps: TaskDep[] } {
  const tasks: Task[] = [];
  const deps: TaskDep[] = [];

  // Match INSERT INTO todos rows: ('id', 'title', 'description', 'status')
  const todoRe =
    /\(\s*'([^']+)'\s*,\s*'([^']+)'\s*,\s*'([^']+)'\s*,\s*'[^']+'\s*\)/g;
  let m;
  while ((m = todoRe.exec(sql)) !== null) {
    tasks.push({
      id: m[1],
      title: m[2],
      description: m[3],
    });
  }

  // Match INSERT INTO todo_deps rows: ('todo_id', 'depends_on')
  const depRe = /\(\s*'([^']+)'\s*,\s*'([^']+)'\s*\)/g;
  // Only scan after "todo_deps" keyword
  const depSection = sql.slice(sql.indexOf("todo_deps") || 0);
  while ((m = depRe.exec(depSection)) !== null) {
    deps.push({ todoId: m[1], dependsOn: m[2] });
  }

  return { tasks, deps };
}

// ── Parse coverage.json ───────────────────────────────────────────────

interface CoverageEntry {
  kind: string;
  verifier: string;
  command?: string;
  task?: string;
  steps?: string[];
}

function getTaskACs(
  coverage: Record<string, CoverageEntry>,
  taskId: string,
): [string, CoverageEntry][] {
  return Object.entries(coverage).filter(([, entry]) => entry.task === taskId);
}

// ── Parse PRD for R-ID → AC mapping ───────────────────────────────────

function extractRIds(acIds: string[]): string[] {
  // AC-NNN-x → R-NNN
  const rIds = new Set<string>();
  for (const ac of acIds) {
    const m = ac.match(/^AC-(\d+)-/);
    if (m) rIds.add(`R-${m[1]}`);
  }
  return [...rIds].sort();
}

function extractRIdSections(prd: string, rIds: string[]): string {
  if (rIds.length === 0) return "(no R-IDs mapped)";

  const lines = prd.split("\n");
  const sections: string[] = [];

  for (const rId of rIds) {
    // Find section header containing R-ID
    const headerIdx = lines.findIndex(
      (l) => l.includes(rId) && /^#+\s/.test(l),
    );
    if (headerIdx === -1) {
      sections.push(`- ${rId}: (section not found in PRD)`);
      continue;
    }

    // Find end of section (next same-level or higher header)
    const headerLevel = (lines[headerIdx].match(/^#+/) || [""])[0].length;
    let endIdx = lines.length;
    for (let i = headerIdx + 1; i < lines.length; i++) {
      const m = lines[i].match(/^(#+)\s/);
      if (m && m[1].length <= headerLevel) {
        endIdx = i;
        break;
      }
    }

    sections.push(lines.slice(headerIdx, endIdx).join("\n").trim());
  }

  return sections.join("\n\n");
}

// ── Generate context.md ───────────────────────────────────────────────

function generateContext(
  task: Task,
  taskACs: [string, CoverageEntry][],
  rIds: string[],
  prdSlice: string,
  deps: TaskDep[],
  allTasks: Task[],
): string {
  const taskDeps = deps
    .filter((d) => d.todoId === task.id)
    .map((d) => {
      const depTask = allTasks.find((t) => t.id === d.dependsOn);
      return depTask
        ? `- \`${d.dependsOn}\` — ${depTask.title}`
        : `- \`${d.dependsOn}\``;
    });

  const verifiers = taskACs.map(([acId, entry]) => {
    let v = `- **${acId}** (${entry.kind})`;
    if (entry.command) v += `\n  \`\`\`\n  ${entry.command}\n  \`\`\``;
    if (entry.steps) v += "\n  Steps: " + entry.steps.join(" → ");
    return v;
  });

  // Extract file references from task description
  const fileRefs = [
    ...new Set(
      task.description.match(/(?:src[-/]?[^\s,;]+|scripts\/[^\s,;]+)/g) || [],
    ),
  ];

  return `# Context briefing: ${task.id} (${task.title})

> Curated for a fresh subagent. Read only this file and the paths it
> references; do NOT load other PRD sections or unrelated repo files.

## R-IDs covered

${rIds.map((r) => `- ${r}`).join("\n") || "(none mapped)"}

## PRD slice

${prdSlice}

## Blueprint slice

See \`features/${slug}/BLUEPRINT.md\` "Architecture decisions" for ${rIds.join(" and ") || "relevant R-IDs"}.

## Task description

${task.description}
${
  taskDeps.length > 0
    ? `
## Dependencies (must be done first)

${taskDeps.join("\n")}
`
    : ""
}${
    fileRefs.length > 0
      ? `
## Files to read first

${fileRefs.map((f) => `- \`${f}\``).join("\n")}
`
      : ""
  }${
    verifiers.length > 0
      ? `
## Verifiers (from coverage.json)

${verifiers.join("\n")}

Expect all verifiers to pass (exit code 0 for scripts, green for manual).
`
      : `
## Verifiers

No acceptance criteria directly mapped to this task in coverage.json.
Check related ACs after completion.
`
  }`;
}

// ── Main ──────────────────────────────────────────────────────────────

async function main(): Promise<number> {
  // Validate feature exists
  if (!(await exists(featureDir))) {
    console.error(`Error: features/${slug}/ does not exist.`);
    return 1;
  }

  // Load artifacts
  const tasksSqlPath = join(featureDir, "tasks.sql");
  const coveragePath = join(featureDir, "coverage.json");
  const prdPath = join(featureDir, "PRD.md");

  if (!(await exists(tasksSqlPath))) {
    console.error(`Error: features/${slug}/tasks.sql not found.`);
    return 1;
  }

  const tasksSql = await readFile(tasksSqlPath, "utf8");
  const { tasks, deps } = parseTasks(tasksSql);

  if (tasks.length === 0) {
    console.error("Error: no tasks found in tasks.sql.");
    return 1;
  }

  // Load coverage if available
  let coverageACs: Record<string, CoverageEntry> = {};
  if (await exists(coveragePath)) {
    try {
      const raw = JSON.parse(await readFile(coveragePath, "utf8"));
      coverageACs = raw.acs || {};
    } catch {
      console.warn("Warning: coverage.json could not be parsed.");
    }
  }

  // Load PRD if available
  let prd = "";
  if (await exists(prdPath)) {
    prd = await readFile(prdPath, "utf8");
  }

  // Filter tasks
  const targetTasks = singleTask
    ? tasks.filter((t) => t.id === singleTask)
    : tasks;

  if (targetTasks.length === 0) {
    console.error(
      `Error: task "${singleTask}" not found. Available: ${tasks.map((t) => t.id).join(", ")}`,
    );
    return 1;
  }

  let generated = 0;
  for (const task of targetTasks) {
    const taskACs = getTaskACs(coverageACs, task.id);
    const acIds = taskACs.map(([id]) => id);
    const rIds = extractRIds(acIds);
    const prdSlice = prd
      ? extractRIdSections(prd, rIds)
      : `See \`features/${slug}/PRD.md\` for requirements.`;

    const content = generateContext(task, taskACs, rIds, prdSlice, deps, tasks);
    const outPath = join(featureDir, "tasks", task.id, "context.md");

    if (DRY_RUN) {
      console.log(`\n--- ${task.id} → ${outPath} ---`);
      console.log(content.slice(0, 200) + "...\n");
    } else {
      await mkdir(dirname(outPath), { recursive: true });
      await writeFile(outPath, content, "utf8");
    }
    generated++;
  }

  const verb = DRY_RUN ? "Would generate" : "Generated";
  console.log(
    `\n[task-context] ${verb} ${generated} context briefing(s) for features/${slug}/`,
  );
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[task-context] internal error:", err);
    process.exit(2);
  });
