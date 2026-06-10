#!/usr/bin/env bun

import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";

import { nonEmpty, requiredArgValue } from "./cli-args.ts";
import { checkReleaseReadiness } from "./check-release-readiness.ts";
import { readJsonRecord as readJsonObject } from "./json-utils.ts";
import { gitHead, isRecord, repoRoot } from "./release-evidence.ts";

const defaultReadinessPath = resolve(repoRoot, "docs/release-readiness.json");
const defaultVersionPath = resolve(repoRoot, "VERSION");

type ReleaseReadiness = {
  releaseVersion: string;
  fuzzCleanRun: unknown;
  externalReview: unknown;
};

type MergeReleaseReadinessInput = {
  releaseVersion: string;
  fuzzPayload: unknown;
  reviewPayload: unknown;
};

type ApplyReleaseReadinessOptions = {
  fuzzPath: string;
  reviewPath: string;
  outPath: string;
  releaseVersion: string;
  expectedCommit?: string;
};

type CliOptions = {
  fuzzPath: string;
  reviewPath: string;
  outPath: string;
  releaseVersion: string;
  expectedCommit: string;
};

if (import.meta.main) {
  const options = parseCliOptions(process.argv.slice(2));
  const readiness = applyReleaseReadiness(options);
  console.log(
    [
      "seagrass release readiness evidence applied",
      `path: ${options.outPath}`,
      `version: ${readiness.releaseVersion}`,
      `commit: ${options.expectedCommit}`,
    ].join("\n"),
  );
}

export function applyReleaseReadiness(options: ApplyReleaseReadinessOptions): ReleaseReadiness {
  const readiness = mergeReleaseReadiness({
    releaseVersion: options.releaseVersion,
    fuzzPayload: readJsonObject(options.fuzzPath, "--fuzz", "must point to a JSON object"),
    reviewPayload: readJsonObject(options.reviewPath, "--review", "must point to a JSON object"),
  });
  assertStrictReleaseReadiness(readiness, {
    releaseVersion: options.releaseVersion,
    expectedCommit: options.expectedCommit ?? gitHead(),
  });
  writeReleaseReadiness(options.outPath, readiness);
  return readiness;
}

export function mergeReleaseReadiness(input: MergeReleaseReadinessInput): ReleaseReadiness {
  return {
    releaseVersion: nonEmpty(input.releaseVersion, "--version"),
    fuzzCleanRun: objectField(input.fuzzPayload, "fuzzCleanRun"),
    externalReview: objectField(input.reviewPayload, "externalReview"),
  };
}

function assertStrictReleaseReadiness(
  readiness: ReleaseReadiness,
  expected: { releaseVersion: string; expectedCommit: string },
): void {
  const readinessPath = writeValidationCopy(readiness);
  const result = checkReleaseReadiness({
    allowPending: false,
    readinessPath,
    expectedVersion: expected.releaseVersion,
    expectedCommit: expected.expectedCommit,
  });
  if (result.status !== "passed") {
    throw new Error(
      `release readiness evidence failed strict validation:\n${result.failures.join("\n")}`,
    );
  }
}

function writeReleaseReadiness(path: string, readiness: ReleaseReadiness): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(readiness, null, 2)}\n`);
}

function writeValidationCopy(readiness: ReleaseReadiness): string {
  const dir = mkdtempSync(resolve(tmpdir(), "seagrass-release-readiness-"));
  const path = resolve(dir, "release-readiness.json");
  writeReleaseReadiness(path, readiness);
  return path;
}

function objectField(record: unknown, field: string): unknown {
  if (!isRecord(record)) {
    throw new Error(`${field} payload must be a JSON object`);
  }
  const value = record[field];
  if (!isRecord(value)) {
    throw new Error(`${field} must be an object`);
  }
  return value;
}

function parseCliOptions(args: string[]): CliOptions {
  const options: Partial<CliOptions> = {
    releaseVersion: readFileSync(defaultVersionPath, "utf8").trim(),
    expectedCommit: gitHead(),
    outPath: defaultReadinessPath,
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    if (arg === "--fuzz") {
      options.fuzzPath = resolve(repoRoot, requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--review") {
      options.reviewPath = resolve(repoRoot, requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--out") {
      options.outPath = resolve(repoRoot, requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--version") {
      options.releaseVersion = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--commit") {
      options.expectedCommit = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  if (!options.fuzzPath) {
    throw new Error("--fuzz is required");
  }
  if (!options.reviewPath) {
    throw new Error("--review is required");
  }
  return options as CliOptions;
}

function printHelp(): void {
  console.log(`Apply Seagrass release readiness evidence.

Usage:
  bun scripts/apply-release-readiness.ts --fuzz <path> --review <path> [options]

Options:
  --version <version>  Release version. Default: VERSION.
  --commit <sha>       Release commit. Default: current git HEAD.
  --out <path>         Output readiness file. Default: docs/release-readiness.json.

Examples:
  bun scripts/apply-release-readiness.ts --fuzz target/fuzz-readiness-<sha>.json --review target/review-readiness.json
  bun scripts/apply-release-readiness.ts --fuzz target/fuzz.json --review target/review.json --version 0.1.0 --commit <sha>`);
}
