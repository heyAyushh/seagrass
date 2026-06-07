#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";

const fixturePath = resolve(repoRoot, "fixtures/preflight/anchor-errors.json");
const debugBinaryPath = resolve(repoRoot, "target/debug/seagrass");
const expectedDetectedErrors = 12;
const expectedExitCodeWithFindings = 1;

const command = existsSync(debugBinaryPath) ? debugBinaryPath : "cargo";
const args = existsSync(debugBinaryPath)
  ? ["preflight", fixturePath, "--json"]
  : ["run", "-p", "seagrass-cli", "--quiet", "--", "preflight", fixturePath, "--json"];

const result = spawnSync(command, args, {
  cwd: repoRoot,
  encoding: "utf8",
});

const failedToStart = result.status === null;

if (failedToStart && result.error) {
  throw new Error(`preflight demo failed to start: ${result.error.message}`);
}
if (result.status !== expectedExitCodeWithFindings) {
  throw new Error(
    `preflight demo should exit ${expectedExitCodeWithFindings} when errors are detected, got ${result.status}\n${result.stderr}`,
  );
}

const report = JSON.parse(result.stdout) as {
  layer?: unknown;
  runtimeEvidence?: { status?: unknown };
  summary?: { detectedErrors?: unknown };
  errors?: { coverage?: unknown }[];
};

if (report.layer !== "preflight") {
  throw new Error(`preflight demo returned wrong layer: ${JSON.stringify(report.layer)}`);
}
if (report.runtimeEvidence?.status !== "notConfigured") {
  throw new Error(
    `preflight demo should not claim runtime evidence: ${JSON.stringify(report.runtimeEvidence)}`,
  );
}
if (report.summary?.detectedErrors !== expectedDetectedErrors) {
  throw new Error(
    `preflight demo should detect ${expectedDetectedErrors} errors, got ${JSON.stringify(report.summary)}`,
  );
}
if (!report.errors?.every((error) => error.coverage === "preflight-covered")) {
  throw new Error(`preflight demo returned non-preflight coverage: ${JSON.stringify(report.errors)}`);
}

console.log(
  `ok: preflight demo detected ${expectedDetectedErrors} preflight-covered errors without runtime evidence`,
);
