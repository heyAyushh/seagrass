#!/usr/bin/env bun

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import {
  elapsedDays,
  expectedGithubRepoSlug,
  finiteIsoTimestamp,
  isExpectedChangelogProofUrl,
  isExpectedReviewProofUrl,
  repoRoot,
  REQUIRED_COMMUNITY_AUDIT_DAYS,
} from "./release-evidence.ts";

type ReviewMode = "paid" | "community";
const RESERVED_EVIDENCE_SENTINELS = new Set(["pending"]);

type ReviewSignoff = {
  reviewer: string;
  artifactUrl: string;
  signedOffAt: string;
};

type FindingsDisposition = {
  status: "dispositioned";
  changelogPath: string;
  artifactUrl: string;
};

type ExternalReviewEvidence = {
  status: "signed-off";
  mode: ReviewMode;
  reviewer: string;
  artifactUrl: string;
  completedAt: string;
  auditOpenedAt?: string;
  auditWindowDays?: number;
  announcementUrl?: string;
  signoffs: ReviewSignoff[];
  findingsDisposition: FindingsDisposition;
};

type CliOptions = {
  mode: ReviewMode;
  reviewer: string;
  artifactUrl: string;
  completedAt: string;
  auditOpenedAt?: string;
  announcementUrl?: string;
  findingsArtifactUrl: string;
  changelogPath: string;
  out?: string;
};

if (import.meta.main) {
  const options = parseCliOptions(process.argv.slice(2));
  const evidence = buildReviewEvidence(options);
  const payload = `${JSON.stringify({ externalReview: evidence }, null, 2)}\n`;
  if (options.out) {
    const outPath = resolve(repoRoot, options.out);
    mkdirSync(dirname(outPath), { recursive: true });
    writeFileSync(outPath, payload);
  } else {
    process.stdout.write(payload);
  }
}

export function buildReviewEvidence(input: {
  mode: ReviewMode;
  reviewer: string;
  artifactUrl: string;
  completedAt: string;
  auditOpenedAt?: string;
  announcementUrl?: string;
  findingsArtifactUrl: string;
  changelogPath?: string;
}): ExternalReviewEvidence {
  const completedAt = finiteIsoTimestamp(input.completedAt, "--completed-at");
  const reviewer = nonEmpty(input.reviewer, "--reviewer");
  const artifactUrl = expectedReviewProofUrl(input.artifactUrl, "--artifact-url");
  const findingsArtifactUrl = expectedChangelogProofUrl(
    input.findingsArtifactUrl,
    "--findings-artifact-url",
  );
  const changelogPath = input.changelogPath ?? "CHANGELOG.md";
  const communityFields = communityReviewFields(input, completedAt);

  return {
    status: "signed-off",
    mode: input.mode,
    reviewer,
    artifactUrl,
    completedAt,
    ...communityFields,
    signoffs: [
      {
        reviewer,
        artifactUrl,
        signedOffAt: completedAt,
      },
    ],
    findingsDisposition: {
      status: "dispositioned",
      changelogPath,
      artifactUrl: findingsArtifactUrl,
    },
  };
}

function parseCliOptions(args: string[]): CliOptions {
  const options: Partial<CliOptions> = {
    completedAt: new Date().toISOString(),
    changelogPath: "CHANGELOG.md",
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    if (arg === "--mode") {
      options.mode = parseMode(requiredArgValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--reviewer") {
      options.reviewer = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--artifact-url") {
      options.artifactUrl = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--completed-at") {
      options.completedAt = finiteIsoTimestamp(requiredArgValue(args, index, arg), arg);
      index += 1;
      continue;
    }
    if (arg === "--audit-opened-at") {
      options.auditOpenedAt = finiteIsoTimestamp(requiredArgValue(args, index, arg), arg);
      index += 1;
      continue;
    }
    if (arg === "--announcement-url") {
      options.announcementUrl = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--findings-artifact-url") {
      options.findingsArtifactUrl = requiredArgValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--changelog-path") {
      options.changelogPath = requiredArgValue(args, index, arg);
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

  if (!options.mode) {
    throw new Error("--mode is required");
  }
  if (!options.reviewer) {
    throw new Error("--reviewer is required");
  }
  if (!options.artifactUrl) {
    throw new Error("--artifact-url is required");
  }
  if (!options.findingsArtifactUrl) {
    throw new Error("--findings-artifact-url is required");
  }

  return options as CliOptions;
}

function requiredArgValue(args: string[], index: number, flag: string): string {
  const value = args[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function parseMode(value: string): ReviewMode {
  if (value === "paid" || value === "community") {
    return value;
  }
  throw new Error("--mode must be paid or community");
}

function nonEmpty(value: string, field: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${field} must not be empty`);
  }
  if (RESERVED_EVIDENCE_SENTINELS.has(trimmed.toLowerCase())) {
    throw new Error(`${field} must name a reviewer`);
  }
  return trimmed;
}

function communityReviewFields(
  input: {
    mode: ReviewMode;
    auditOpenedAt?: string;
    announcementUrl?: string;
  },
  completedAt: string,
): Pick<ExternalReviewEvidence, "auditOpenedAt" | "auditWindowDays" | "announcementUrl"> {
  if (input.mode !== "community") {
    return {};
  }
  if (!input.auditOpenedAt) {
    throw new Error("--audit-opened-at is required for community review");
  }
  if (!input.announcementUrl) {
    throw new Error("--announcement-url is required for community review");
  }
  const auditOpenedAt = finiteIsoTimestamp(input.auditOpenedAt, "--audit-opened-at");
  const auditWindowDays = elapsedDays(auditOpenedAt, completedAt);
  if (auditWindowDays < REQUIRED_COMMUNITY_AUDIT_DAYS) {
    throw new Error(`community review must be open for at least ${REQUIRED_COMMUNITY_AUDIT_DAYS} days`);
  }
  return {
    auditOpenedAt,
    auditWindowDays,
    announcementUrl: expectedReviewProofUrl(input.announcementUrl, "--announcement-url"),
  };
}

function expectedReviewProofUrl(value: string, field: string): string {
  if (isExpectedReviewProofUrl(value)) {
    return value;
  }
  throw new Error(
    `${field} must be a concrete review proof URL under github.com/${expectedGithubRepoSlug()}`,
  );
}

function expectedChangelogProofUrl(value: string, field: string): string {
  if (isExpectedChangelogProofUrl(value)) {
    return value;
  }
  throw new Error(
    `${field} must point to an immutable CHANGELOG.md proof under github.com/${expectedGithubRepoSlug()}`,
  );
}

function printHelp(): void {
  console.log(`Generate Seagrass external review readiness evidence.

Usage:
  bun scripts/review-readiness.ts --mode <paid|community> --reviewer <name> --artifact-url <url> --findings-artifact-url <url> [options]

Options:
  --completed-at <iso>          Review completion timestamp. Default: now.
  --audit-opened-at <iso>       Required for community review.
  --announcement-url <url>      Required for community review.
  --changelog-path <path>       Findings disposition changelog path. Default: CHANGELOG.md.
  --out <path>                  Write JSON to this path instead of stdout.`);
}
