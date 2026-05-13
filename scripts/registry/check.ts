#!/usr/bin/env bun
/**
 * gate/check-registry.ts
 *
 * CI drift gate for `.github/_shared/registry/*.json`.
 *
 * Validates that rules.json and pipeline-registry.json parse correctly,
 * declare `version: 1`, and that any `$schema` pointers resolve.
 *
 * Exit codes:
 *   0 — registries clean
 *   1 — malformed JSON, missing file, or broken schema ref
 */
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const REGISTRY_DIR = path.join(REPO_ROOT, ".github", "_shared", "registry");

const REQUIRED = ["rules.json", "pipeline-registry.json"];

function fail(msg: string): never {
  console.error(`FAIL: ${msg}`);
  process.exit(1);
}

function main(): void {
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

  console.log("registry gate: OK");
}

main();
