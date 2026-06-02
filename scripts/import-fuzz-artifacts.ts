#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";

import {
  compareStrings,
  corpusTreeSha256,
  expectedFuzzTargets,
  finiteIsoTimestamp,
  isExpectedActionsRunUrl,
  isRecord,
  repoRoot,
  REQUIRED_FUZZ_HOURS,
  stringField,
  stringSetFailures,
  workflowMatrixFuzzShardCount,
  workflowMatrixFuzzTargets,
} from "./release-evidence.ts";

const tarGzipExtension = ".tar.gz";
const defaultCorpusOut = resolve(repoRoot, "fuzz/corpus");
const defaultReadinessDir = resolve(repoRoot, "target");

type FuzzTargetRun = {
  target: string;
  status: string;
  shards: number;
  secondsPerShard: number;
  fuzzHours: number;
};

type FuzzCleanRun = {
  status: string;
  workflowRunUrl: string;
  commit: string;
  startedAt: string;
  completedAt: string;
  aggregateFuzzHours: number;
  corpusSha256?: string;
  targets: string[];
  workflowMatrixTargets: string[];
  targetRuns: FuzzTargetRun[];
};

type ImportFuzzArtifactsOptions = {
  artifactRoot: string;
  expectedCommit: string;
  corpusOut: string;
  readinessOut: string;
  dryRun?: boolean;
  overwrite?: boolean;
};

type CliOptions = ImportFuzzArtifactsOptions;

type ImportPlan = {
  readinessSource: string;
  readiness: FuzzCleanRun;
  archives: FuzzArchive[];
};

type FuzzArchive = {
  path: string;
  target: string;
  shard: number;
};

type ImportFuzzArtifactsResult = {
  readinessPath: string;
  archiveCount: number;
  corpusSha256: string;
  copiedCorpusFiles: number;
  unchangedCorpusFiles: number;
};

if (import.meta.main) {
  const options = parseCliOptions(process.argv.slice(2));
  const result = importFuzzArtifacts(options);
  console.log(
    [
      options.dryRun ? "seagrass fuzz artifact import dry-run passed" : "seagrass fuzz artifacts imported",
      `readiness: ${result.readinessPath}`,
      `archives: ${result.archiveCount}`,
      `copied_corpus_files: ${result.copiedCorpusFiles}`,
      `unchanged_corpus_files: ${result.unchangedCorpusFiles}`,
    ].join("\n"),
  );
}

export function importFuzzArtifacts(options: ImportFuzzArtifactsOptions): ImportFuzzArtifactsResult {
  const plan = buildImportPlan(options.artifactRoot, options.expectedCommit);
  const staging = mkdtempSync(resolve(tmpdir(), "seagrass-fuzz-artifacts-"));
  for (const archive of plan.archives) {
    extractValidatedArchive(archive.path, staging);
  }
  const corpusSource = resolve(staging, "fuzz/corpus");
  const copyResult = copyCorpusFiles({
    source: corpusSource,
    destination: options.corpusOut,
    dryRun: options.dryRun ?? false,
    overwrite: options.overwrite ?? false,
  });
  const corpusSha256 = corpusTreeSha256((options.dryRun ?? false) ? corpusSource : options.corpusOut);
  const readiness = { ...plan.readiness, corpusSha256 };
  if (!(options.dryRun ?? false)) {
    mkdirSync(dirname(options.readinessOut), { recursive: true });
    writeFileSync(
      options.readinessOut,
      `${JSON.stringify({ fuzzCleanRun: readiness }, null, 2)}\n`,
    );
  }
  return {
    readinessPath: options.readinessOut,
    archiveCount: plan.archives.length,
    corpusSha256,
    copiedCorpusFiles: copyResult.copied,
    unchangedCorpusFiles: copyResult.unchanged,
  };
}

