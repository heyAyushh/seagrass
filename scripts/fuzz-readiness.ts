#!/usr/bin/env bun

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import { requiredArgValue } from "./cli-args.ts";
import {
  assertSameStringSet,
  compareStrings,
  expectedFuzzTargets,
  finiteIsoTimestamp,
  fuzzHours,
  MILLISECONDS_PER_HOUR,
  repoRoot,
  REQUIRED_FUZZ_HOURS,
  roundHours,
  workflowMatrixFuzzTargets,
  workflowMatrixFuzzShardCount,
} from "./release-evidence.ts";

const DEFAULT_SECONDS_PER_SHARD = 3_600;
const DEFAULT_SHARDS = 8;

type FuzzStatus = "passed" | "failed";

type FuzzTargetRun = {
  target: string;
  status: FuzzStatus;
  shards: number;
  secondsPerShard: number;
  fuzzHours: number;
};

type FuzzCleanRunEvidence = {
  status: FuzzStatus;
  workflowRunUrl: string;
  commit: string;
  startedAt: string;
  completedAt: string;
  aggregateFuzzHours: number;
  targets: string[];
  workflowMatrixTargets: string[];
  targetRuns: FuzzTargetRun[];
};

type CliOptions = {
  status: FuzzStatus;
  workflowRunUrl: string;
  commit: string;
  secondsPerShard: number;
  shards: number;
  startedAt?: string;
  completedAt: string;
  out?: string;
};

if (import.meta.main) {
  const options = parseCliOptions(process.argv.slice(2));
  const evidence = buildFuzzEvidence({
    ...options,
    targets: expectedFuzzTargets(),
    workflowMatrixTargets: workflowMatrixFuzzTargets(),
  });
  const payload = `${JSON.stringify({ fuzzCleanRun: evidence }, null, 2)}\n`;
  if (options.out) {
    const outPath = resolve(repoRoot, options.out);
    mkdirSync(dirname(outPath), { recursive: true });
    writeFileSync(outPath, payload);
  } else {
    process.stdout.write(payload);
  }
}

export function buildFuzzEvidence(input: {
  status: FuzzStatus;
  workflowRunUrl: string;
  commit: string;
  secondsPerShard: number;
  shards: number;
  startedAt?: string;
  completedAt: string;
  targets: string[];
  workflowMatrixTargets: string[];
}): FuzzCleanRunEvidence {
  const workflowShardCount = workflowMatrixFuzzShardCount();
  if (input.shards !== workflowShardCount) {
    throw new Error(
      `workflow shard count is ${workflowShardCount}, but fuzz evidence used ${input.shards}`,
    );
  }
  const targets = [...input.targets].sort(compareStrings);
  const workflowMatrixTargets = [...input.workflowMatrixTargets].sort(compareStrings);
  assertSameStringSet(workflowMatrixTargets, targets, "workflowMatrixTargets");
  const targetRuns = targets.map((target) =>
    targetRun({
      target,
      status: input.status,
      shards: input.shards,
      secondsPerShard: input.secondsPerShard,
    }),
  );
  const aggregateFuzzHours = roundHours(
    targetRuns.reduce((sum, run) => sum + run.fuzzHours, 0),
  );
  const completedAt = finiteIsoTimestamp(input.completedAt, "--completed-at");
  const startedAt =
    input.startedAt ??
    new Date(Date.parse(completedAt) - aggregateFuzzHours * MILLISECONDS_PER_HOUR).toISOString();
  const normalizedStartedAt = finiteIsoTimestamp(startedAt, "--started-at");
  if (input.status === "passed" && aggregateFuzzHours < REQUIRED_FUZZ_HOURS) {
    throw new Error(
      `passed fuzz evidence must cover at least ${REQUIRED_FUZZ_HOURS} aggregate fuzz-hours`,
    );
  }
  if (Date.parse(completedAt) <= Date.parse(normalizedStartedAt)) {
    throw new Error("fuzz evidence completedAt must be after startedAt");
  }

  return {
    status: input.status,
    workflowRunUrl: input.workflowRunUrl,
    commit: input.commit,
    startedAt: normalizedStartedAt,
    completedAt,
    aggregateFuzzHours,
    targets: targetRuns.map((run) => run.target),
    workflowMatrixTargets,
    targetRuns,
  };
}

function targetRun(input: {
  target: string;
  status: FuzzStatus;
  shards: number;
  secondsPerShard: number;
}): FuzzTargetRun {
  return {
    target: input.target,
    status: input.status,
    shards: input.shards,
    secondsPerShard: input.secondsPerShard,
    fuzzHours: fuzzHours(input.shards, input.secondsPerShard),
  };
}

function parseCliOptions(args: string[]): CliOptions {
  const options: Partial<CliOptions> = {
    secondsPerShard: DEFAULT_SECONDS_PER_SHARD,
    shards: DEFAULT_SHARDS,
    completedAt: new Date().toISOString(),
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    if (arg === "--status") {
      options.status = parseStatus(requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--workflow-run-url") {
      options.workflowRunUrl = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--commit") {
      options.commit = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--seconds-per-shard") {
      options.secondsPerShard = positiveInteger(requiredArgValue(args, index, arg), arg);
      index += 1;
      continue;
    }
    if (arg === "--shards") {
      options.shards = positiveInteger(requiredArgValue(args, index, arg), arg);
      index += 1;
      continue;
    }
    if (arg === "--completed-at") {
      options.completedAt = finiteIsoTimestamp(requiredArgValue(args, index, arg), arg);
      index += 1;
      continue;
    }
    if (arg === "--started-at") {
      options.startedAt = finiteIsoTimestamp(requiredArgValue(args, index, arg), arg);
      index += 1;
      continue;
    }
    if (arg === "--out") {
      options.out = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  if (!options.status) {
    throw new Error("--status is required");
  }
  if (!options.workflowRunUrl) {
    throw new Error("--workflow-run-url is required");
  }
  if (!options.commit) {
    throw new Error("--commit is required");
  }

  return options as CliOptions;
}

function parseStatus(value: string): FuzzStatus {
  if (value === "passed" || value === "failed") {
    return value;
  }
  throw new Error("--status must be passed or failed");
}

function positiveInteger(value: string, flag: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive integer`);
  }
  return parsed;
}

function printHelp(): void {
  console.log(`Generate Seagrass fuzz readiness evidence.

Usage:
  bun scripts/fuzz-readiness.ts --status <passed|failed> --workflow-run-url <url> --commit <sha> [options]

Options:
  --seconds-per-shard <n>  Seconds fuzzed by each matrix shard. Default: 3600.
  --shards <n>             Number of shards per target. Must match fuzz.yaml. Default: 8.
  --started-at <iso>       Workflow start timestamp. Default: computed from aggregate hours.
  --completed-at <iso>     Completion timestamp. Default: now.
  --out <path>             Write JSON to this path instead of stdout.`);
}
