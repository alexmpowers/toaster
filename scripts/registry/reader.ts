#!/usr/bin/env bun
/**
 * agents-registry.ts
 *
 * Reader CLI over `.github/registry/*.json`. Lets an agent pull exactly
 * the section it needs without loading the full AGENTS.md narrative.
 *
 * Usage:
 *   bun scripts/agents-registry.ts <section> [--filter key=value] [--json]
 *   bun scripts/agents-registry.ts list
 *   bun scripts/agents-registry.ts render <section>
 *
 * Sections: rules | commands | testing | boundaries | hygiene | verification
 *           | skills | agents
 *
 * Examples:
 *   bun scripts/agents-registry.ts rules --verb NEVER
 *   bun scripts/agents-registry.ts commands --tier fast
 *   bun scripts/agents-registry.ts boundaries --category never
 *   bun scripts/agents-registry.ts skills
 *   bun scripts/agents-registry.ts render commands
 */
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const REGISTRY_DIR = path.join(REPO_ROOT, ".github", "registry");

const SECTIONS = [
  "rules",
  "commands",
  "testing",
  "boundaries",
  "hygiene",
  "verification",
  "skills",
  "agents",
] as const;
type Section = (typeof SECTIONS)[number];

function load(section: Section): Record<string, unknown> {
  const p = path.join(REGISTRY_DIR, `${section}.json`);
  if (!fs.existsSync(p)) {
    console.error(`registry file missing: ${p}`);
    process.exit(2);
  }
  return JSON.parse(fs.readFileSync(p, "utf8"));
}

function parseFilters(argv: string[]): Record<string, string> {
  const filters: Record<string, string> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith("--") && !["--json", "--help"].includes(a)) {
      const key = a.slice(2);
      const val = argv[i + 1];
      if (val && !val.startsWith("--")) {
        filters[key] = val;
        i++;
      }
    }
  }
  return filters;
}

function itemKey(section: Section): string {
  switch (section) {
    case "commands":
      return "commands";
    case "testing":
      return "layers";
    case "boundaries":
      return "boundaries";
    case "verification":
      return "gates";
    case "skills":
      return "skills";
    case "agents":
      return "agents";
    default:
      return "rules";
  }
}

function applyFilters(
  items: Record<string, unknown>[],
  filters: Record<string, string>,
): Record<string, unknown>[] {
  return items.filter((item) =>
    Object.entries(filters).every(
      ([k, v]) => String(item[k] ?? "").toLowerCase() === v.toLowerCase(),
    ),
  );
}

function renderSection(
  section: Section,
  data: Record<string, unknown>,
): string {
  const key = itemKey(section);
  const items = (data[key] ?? []) as Record<string, unknown>[];
  const lines: string[] = [`## ${section}`, ""];
  if (section === "commands") {
    for (const tier of ["fast", "full", "live"] as const) {
      const subset = items.filter((i) => i.tier === tier);
      if (!subset.length) continue;
      lines.push(
        `### ${tier === "fast" ? "Fast inner loop" : tier === "full" ? "Full sweep" : "Live app + evals"}`,
        "",
        "```bash",
      );
      for (const c of subset) {
        const cwd = c.cwd ? `# (cwd: ${c.cwd}) ` : "";
        lines.push(`${cwd}${c.command as string}   # ${c.purpose}`);
      }
      lines.push("```", "");
    }
  } else if (section === "rules") {
    for (const r of items) {
      lines.push(
        `- **${r.verb}** (${r.scope}) — ${r.rule}${
          r.critical ? " _(critical)_" : ""
        }`,
      );
    }
  } else if (section === "boundaries") {
    for (const cat of ["always", "ask-first", "never"] as const) {
      const subset = items.filter((i) => i.category === cat);
      if (!subset.length) continue;
      const title =
        cat === "always"
          ? "Always do"
          : cat === "ask-first"
            ? "Ask first"
            : "Never do";
      lines.push(`### ${title}`, "");
      for (const b of subset) lines.push(`- ${b.rule}`);
      lines.push("");
    }
  } else if (section === "testing") {
    lines.push("| Layer | Command | Notes |", "|-------|---------|-------|");
    for (const l of items)
      lines.push(`| ${l.layer} | \`${l.command}\` | ${l.notes ?? ""} |`);
  } else if (section === "hygiene") {
    for (const r of items) lines.push(`- **${r.id}. ${r.name}** — ${r.rule}`);
  } else if (section === "verification") {
    for (const g of items)
      lines.push(`- _${g.when}_ → \`${g.command}\` (evidence: ${g.evidence})`);
  } else if (section === "skills" || section === "agents") {
    for (const s of items)
      lines.push(`- **${s.name}** — ${s.description} (${s.path})`);
  }
  return lines.join("\n") + "\n";
}

function main(): void {
  const [, , cmd, ...rest] = process.argv;

  if (!cmd || cmd === "--help" || cmd === "-h") {
    console.log(
      `Sections: ${SECTIONS.join(", ")}\nCommands: <section> [filters] | list | render <section>`,
    );
    return;
  }

  if (cmd === "list") {
    for (const s of SECTIONS) console.log(s);
    return;
  }

  if (cmd === "render") {
    const target = rest[0];
    if (!target || !(SECTIONS as readonly string[]).includes(target)) {
      console.error(`unknown section: ${target}`);
      process.exit(2);
    }
    const section = target as Section;
    process.stdout.write(renderSection(section, load(section)));
    return;
  }

  if (!SECTIONS.includes(cmd as Section)) {
    console.error(
      `unknown section '${cmd}'. Valid: ${SECTIONS.join(", ")} | list | render <section>`,
    );
    process.exit(2);
  }

  const section = cmd as Section;
  const data = load(section);
  const filters = parseFilters(rest);
  const jsonMode = rest.includes("--json");
  const key = itemKey(section);
  let items = (data[key] ?? []) as Record<string, unknown>[];
  if (Object.keys(filters).length) items = applyFilters(items, filters);

  if (jsonMode) {
    process.stdout.write(
      JSON.stringify({ ...data, [key]: items }, null, 2) + "\n",
    );
    return;
  }

  // Default: compact human-readable
  if (!items.length) {
    console.log("(no items)");
    return;
  }
  for (const i of items) {
    const head =
      (i.id as string) ?? (i.name as string) ?? (i.layer as string) ?? "-";
    const body =
      (i.rule as string) ??
      (i.command as string) ??
      (i.description as string) ??
      "";
    console.log(`${head}: ${body}`);
  }
}

main();