export function validateFuzzArtifactEntries(entries: string[], archivePath: string): void {
  for (const entry of entries) {
    const normalized = entry.replaceAll("\\", "/");
    const unsafe =
      normalized.startsWith("/") ||
      normalized === "." ||
      normalized.includes("../") ||
      normalized.startsWith("../") ||
      (normalized !== "fuzz/" &&
        normalized !== "fuzz/corpus/" &&
        normalized !== "fuzz/artifacts/" &&
        !normalized.startsWith("fuzz/corpus/") &&
        !normalized.startsWith("fuzz/artifacts/"));
    if (unsafe) {
      throw new Error(`unsafe tar entry in ${archivePath}: ${entry}`);
    }
  }
}

function buildImportPlan(artifactRoot: string, expectedCommit: string): ImportPlan {
  const files = listFiles(artifactRoot);
  const readinessSource = findReadinessArtifact(files, expectedCommit);
  const readiness = fuzzCleanRun(readJsonObject(readinessSource, "readiness artifact").fuzzCleanRun);
  validateReadiness(readiness, expectedCommit);
  const archives = findCorpusArchives(files, expectedCommit);
  validateArchiveCoverage(archives, readiness.targetRuns);
  return { readinessSource, readiness, archives };
}

function findReadinessArtifact(files: string[], expectedCommit: string): string {
  const expectedName = `fuzz-readiness-${expectedCommit}.json`;
  const matches = files.filter((path) => basename(path) === expectedName).sort(compareStrings);
  if (matches.length !== 1) {
    throw new Error(`expected exactly one ${expectedName}, found ${matches.length}`);
  }
  return matches[0];
}

function findCorpusArchives(files: string[], expectedCommit: string): FuzzArchive[] {
  const targets = expectedFuzzTargets();
  return files
    .filter((path) => basename(path).endsWith(tarGzipExtension))
    .flatMap((path) => archiveFromPath(path, expectedCommit, targets))
    .sort((left, right) => left.target.localeCompare(right.target) || left.shard - right.shard);
}

function archiveFromPath(path: string, expectedCommit: string, targets: string[]): FuzzArchive[] {
  const fileName = basename(path);
  for (const target of targets) {
    const prefix = `fuzz-corpus-${expectedCommit}-${target}-`;
    if (!fileName.startsWith(prefix) || !fileName.endsWith(tarGzipExtension)) {
      continue;
    }
    const shardText = fileName.slice(prefix.length, -tarGzipExtension.length);
    const shard = Number(shardText);
    if (!Number.isInteger(shard) || shard < 0) {
      throw new Error(`invalid fuzz corpus shard in ${fileName}`);
    }
    return [{ path, target, shard }];
  }
  return [];
}

function validateArchiveCoverage(archives: FuzzArchive[], targetRuns: FuzzTargetRun[]): void {
  for (const run of targetRuns) {
    for (let shard = 0; shard < run.shards; shard += 1) {
      const found = archives.some(
        (archive) => archive.target === run.target && archive.shard === shard,
      );
      if (!found) {
        throw new Error(`missing fuzz corpus archive for ${run.target} shard ${shard}`);
      }
    }
  }
}

function validateReadiness(readiness: FuzzCleanRun, expectedCommit: string): void {
  const failures: string[] = [];
  if (readiness.status !== "passed") {
    failures.push(`fuzzCleanRun.status must be passed, got ${readiness.status}`);
  }
  if (readiness.commit !== expectedCommit) {
    failures.push(`fuzzCleanRun.commit must match ${expectedCommit}, got ${readiness.commit}`);
  }
  if (!isExpectedActionsRunUrl(readiness.workflowRunUrl)) {
    failures.push("fuzzCleanRun.workflowRunUrl must be an actions run URL under the origin repo");
  }
  if (Date.parse(readiness.completedAt) <= Date.parse(readiness.startedAt)) {
    failures.push("fuzzCleanRun.completedAt must be after startedAt");
  }
  if (readiness.aggregateFuzzHours < REQUIRED_FUZZ_HOURS) {
    failures.push(`fuzzCleanRun.aggregateFuzzHours must be at least ${REQUIRED_FUZZ_HOURS}`);
  }
  const expectedTargets = expectedFuzzTargets();
  failures.push(...stringSetFailures(readiness.targets, expectedTargets, "fuzzCleanRun.targets"));
  failures.push(
    ...stringSetFailures(
      readiness.workflowMatrixTargets,
      workflowMatrixFuzzTargets(),
      "fuzzCleanRun.workflowMatrixTargets",
    ),
  );
  failures.push(...validateTargetRuns(readiness.targetRuns, expectedTargets));
  if (failures.length > 0) {
    throw new Error(`invalid fuzz readiness evidence:\n${failures.join("\n")}`);
  }
}

