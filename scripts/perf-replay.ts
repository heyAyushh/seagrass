#!/usr/bin/env bun

import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const defaultReportPath = resolve(repoRoot, "target/seagrass-hotpath-report.json");
const hotpathServerBinary = resolve(repoRoot, "target/release/seagrass");
const nanosPerMillisecond = 1_000_000;

export const handlerBudgets = [
  { label: "completion", name: "seagrass::server::completion", p99Millis: 100 },
  {
    label: "hover",
    name: "seagrass::server::navigation_handlers::hover_impl",
    p99Millis: 50,
  },
  {
    label: "diagnostics",
    name: "seagrass::server::diagnostic_pipeline::parse_index_and_publish_hot",
    p99Millis: 150,
  },
  {
    label: "workspace scan",
    name: "seagrass::server::backend_features::refresh_workspace_index",
    p99Millis: 2_000,
  },
];

export function parseDurationNanos(value) {
  if (typeof value !== "string") {
    throw new Error(`expected duration string, got ${typeof value}`);
  }
  const match = /^([0-9]+(?:\.[0-9]+)?)\s*(ns|µs|us|ms|s)$/.exec(value.trim());
  if (!match) {
    throw new Error(`unsupported duration: ${value}`);
  }

  const amount = Number(match[1]);
  const unit = match[2];
  const multiplier = {
    ns: 1,
    "µs": 1_000,
    us: 1_000,
    ms: nanosPerMillisecond,
    s: 1_000_000_000,
  }[unit];

  return Math.round(amount * multiplier);
}

export function assertHotpathBudgetReport(report, budgets = handlerBudgets) {
  const entries = report?.functions_timing?.data;
  if (!Array.isArray(entries)) {
    throw new Error("hotpath report is missing functions_timing.data");
  }

  budgets.forEach((budget) => {
    const entry = entries.find(
      (candidate) =>
        candidate?.name === budget.name &&
        Number(candidate.calls ?? 0) > 0,
    );
    if (!entry) {
      throw new Error(`missing hotpath measurement for ${budget.label}`);
    }

    const p99 = entry.p99 ?? entry.p99_0 ?? entry["p99.0"];
    const p99Nanos = parseDurationNanos(p99);
    const budgetNanos = budget.p99Millis * nanosPerMillisecond;
    if (p99Nanos > budgetNanos) {
      throw new Error(
        `${budget.label} exceeded p99 budget: ${p99} > ${budget.p99Millis} ms (${entry.name})`,
      );
    }
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runPerfReplay();
}

function runPerfReplay() {
  const reportPath = process.env.SEAGRASS_HOTPATH_REPORT ?? defaultReportPath;
  mkdirSync(dirname(reportPath), { recursive: true });
  runChecked("cargo", ["build", "-p", "seagrass", "--features", "hotpath", "--release"]);

  const result = spawnSync("bun", ["scripts/hotpath-replay-session.ts"], {
    cwd: repoRoot,
    stdio: "inherit",
    env: {
      ...process.env,
      SEAGRASS_HOTPATH: "1",
      SEAGRASS_SERVER_BINARY: hotpathServerBinary,
      SEAGRASS_WAIT_FOR_EXIT: "1",
      HOTPATH_OUTPUT_PATH: reportPath,
      HOTPATH_OUTPUT_FORMAT: "json-pretty",
      HOTPATH_REPORT: "functions-timing",
      HOTPATH_FUNCTIONS_LIMIT: "0",
      HOTPATH_METRICS_SERVER_OFF: "true",
    },
  });

  if (result.error) {
    fail(`failed to start hotpath replay: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`hotpath replay failed with exit code ${result.status}`);
  }
  if (!existsSync(reportPath)) {
    fail(`hotpath replay did not write a report: ${reportPath}`);
  }

  const report = JSON.parse(readFileSync(reportPath, "utf8"));
  assertHotpathBudgetReport(report);
  console.log(`seagrass hotpath replay passed: ${reportPath}`);
}

function runChecked(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });

  if (result.error) {
    fail(`${command} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed with exit code ${result.status}`);
  }
}

function fail(message) {
  console.error(`\nseagrass hotpath replay failed:\n${message}`);
  process.exit(1);
}
