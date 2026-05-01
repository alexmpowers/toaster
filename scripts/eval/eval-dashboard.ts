#!/usr/bin/env bun
/**
 * Unified eval dashboard.
 *
 * Runs all CI gate scripts and produces a consolidated pass/fail report.
 * Useful for local pre-push verification and agent self-checks.
 *
 * Usage:
 *   bun scripts/eval/eval-dashboard.ts           # run all gates
 *   bun scripts/eval/eval-dashboard.ts --json     # JSON output
 *   bun scripts/eval/eval-dashboard.ts --strict   # exit 1 on any failure
 *
 * Exit codes: 0 all green, 1 at least one gate failed (strict mode), 2 internal error.
 */

import { relative } from "node:path";

const STRICT = process.argv.includes("--strict");
const JSON_OUTPUT = process.argv.includes("--json");

interface GateResult {
  name: string;
  command: string;
  status: "pass" | "warn" | "fail" | "skip";
  duration_ms: number;
  output: string;
}

/** Gate definitions — ordered by speed (fastest first) */
const GATES: { name: string; command: string }[] = [
  {
    name: "instruction-drift",
    command: "bun scripts/gate/check-instruction-drift.ts",
  },
  {
    name: "handy-legacy",
    command: "bun scripts/gate/check-handy-legacy-usage.ts",
  },
  {
    name: "backend-authority",
    command: "bun scripts/gate/check-backend-authority-drift.ts",
  },
  {
    name: "adapter-schema",
    command: "bun scripts/gate/check-adapter-schema.ts",
  },
  {
    name: "feature-state",
    command: "bun scripts/gate/check-feature-state.ts",
  },
  {
    name: "reachability",
    command: "bun scripts/gate/check-reachability.ts",
  },
  {
    name: "brand-token-drift",
    command: "bun scripts/gate/check-brand-token-drift.ts",
  },
  {
    name: "button-variants",
    command: "bun scripts/gate/check-button-variant-drift.ts",
  },
  {
    name: "settings-updater",
    command: "bun scripts/gate/check-settings-updater-coverage.ts",
  },
  {
    name: "file-sizes",
    command: "bun scripts/check-file-sizes.ts",
  },
  {
    name: "translations",
    command: "bun scripts/check-translations.ts",
  },
  {
    name: "registry-drift",
    command: "bun scripts/registry/check.ts",
  },
];

async function runGate(gate: {
  name: string;
  command: string;
}): Promise<GateResult> {
  const start = performance.now();
  try {
    const proc = Bun.spawn(gate.command.split(" "), {
      cwd: process.cwd(),
      stdout: "pipe",
      stderr: "pipe",
    });

    const [stdout, stderr] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ]);
    const exitCode = await proc.exited;
    const duration_ms = Math.round(performance.now() - start);
    const output = (stdout + stderr).trim();

    let status: GateResult["status"];
    if (exitCode === 0) {
      status = output.includes("WARN") ? "warn" : "pass";
    } else {
      status = "fail";
    }

    return {
      name: gate.name,
      command: gate.command,
      status,
      duration_ms,
      output,
    };
  } catch (err) {
    const duration_ms = Math.round(performance.now() - start);
    return {
      name: gate.name,
      command: gate.command,
      status: "skip",
      duration_ms,
      output: `Error: ${err}`,
    };
  }
}

function statusIcon(s: GateResult["status"]): string {
  switch (s) {
    case "pass":
      return "✓";
    case "warn":
      return "⚠";
    case "fail":
      return "✗";
    case "skip":
      return "○";
  }
}

async function main(): Promise<number> {
  const totalStart = performance.now();

  if (!JSON_OUTPUT) {
    console.log("Running all gates...\n");
  }

  const results: GateResult[] = [];
  for (const gate of GATES) {
    if (!JSON_OUTPUT) {
      process.stdout.write(`  ${gate.name}... `);
    }
    const result = await runGate(gate);
    results.push(result);
    if (!JSON_OUTPUT) {
      console.log(`${statusIcon(result.status)} (${result.duration_ms}ms)`);
    }
  }

  const totalMs = Math.round(performance.now() - totalStart);

  if (JSON_OUTPUT) {
    const summary = {
      total: results.length,
      pass: results.filter((r) => r.status === "pass").length,
      warn: results.filter((r) => r.status === "warn").length,
      fail: results.filter((r) => r.status === "fail").length,
      skip: results.filter((r) => r.status === "skip").length,
      duration_ms: totalMs,
      results: results.map(({ name, status, duration_ms }) => ({
        name,
        status,
        duration_ms,
      })),
    };
    console.log(JSON.stringify(summary, null, 2));
  } else {
    const pass = results.filter((r) => r.status === "pass").length;
    const warn = results.filter((r) => r.status === "warn").length;
    const fail = results.filter((r) => r.status === "fail").length;
    const skip = results.filter((r) => r.status === "skip").length;

    console.log(`\n${"─".repeat(50)}`);
    console.log(
      `Dashboard: ${pass} pass, ${warn} warn, ${fail} fail, ${skip} skip (${totalMs}ms)`,
    );

    if (fail > 0) {
      console.log("\nFailed gates:");
      for (const r of results.filter((r) => r.status === "fail")) {
        console.log(`\n  ${r.name}:`);
        for (const line of r.output.split("\n").slice(0, 5)) {
          console.log(`    ${line}`);
        }
      }
    }

    if (warn > 0) {
      console.log("\nWarnings:");
      for (const r of results.filter((r) => r.status === "warn")) {
        // Show first warning line
        const warnLine = r.output.split("\n").find((l) => l.includes("WARN"));
        if (warnLine) console.log(`  ${r.name}: ${warnLine.trim()}`);
      }
    }
  }

  if (STRICT && results.some((r) => r.status === "fail")) {
    return 1;
  }
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((err) => {
    console.error("[eval-dashboard] internal error:", err);
    process.exit(2);
  });