function validateTargetRuns(targetRuns: FuzzTargetRun[], expectedTargets: string[]): string[] {
  const workflowShardCount = workflowMatrixFuzzShardCount();
  const failures = stringSetFailures(
    targetRuns.map((run) => run.target),
    expectedTargets,
    "fuzzCleanRun.targetRuns",
  );
  for (const run of targetRuns) {
    if (run.status !== "passed") {
      failures.push(`fuzz target ${run.target} status must be passed, got ${run.status}`);
    }
    if (!Number.isInteger(run.shards) || run.shards <= 0) {
      failures.push(`fuzz target ${run.target} shards must be a positive integer`);
    }
    if (run.shards !== workflowShardCount) {
      failures.push(
        `fuzz target ${run.target} workflow shard count must be ${workflowShardCount}, got ${run.shards}`,
      );
    }
    if (run.secondsPerShard <= 0) {
      failures.push(`fuzz target ${run.target} secondsPerShard must be positive`);
    }
    if (run.fuzzHours <= 0) {
      failures.push(`fuzz target ${run.target} fuzzHours must be positive`);
    }
  }
  return failures;
}

function extractValidatedArchive(archivePath: string, destination: string): void {
  const entries = tarEntries(archivePath);
  validateFuzzArtifactEntries(entries, archivePath);
  runShellTar(["tar", "-xzf", "$1", "-C", "$2"], [archivePath, destination], archivePath);
}

function tarEntries(archivePath: string): string[] {
  const result = runShellTar(["tar", "-tzf", "$1"], [archivePath], archivePath);
  return result.split(/\r?\n/).filter(Boolean);
}

function runShellTar(commandParts: string[], positionalArgs: string[], archivePath: string): string {
  const result = spawnSync("bash", ["-lc", commandParts.join(" "), "bash", ...positionalArgs], {
    encoding: "utf8",
  });
  if (result.error) {
    throw new Error(`tar failed to start for ${archivePath}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `tar failed for ${archivePath}: status ${result.status}; stdout=${result.stdout}; stderr=${result.stderr}`,
    );
  }
  return result.stdout;
}

function copyCorpusFiles(input: {
  source: string;
  destination: string;
  dryRun: boolean;
  overwrite: boolean;
}): { copied: number; unchanged: number } {
  let copied = 0;
  let unchanged = 0;
  for (const sourcePath of listFiles(input.source)) {
    const relativePath = relative(input.source, sourcePath);
    const destinationPath = resolve(input.destination, relativePath);
    const sourceBytes = readFileSync(sourcePath);
    if (existsSync(destinationPath)) {
      const destinationBytes = readFileSync(destinationPath);
      if (Buffer.compare(sourceBytes, destinationBytes) === 0) {
        unchanged += 1;
        continue;
      }
      if (!input.overwrite) {
        throw new Error(`corpus file already exists with different content: ${destinationPath}`);
      }
    }
    copied += 1;
    if (!input.dryRun) {
      mkdirSync(dirname(destinationPath), { recursive: true });
      writeFileSync(destinationPath, sourceBytes);
    }
  }
  return { copied, unchanged };
}

function listFiles(root: string): string[] {
  if (!existsSync(root)) {
    throw new Error(`path does not exist: ${root}`);
  }
  const files: string[] = [];
  for (const entry of readdirSync(root)) {
    const path = join(root, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      files.push(...listFiles(path));
      continue;
    }
    if (stat.isFile()) {
      files.push(path);
    }
  }
  return files.sort(compareStrings);
}

function fuzzCleanRun(value: unknown): FuzzCleanRun {
  if (!isRecord(value)) {
    throw new Error("fuzzCleanRun must be an object");
  }
  return {
    status: stringField(value, "status"),
    workflowRunUrl: stringField(value, "workflowRunUrl"),
    commit: stringField(value, "commit"),
    startedAt: finiteIsoTimestamp(stringField(value, "startedAt"), "fuzzCleanRun.startedAt"),
    completedAt: finiteIsoTimestamp(stringField(value, "completedAt"), "fuzzCleanRun.completedAt"),
    aggregateFuzzHours: numberField(value, "aggregateFuzzHours"),
    targets: stringArrayField(value, "targets"),
    workflowMatrixTargets: stringArrayField(value, "workflowMatrixTargets"),
    targetRuns: targetRunsField(value),
  };
}

function readJsonObject(path: string, label: string): Record<string, unknown> {
  const parsed = JSON.parse(readFileSync(path, "utf8")) as unknown;
  if (!isRecord(parsed)) {
    throw new Error(`${label} must contain a JSON object`);
  }
  return parsed;
}

function targetRunsField(record: Record<string, unknown>): FuzzTargetRun[] {
  const value = record.targetRuns;
  if (!Array.isArray(value)) {
    throw new Error("fuzzCleanRun.targetRuns must be an array");
  }
  return value.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`fuzzCleanRun.targetRuns[${index}] must be an object`);
    }
    return {
      target: stringField(entry, "target"),
      status: stringField(entry, "status"),
      shards: numberField(entry, "shards"),
      secondsPerShard: numberField(entry, "secondsPerShard"),
      fuzzHours: numberField(entry, "fuzzHours"),
    };
  });
}

function numberField(record: Record<string, unknown>, field: string): number {
  const value = record[field];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${field} must be a finite number`);
  }
  return value;
}

