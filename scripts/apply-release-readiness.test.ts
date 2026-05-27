import { describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

import { applyReleaseReadiness, mergeReleaseReadiness } from "./apply-release-readiness.ts";
import { checkReleaseReadiness } from "./check-release-readiness.ts";
import {
  corpusTreeSha256,
  expectedFuzzTargets,
  expectedGithubRepoSlug,
  gitHead,
  REQUIRED_COMMUNITY_AUDIT_DAYS,
  REQUIRED_FUZZ_HOURS,
  workflowMatrixFuzzTargets,
} from "./release-evidence.ts";

const expectedRepoSlug = "heyAyushh/seagrass";
const releaseVersion = readFileSync(resolve(import.meta.dir, "../VERSION"), "utf8").trim();
const completedAt = "2026-05-26T12:00:00.000Z";
const fuzzStartedAt = "2026-05-25T12:00:00.000Z";
const auditOpenedAt = "2026-04-26T12:00:00.000Z";
const fuzzShardCount = 8;
const secondsPerFuzzShard = 3_600;

process.env.GITHUB_REPOSITORY = expectedRepoSlug;

describe("apply release readiness evidence", () => {
  test("merges generated fuzz and review evidence into a final release artifact", () => {
    const readiness = mergeReleaseReadiness({
      releaseVersion,
      fuzzPayload: fuzzPayload(),
      reviewPayload: reviewPayload(),
    });
    const readinessPath = writeJson("merged-release-readiness", readiness);

    const result = checkReleaseReadiness({
      allowPending: false,
      readinessPath,
      expectedVersion: releaseVersion,
      expectedCommit: gitHead(),
    });

    expect(result).toEqual({ status: "passed", failures: [] });
  });

  test("writes final evidence only after strict validation passes", () => {
    const dir = mkdtempSync(resolve(tmpdir(), "seagrass-apply-readiness-"));
    const fuzzPath = resolve(dir, "fuzz-readiness.json");
    const reviewPath = resolve(dir, "review-readiness.json");
    const outPath = resolve(dir, "release-readiness.json");
    writeFileSync(fuzzPath, `${JSON.stringify(fuzzPayload(), null, 2)}\n`);
    writeFileSync(reviewPath, `${JSON.stringify(reviewPayload(), null, 2)}\n`);

    applyReleaseReadiness({
      fuzzPath,
      reviewPath,
      outPath,
      releaseVersion,
      expectedCommit: gitHead(),
    });

    expect(JSON.parse(readFileSync(outPath, "utf8"))).toEqual(
      mergeReleaseReadiness({
        releaseVersion,
        fuzzPayload: fuzzPayload(),
        reviewPayload: reviewPayload(),
      }),
    );
  });

  test("refuses invalid evidence without writing the output file", () => {
    const dir = mkdtempSync(resolve(tmpdir(), "seagrass-apply-readiness-invalid-"));
    const fuzzPath = resolve(dir, "fuzz-readiness.json");
    const reviewPath = resolve(dir, "review-readiness.json");
    const outPath = resolve(dir, "release-readiness.json");
    writeFileSync(fuzzPath, `${JSON.stringify({ fuzzCleanRun: { status: "pending" } }, null, 2)}\n`);
    writeFileSync(reviewPath, `${JSON.stringify(reviewPayload(), null, 2)}\n`);

    expect(() =>
      applyReleaseReadiness({
        fuzzPath,
        reviewPath,
        outPath,
        releaseVersion,
        expectedCommit: gitHead(),
      }),
    ).toThrow(/release readiness evidence failed strict validation/);
    expect(existsSync(outPath)).toBe(false);
  });
});

function fuzzPayload(): { fuzzCleanRun: unknown } {
  const targets = expectedFuzzTargets();
  return {
    fuzzCleanRun: {
      status: "passed",
      workflowRunUrl: `${repoBaseUrl()}/actions/runs/1`,
      commit: gitHead(),
      startedAt: fuzzStartedAt,
      completedAt,
      aggregateFuzzHours: REQUIRED_FUZZ_HOURS,
      corpusSha256: corpusTreeSha256(),
      targets,
      workflowMatrixTargets: workflowMatrixFuzzTargets(),
      targetRuns: targets.map((target) => ({
        target,
        status: "passed",
        shards: fuzzShardCount,
        secondsPerShard: secondsPerFuzzShard,
        fuzzHours: fuzzShardCount,
      })),
    },
  };
}

function reviewPayload(): { externalReview: unknown } {
  return {
    externalReview: {
      status: "signed-off",
      mode: "community",
      reviewer: "External Reviewer",
      artifactUrl: `${repoBaseUrl()}/pull/1#issuecomment-1`,
      completedAt,
      auditOpenedAt,
      auditWindowDays: REQUIRED_COMMUNITY_AUDIT_DAYS,
      announcementUrl: `${repoBaseUrl()}/issues/1`,
      signoffs: [
        {
          reviewer: "External Reviewer",
          artifactUrl: `${repoBaseUrl()}/pull/1#issuecomment-1`,
          signedOffAt: completedAt,
        },
      ],
      findingsDisposition: {
        status: "dispositioned",
        changelogPath: "CHANGELOG.md",
        artifactUrl: `${repoBaseUrl()}/blob/v${releaseVersion}/CHANGELOG.md`,
      },
    },
  };
}

function writeJson(name: string, value: unknown): string {
  const dir = mkdtempSync(resolve(tmpdir(), `seagrass-${name}-`));
  const path = resolve(dir, `${name}.json`);
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
  return path;
}

function repoBaseUrl(): string {
  return `https://github.com/${expectedGithubRepoSlug()}`;
}
