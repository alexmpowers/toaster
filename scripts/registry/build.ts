#!/usr/bin/env bun
/**
 * build-registry.ts
 *
 * Auto-generates `.github/registry/skills.json` and `.github/registry/agents.json`
 * from the frontmatter of local `.github/skills/<name>/SKILL.md` and
 * `.github/agents/<name>.agent.md` files.
 *
 * The registry JSON is the single machine-readable index consumed by
 * `scripts/agents-registry.ts` and by CI drift checks. Hand-editing those
 * two files is forbidden — edit the frontmatter of the source SKILL.md /
 * *.agent.md and re-run this generator.
 *
 * Usage:
 *   bun scripts/build-registry.ts            # write both files
 *   bun scripts/build-registry.ts --check    # exit 1 if any file is stale
 */
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const SKILLS_DIR = path.join(REPO_ROOT, ".github", "skills");
const AGENTS_DIR = path.join(REPO_ROOT, ".github", "agents");
const REGISTRY_DIR = path.join(REPO_ROOT, ".github", "registry");

type Frontmatter = { name?: string; description?: string };

function parseFrontmatter(src: string): Frontmatter {
  if (!src.startsWith("---")) return {};
  const end = src.indexOf("\n---", 3);
  if (end === -1) return {};
  const body = src.slice(3, end);
  const out: Frontmatter = {};

  // Simple YAML scanner that handles `key: value`, `key: 'quoted'`,
  // `key: "quoted"`, folded scalars (`key: >`), and literal scalars
  // (`key: |`). Sufficient for name/description fields only.
  const lines = body.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const m = /^([A-Za-z_][A-Za-z0-9_-]*):\s*(.*)$/.exec(line);
    if (!m) continue;
    const key = m[1];
    let val = m[2];
    if (val === ">" || val === "|" || val === ">-" || val === "|-") {
      const parts: string[] = [];
      i++;
      while (i < lines.length) {
        const next = lines[i];
        if (!/^\s+\S/.test(next) && next.trim() !== "") break;
        parts.push(next.trim());
        i++;
      }
      i--;
      val = parts.join(" ").trim();
    } else {
      val = val.trim();
      if (
        (val.startsWith('"') && val.endsWith('"')) ||
        (val.startsWith("'") && val.endsWith("'"))
      ) {
        val = val.slice(1, -1);
      }
    }
    (out as Record<string, string>)[key] = val;
  }
  return out;
}

function readSkills(): Array<{
  name: string;
  description: string;
  path: string;
}> {
  if (!fs.existsSync(SKILLS_DIR)) return [];
  const out: Array<{ name: string; description: string; path: string }> = [];
  for (const dir of fs.readdirSync(SKILLS_DIR)) {
    const skillMd = path.join(SKILLS_DIR, dir, "SKILL.md");
    if (!fs.existsSync(skillMd)) continue;
    const src = fs.readFileSync(skillMd, "utf8");
    const fm = parseFrontmatter(src);
    if (!fm.name || !fm.description) {
      console.error(`warn: ${skillMd} missing name/description in frontmatter`);
      continue;
    }
    out.push({
      name: fm.name,
      description: fm.description,
      path: path.relative(REPO_ROOT, skillMd).replaceAll("\\", "/"),
    });
  }
  out.sort((a, b) => a.name.localeCompare(b.name));
  return out;
}

function readAgents(): Array<{
  name: string;
  description: string;
  path: string;
}> {
  if (!fs.existsSync(AGENTS_DIR)) return [];
  const out: Array<{ name: string; description: string; path: string }> = [];
  for (const entry of fs.readdirSync(AGENTS_DIR)) {
    if (!entry.endsWith(".agent.md") && !entry.endsWith(".md")) continue;
    const full = path.join(AGENTS_DIR, entry);
    const stat = fs.statSync(full);
    if (stat.isDirectory()) continue;
    const src = fs.readFileSync(full, "utf8");
    const fm = parseFrontmatter(src);
    const name = fm.name ?? entry.replace(/\.(agent\.)?md$/, "");
    if (!fm.description) {
      console.error(`warn: ${full} missing description in frontmatter`);
      continue;
    }
    out.push({
      name,
      description: fm.description,
      path: path.relative(REPO_ROOT, full).replaceAll("\\", "/"),
    });
  }
  out.sort((a, b) => a.name.localeCompare(b.name));
  return out;
}

function render(kind: "skills" | "agents", items: unknown[]): string {
  const source =
    kind === "skills"
      ? ".github/skills/*/SKILL.md frontmatter"
      : ".github/agents/*.agent.md frontmatter";
  const obj = {
    $schema: `./schema/${kind}.schema.json`,
    version: 1,
    source,
    [kind]: items,
  };
  return JSON.stringify(obj, null, 2) + "\n";
}

function main(): void {
  const check = process.argv.includes("--check");
  const skills = readSkills();
  const agents = readAgents();

  const skillsJson = render("skills", skills);
  const agentsJson = render("agents", agents);

  const skillsPath = path.join(REGISTRY_DIR, "skills.json");
  const agentsPath = path.join(REGISTRY_DIR, "agents.json");

  if (check) {
    const existingSkills = fs.existsSync(skillsPath)
      ? fs.readFileSync(skillsPath, "utf8")
      : "";
    const existingAgents = fs.existsSync(agentsPath)
      ? fs.readFileSync(agentsPath, "utf8")
      : "";
    if (existingSkills !== skillsJson || existingAgents !== agentsJson) {
      console.error(
        "registry drift: run `bun scripts/build-registry.ts` to regenerate skills.json / agents.json",
      );
      process.exit(1);
    }
    console.log(
      `registry OK: ${skills.length} skills, ${agents.length} agents`,
    );
    return;
  }

  fs.mkdirSync(REGISTRY_DIR, { recursive: true });
  fs.writeFileSync(skillsPath, skillsJson);
  fs.writeFileSync(agentsPath, agentsJson);
  console.log(
    `wrote ${skillsPath} (${skills.length}) and ${agentsPath} (${agents.length})`,
  );
}

main();
