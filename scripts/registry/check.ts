#!/usr/bin/env bun
/**
 * gate/check-registry.ts
 *
 * CI drift + schema gate for `.github/registry/*.json`.
 *
 * 1. Validates every registry JSON parses and declares `version: 1`.
 * 2. Validates that `skills.json` and `agents.json` are in sync with the
 *    frontmatter of `.github/skills/<name>/SKILL.md` and
 *    `.github/agents/<name>.agent.md` by running `build-registry.ts --check`.
 * 3. Validates that every `$schema` pointer resolves to an existing file.
 *
 * Exit codes:
 *   0 — all registries clean
 *   1 — drift, missing schema, or malformed JSON
 */
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { spawnSync } from "child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const REGISTRY_DIR = path.join(REPO_ROOT, ".github", "registry");

const REQUIRED = [
  "rules.json",
  "commands.json",
  "testing.json",
  "boundaries.json",
  "hygiene.json",
  "verification.json",
  "skills.json",
  "agents.json",
];

function fail(msg: string): never {
  console.error(`FAIL: ${msg}`);
  process.exit(1);
}

function main(): void {
  // 1. All required files exist and parse with version=1.
  for (const name of REQUIRED) {
    const p = path.join(REGISTRY_DIR, name);
    if (!fs.existsSync(p)) fail(`missing ${name}`);
    let data: Record<string, unknown>;
    try {
      data = JSON.parse(fs.readFileSync(p, "utf8"));
    } catch (err) {
      fail(`malformed JSON in ${name}: ${(err as Error).message}`);
    }
    if (data.version !== 1) fail(`${name} must declare "version": 1`);

    const schemaRef = data["$schema"] as string | undefined;
    if (schemaRef) {
      const schemaPath = path.resolve(path.dirname(p), schemaRef);
      if (!fs.existsSync(schemaPath))
        fail(`${name} references missing schema ${schemaRef}`);
    }
  }

  // 2. skills.json / agents.json are in sync with source frontmatter.
  const builder = path.join(REPO_ROOT, "scripts", "registry", "build.ts");
  const result = spawnSync("bun", [builder, "--check"], {
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (result.status !== 0) fail("skills.json / agents.json are out of sync");

  console.log("registry gate: OK");
}

main();