function stringArrayField(record: Record<string, unknown>, field: string): string[] {
  const value = record[field];
  if (!Array.isArray(value) || !value.every((entry) => typeof entry === "string")) {
    throw new Error(`${field} must be a string array`);
  }
  return [...value].sort(compareStrings);
}

function parseCliOptions(args: string[]): CliOptions {
  const options: Partial<CliOptions> = {
    corpusOut: defaultCorpusOut,
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    if (arg === "--artifacts-dir") {
      options.artifactRoot = resolve(repoRoot, requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--commit") {
      options.expectedCommit = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--corpus-out") {
      options.corpusOut = resolve(repoRoot, requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--readiness-out") {
      options.readinessOut = resolve(repoRoot, requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--dry-run") {
      options.dryRun = true;
      continue;
    }
    if (arg === "--overwrite") {
      options.overwrite = true;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  if (!options.artifactRoot) {
    throw new Error("--artifacts-dir is required");
  }
  if (!options.expectedCommit) {
    throw new Error("--commit is required");
  }
  options.readinessOut ??= resolve(
    defaultReadinessDir,
    `fuzz-readiness-${options.expectedCommit}.json`,
  );
  return options as CliOptions;
}

function requiredArgValue(args: string[], index: number, flag: string): string {
  const value = args[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function printHelp(): void {
  console.log(`Import Seagrass fuzz workflow artifacts.

Usage:
  bun scripts/import-fuzz-artifacts.ts --artifacts-dir <path> --commit <sha> [options]

Options:
  --corpus-out <path>     Corpus destination. Default: fuzz/corpus.
  --readiness-out <path>  Fuzz readiness output. Default: target/fuzz-readiness-<sha>.json.
  --dry-run               Validate artifacts and report planned writes without writing.
  --overwrite             Allow replacing existing corpus files with different contents.

Examples:
  gh run download <run-id> --dir target/fuzz-artifacts
  bun scripts/import-fuzz-artifacts.ts --artifacts-dir target/fuzz-artifacts --commit <sha> --dry-run
  bun scripts/import-fuzz-artifacts.ts --artifacts-dir target/fuzz-artifacts --commit <sha>`);
}
