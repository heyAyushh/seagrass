#!/usr/bin/env bun

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { requiredArgValue } from "./cli-args.ts";
import {
  finiteNumberField as numberField,
  optionalFiniteNumberField as optionalNumberField,
  optionalStringField,
  stringArrayField,
} from "./json-utils.ts";
import {
  compareStrings,
  corpusTreeSha256,
  elapsedDays,
  expectedFuzzTargets,
  expectedGithubRepoSlug,
  fuzzHours,
  gitHead,
  isExpectedActionsRunUrl,
  isExpectedChangelogProofUrl,
  isExpectedReviewProofUrl,
  isFiniteDate,
  isRecord,
  repoRoot,
  REQUIRED_COMMUNITY_AUDIT_DAYS,
  REQUIRED_FUZZ_HOURS,
  stringField,
  stringSetFailures,
  workflowMatrixFuzzShardCount,
  workflowMatrixFuzzTargets,
} from "./release-evidence.ts";

const defaultReadinessPath = resolve(repoRoot, "docs/release-readiness.json");
const FUZZ_HOURS_TOLERANCE = 0.01;
const PENDING_PREFIX = "pending:";
const RESERVED_EVIDENCE_SENTINELS = new Set(["pending"]);

type ReviewMode = "paid" | "community";

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

type FuzzTargetRun = {
  target: string;
  status: string;
  shards: number;
  secondsPerShard: number;
  fuzzHours: number;
};

type ExternalReview = {
  status: string;
  mode: ReviewMode;
  reviewer: string;
  artifactUrl: string;
  completedAt: string;
  auditOpenedAt?: string;
  auditWindowDays?: number;
  announcementUrl?: string;
  signoffs?: ReviewSignoff[];
  findingsDisposition?: FindingsDisposition;
};

type ReviewSignoff = {
  reviewer: string;
  artifactUrl: string;
  signedOffAt: string;
};

type FindingsDisposition = {
  status: string;
  changelogPath: string;
  artifactUrl: string;
};

type ReleaseReadiness = {
  releaseVersion: string;
  fuzzCleanRun: FuzzCleanRun;
  externalReview: ExternalReview;
};

export type ReleaseCheckOptions = {
  allowPending: boolean;
  readinessPath: string;
  expectedVersion?: string;
  expectedCommit?: string;
};

export type ReleaseCheckResult = {
  status: "passed" | "pending" | "failed";
  failures: string[];
};

if (import.meta.main) {
  const result = checkReleaseReadiness(parseCliOptions(process.argv.slice(2)));
  if (result.status === "pending") {
    console.log("seagrass release readiness shape check passed: pending evidence accepted");
    process.exit(0);
  }
  if (result.status === "failed") {
    console.error(`\nseagrass release readiness check failed:\n${result.failures.join("\n")}`);
    process.exit(1);
  }
  console.log("seagrass release readiness check passed");
}

export function checkReleaseReadiness(options: ReleaseCheckOptions): ReleaseCheckResult {
  try {
    const readiness = parseReadiness(options.readinessPath);
    const failures = [
      ...validateVersion(readiness.releaseVersion, options.expectedVersion),
      ...validateFuzzCleanRun(readiness.fuzzCleanRun, options),
      ...validateExternalReview(readiness.externalReview),
    ];
    if (options.allowPending && pendingOnly(failures)) {
      return { status: "pending", failures };
    }
    if (failures.length > 0) {
      return { status: "failed", failures };
    }
    return { status: "passed", failures };
  } catch (error) {
    return { status: "failed", failures: [errorMessage(error)] };
  }
}

function parseCliOptions(args: string[]): ReleaseCheckOptions {
  const options: ReleaseCheckOptions = { allowPending: false, readinessPath: defaultReadinessPath };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--allow-pending") {
      options.allowPending = true;
      continue;
    }
    if (arg === "--version") {
      options.expectedVersion = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--commit") {
      options.expectedCommit = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--readiness-path") {
      options.readinessPath = resolve(repoRoot, requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function parseReadiness(path: string): ReleaseReadiness {
  const parsed = JSON.parse(readFileSync(path, "utf8")) as unknown;
  if (!isRecord(parsed)) {
    throw new Error(`${path} must contain a JSON object`);
  }

  return {
    releaseVersion: stringField(parsed, "releaseVersion"),
    fuzzCleanRun: fuzzCleanRun(parsed.fuzzCleanRun),
    externalReview: externalReview(parsed.externalReview),
  };
}

function fuzzCleanRun(value: unknown): FuzzCleanRun {
  if (!isRecord(value)) {
    throw new Error("fuzzCleanRun must be an object");
  }
  return {
    status: stringField(value, "status"),
    workflowRunUrl: stringField(value, "workflowRunUrl"),
    commit: stringField(value, "commit"),
    startedAt: stringField(value, "startedAt"),
    completedAt: stringField(value, "completedAt"),
    aggregateFuzzHours: numberField(value, "aggregateFuzzHours"),
    ...optionalCorpusSha256(value),
    targets: stringArrayField(value, "targets"),
    workflowMatrixTargets: stringArrayField(value, "workflowMatrixTargets"),
    targetRuns: targetRunsField(value),
  };
}

function externalReview(value: unknown): ExternalReview {
  if (!isRecord(value)) {
    throw new Error("externalReview must be an object");
  }
  const mode = stringField(value, "mode");
  if (mode !== "paid" && mode !== "community") {
    throw new Error("externalReview.mode must be paid or community");
  }
  const auditWindowDays = optionalNumberField(value, "auditWindowDays");
  const auditOpenedAt = optionalStringField(value, "auditOpenedAt");
  const announcementUrl = optionalStringField(value, "announcementUrl");
  return {
    status: stringField(value, "status"),
    mode,
    reviewer: stringField(value, "reviewer"),
    artifactUrl: stringField(value, "artifactUrl"),
    completedAt: stringField(value, "completedAt"),
    ...(auditOpenedAt === undefined ? {} : { auditOpenedAt }),
    ...(auditWindowDays === undefined ? {} : { auditWindowDays }),
    ...(announcementUrl === undefined ? {} : { announcementUrl }),
    ...optionalReviewSignoffs(value),
    ...optionalFindingsDisposition(value),
  };
}

function validateVersion(actual: string, expected: string | undefined): string[] {
  if (!expected || actual === expected) {
    return [];
  }
  return [`releaseVersion must match ${expected}, got ${actual}`];
}

function validateFuzzCleanRun(cleanRun: FuzzCleanRun, options: ReleaseCheckOptions): string[] {
  const failures: string[] = [];
  const pending = cleanRun.status !== "passed";
  if (cleanRun.status !== "passed") {
    failures.push(`pending:fuzzCleanRun.status must be passed, got ${cleanRun.status}`);
  }
  if (!isExpectedActionsRunUrl(cleanRun.workflowRunUrl)) {
    failures.push(
      pendingFailure(
        `fuzzCleanRun.workflowRunUrl must be a concrete actions run URL under github.com/${expectedGithubRepoSlug()}`,
        pending,
      ),
    );
  }
  const expectedCommit = options.expectedCommit ?? gitHead();
  if (cleanRun.commit !== expectedCommit) {
    failures.push(
      pendingFailure(
        `fuzzCleanRun.commit must match ${expectedCommit}, got ${cleanRun.commit}`,
        pending,
      ),
    );
  }
  if (cleanRun.aggregateFuzzHours < REQUIRED_FUZZ_HOURS) {
    failures.push(
      pendingFailure(
        `fuzzCleanRun.aggregateFuzzHours must be at least ${REQUIRED_FUZZ_HOURS}`,
        pending,
      ),
    );
  }
  failures.push(...validateFuzzWorkflowTimestamps(cleanRun, pending));
  if (cleanRun.targets.length === 0) {
    failures.push("fuzzCleanRun.targets must list fuzzed targets");
  }
  const expectedTargets = expectedFuzzTargets();
  failures.push(...stringSetFailures(cleanRun.targets, expectedTargets, "fuzzCleanRun.targets"));
  failures.push(
    ...stringSetFailures(
      cleanRun.workflowMatrixTargets,
      workflowMatrixFuzzTargets(),
      "fuzzCleanRun.workflowMatrixTargets",
    ),
  );
  failures.push(
    ...stringSetFailures(
      cleanRun.workflowMatrixTargets,
      expectedTargets,
      "fuzzCleanRun.workflowMatrixTargets",
    ),
  );
  failures.push(...validateFuzzTargetRuns(cleanRun, expectedTargets, pending));
  failures.push(...validateFuzzCorpusHash(cleanRun));
  return failures;
}

function validateFuzzWorkflowTimestamps(cleanRun: FuzzCleanRun, pending: boolean): string[] {
  if (!isFiniteDate(cleanRun.startedAt) || !isFiniteDate(cleanRun.completedAt)) {
    return [
      pendingFailure("fuzzCleanRun.startedAt/completedAt must be ISO timestamps", pending),
    ];
  }
  if (Date.parse(cleanRun.completedAt) <= Date.parse(cleanRun.startedAt)) {
    return ["fuzzCleanRun.completedAt must be after startedAt"];
  }
  return [];
}

function validateFuzzCorpusHash(cleanRun: FuzzCleanRun): string[] {
  if (cleanRun.status !== "passed") {
    return [];
  }
  if (!cleanRun.corpusSha256) {
    return ["fuzzCleanRun.corpusSha256 must bind final evidence to fuzz/corpus"];
  }
  if (!/^[a-f0-9]{64}$/i.test(cleanRun.corpusSha256)) {
    return ["fuzzCleanRun.corpusSha256 must be a SHA-256 hex digest"];
  }
  const actual = corpusTreeSha256();
  if (cleanRun.corpusSha256.toLowerCase() !== actual) {
    return [`fuzzCleanRun.corpusSha256 must match fuzz/corpus, got ${cleanRun.corpusSha256}`];
  }
  return [];
}

function validateFuzzTargetRuns(
  cleanRun: FuzzCleanRun,
  expectedTargets: string[],
  pending: boolean,
): string[] {
  const failures: string[] = [];
  const workflowShardCount = workflowMatrixFuzzShardCount();
  const runTargets = cleanRun.targetRuns.map((run) => run.target).sort(compareStrings);
  failures.push(...stringSetFailures(runTargets, expectedTargets, "fuzzCleanRun.targetRuns"));
  const targetRunHours = cleanRun.targetRuns.reduce((sum, run) => sum + run.fuzzHours, 0);
  if (Math.abs(targetRunHours - cleanRun.aggregateFuzzHours) > FUZZ_HOURS_TOLERANCE) {
    failures.push(
      `fuzzCleanRun.targetRuns hours (${targetRunHours}) must match aggregateFuzzHours (${cleanRun.aggregateFuzzHours})`,
    );
  }
  for (const run of cleanRun.targetRuns) {
    if (run.status !== "passed") {
      failures.push(
        pendingFailure(
          `fuzz target ${run.target} status must be passed, got ${run.status}`,
          pending,
        ),
      );
    }
    if (run.shards <= 0) {
      failures.push(`fuzz target ${run.target} shards must be positive`);
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
      failures.push(
        pendingFailure(`fuzz target ${run.target} fuzzHours must be positive`, pending),
      );
    }
    const expectedRunHours = fuzzHours(run.shards, run.secondsPerShard);
    if (Math.abs(run.fuzzHours - expectedRunHours) > FUZZ_HOURS_TOLERANCE) {
      failures.push(
        pendingFailure(
          `fuzz target ${run.target} fuzzHours must match shards and secondsPerShard`,
          pending,
        ),
      );
    }
  }
  return failures;
}

function validateExternalReview(review: ExternalReview): string[] {
  const failures: string[] = [];
  const pending = review.status !== "signed-off";
  if (review.status !== "signed-off") {
    failures.push(`pending:externalReview.status must be signed-off, got ${review.status}`);
  }
  if (!isConcreteEvidenceText(review.reviewer)) {
    failures.push(pendingFailure("externalReview.reviewer must name a reviewer", pending));
  }
  if (!isExpectedReviewProofUrl(review.artifactUrl)) {
    failures.push(
      pendingFailure(
        `externalReview.artifactUrl must be a concrete review proof URL under github.com/${expectedGithubRepoSlug()}`,
        pending,
      ),
    );
  }
  if (!isFiniteDate(review.completedAt)) {
    failures.push(pendingFailure("externalReview.completedAt must be an ISO timestamp", pending));
  }
  if (review.mode === "community") {
    failures.push(...validateCommunityReviewWindow(review, pending));
  }
  if (review.status === "signed-off") {
    failures.push(...validateReviewSignoffs(review));
    failures.push(...validateFindingsDisposition(review));
  }
  return failures;
}

function validateReviewSignoffs(review: ExternalReview): string[] {
  const failures: string[] = [];
  if (!review.signoffs || review.signoffs.length === 0) {
    failures.push("externalReview.signoffs must include at least one reviewer signoff");
    return failures;
  }
  for (const [index, signoff] of review.signoffs.entries()) {
    if (!isConcreteEvidenceText(signoff.reviewer)) {
      failures.push(`externalReview.signoffs[${index}].reviewer must name a reviewer`);
    }
    if (!isExpectedReviewProofUrl(signoff.artifactUrl)) {
      failures.push(
        `externalReview.signoffs[${index}].artifactUrl must be a concrete review proof URL under github.com/${expectedGithubRepoSlug()}`,
      );
    }
    if (!isFiniteDate(signoff.signedOffAt)) {
      failures.push(`externalReview.signoffs[${index}].signedOffAt must be an ISO timestamp`);
    }
  }
  return failures;
}

function validateFindingsDisposition(review: ExternalReview): string[] {
  const disposition = review.findingsDisposition;
  if (!disposition) {
    return ["externalReview.findingsDisposition must record findings disposition"];
  }
  const failures: string[] = [];
  if (disposition.status !== "dispositioned") {
    failures.push(
      `externalReview.findingsDisposition.status must be dispositioned, got ${disposition.status}`,
    );
  }
  if (
    disposition.changelogPath !== "CHANGELOG.md" &&
    disposition.changelogPath !== "CHANGELOG.md"
  ) {
    failures.push("externalReview.findingsDisposition.changelogPath must point to CHANGELOG.md");
  }
  if (!isExpectedChangelogProofUrl(disposition.artifactUrl)) {
    failures.push(
      `externalReview.findingsDisposition.artifactUrl must point to an immutable CHANGELOG.md proof under github.com/${expectedGithubRepoSlug()}`,
    );
  }
  return failures;
}

function validateCommunityReviewWindow(review: ExternalReview, pending: boolean): string[] {
  const failures: string[] = [];
  if (!review.auditOpenedAt) {
    failures.push("pending:externalReview.auditOpenedAt is required for community review");
    return failures;
  }
  if (!isFiniteDate(review.auditOpenedAt)) {
    failures.push(
      pendingFailure("externalReview.auditOpenedAt must be an ISO timestamp", pending),
    );
    return failures;
  }
  const actualAuditWindowDays = elapsedDays(review.auditOpenedAt, review.completedAt);
  if (actualAuditWindowDays < REQUIRED_COMMUNITY_AUDIT_DAYS) {
    failures.push(
      `pending:community review requires at least ${REQUIRED_COMMUNITY_AUDIT_DAYS} elapsed days`,
    );
  }
  if (review.auditWindowDays !== actualAuditWindowDays) {
    failures.push("externalReview.auditWindowDays must match auditOpenedAt/completedAt");
  }
  if (!review.announcementUrl) {
    failures.push("pending:externalReview.announcementUrl is required for community review");
  } else if (!isExpectedReviewProofUrl(review.announcementUrl)) {
    failures.push(
      pendingFailure(
        `externalReview.announcementUrl must be a concrete review proof URL under github.com/${expectedGithubRepoSlug()}`,
        pending,
      ),
    );
  }
  return failures;
}

function pendingOnly(failures: string[]): boolean {
  return failures.length > 0 && failures.every((failure) => failure.startsWith(PENDING_PREFIX));
}

function pendingFailure(message: string, pending: boolean): string {
  return pending ? `${PENDING_PREFIX}${message}` : message;
}

function isConcreteEvidenceText(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return normalized !== "" && !RESERVED_EVIDENCE_SENTINELS.has(normalized);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function optionalCorpusSha256(record: Record<string, unknown>): Pick<FuzzCleanRun, "corpusSha256"> {
  const value = optionalStringField(record, "corpusSha256");
  return value === undefined ? {} : { corpusSha256: value };
}

function targetRunsField(record: Record<string, unknown>): FuzzTargetRun[] {
  const value = record.targetRuns;
  if (!Array.isArray(value)) {
    throw new Error("targetRuns must be an array");
  }
  return value.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`targetRuns[${index}] must be an object`);
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

function optionalReviewSignoffs(record: Record<string, unknown>): { signoffs?: ReviewSignoff[] } {
  const value = record.signoffs;
  if (value === undefined) {
    return {};
  }
  if (!Array.isArray(value)) {
    throw new Error("signoffs must be an array when present");
  }
  return {
    signoffs: value.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(`signoffs[${index}] must be an object`);
      }
      return {
        reviewer: stringField(entry, "reviewer"),
        artifactUrl: stringField(entry, "artifactUrl"),
        signedOffAt: stringField(entry, "signedOffAt"),
      };
    }),
  };
}

function optionalFindingsDisposition(
  record: Record<string, unknown>,
): { findingsDisposition?: FindingsDisposition } {
  const value = record.findingsDisposition;
  if (value === undefined) {
    return {};
  }
  if (!isRecord(value)) {
    throw new Error("findingsDisposition must be an object when present");
  }
  return {
    findingsDisposition: {
      status: stringField(value, "status"),
      changelogPath: stringField(value, "changelogPath"),
      artifactUrl: stringField(value, "artifactUrl"),
    },
  };
}
