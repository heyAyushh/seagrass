import { describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { checkReleaseReadiness } from "./check-release-readiness.ts";
import { corpusTreeSha256, expectedGithubRepoSlug } from "./release-evidence.ts";

const expectedRepoSlug = "heyAyushh/seagrass";

process.env.GITHUB_REPOSITORY = expectedRepoSlug;

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const releaseVersion = readFileSync(resolve(repoRoot, "VERSION"), "utf8").trim();

describe("release readiness checker", () => {
  test("accepts complete fuzz and external-review evidence", () => {
    const readinessPath = writeReadiness("complete", completeReadiness());
    const result = runChecker(readinessPath);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("seagrass release readiness check passed");
  });

  test("accepts explicit pending placeholders only with allow-pending", () => {
    const readinessPath = writeReadiness("pending", pendingReadiness());
    const pending = checkReleaseReadiness({
      allowPending: true,
      readinessPath,
      expectedVersion: releaseVersion,
      expectedCommit: gitHead(),
    });
    const strict = checkReleaseReadiness({
      allowPending: false,
      readinessPath,
      expectedVersion: releaseVersion,
      expectedCommit: gitHead(),
    });

    expect(pending.status).toBe("pending");
    expect(pending.failures.length).toBeGreaterThan(0);
    expect(pending.failures.every((failure) => failure.startsWith("pending:"))).toBe(true);
    expect(strict.status).toBe("failed");
  });

  test("checked-in pending evidence is visibly non-final", () => {
    const readiness = JSON.parse(
      readFileSync(resolve(repoRoot, "docs/release-readiness.json"), "utf8"),
    ) as ReleaseReadiness;

    expect(readiness.fuzzCleanRun.status).toBe("pending");
    expect(readiness.fuzzCleanRun.workflowRunUrl).toBe("pending");
    expect(readiness.fuzzCleanRun.commit).toBe("pending");
    expect(readiness.fuzzCleanRun.startedAt).toBe("pending");
    expect(readiness.fuzzCleanRun.completedAt).toBe("pending");
    expect(readiness.fuzzCleanRun.aggregateFuzzHours).toBe(0);
    expect(readiness.fuzzCleanRun.targetRuns?.every((run) => run.status === "pending")).toBe(true);
    expect(readiness.fuzzCleanRun.targetRuns?.every((run) => run.fuzzHours === 0)).toBe(true);
    expect(readiness.externalReview.status).toBe("pending");
    expect(readiness.externalReview.reviewer).toBe("pending");
    expect(readiness.externalReview.artifactUrl).toBe("pending");
    expect(readiness.externalReview.completedAt).toBe("pending");
    expect(readiness.externalReview.auditOpenedAt).toBe("pending");
    expect(readiness.externalReview.auditWindowDays).toBe(0);
    expect(readiness.externalReview.announcementUrl).toBe("pending");
  });

  test("rejects signed-off external review without explicit signoff rows", () => {
    const readiness = completeReadiness();
    delete readiness.externalReview.signoffs;
    const readinessPath = writeReadiness("missing-signoffs", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("externalReview.signoffs");
  });

  test("rejects signed-off external review without findings disposition", () => {
    const readiness = completeReadiness();
    delete readiness.externalReview.findingsDisposition;
    const readinessPath = writeReadiness("missing-disposition", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("findingsDisposition");
  });

  test("rejects fuzz evidence without explicit target runs", () => {
    const readiness = completeReadiness();
    delete readiness.fuzzCleanRun.targetRuns;
    const readinessPath = writeReadiness("missing-target-runs", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("targetRuns");
  });

  test("rejects stale workflow matrix evidence", () => {
    const readiness = completeReadiness();
    readiness.fuzzCleanRun.workflowMatrixTargets = ["fuzz_anchor_attr", "fuzz_document_parse"];
    const readinessPath = writeReadiness("stale-workflow-targets", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("workflowMatrixTargets");
  });

  test("rejects target run shard counts that drift from the workflow matrix", () => {
    const readiness = completeReadiness();
    for (const run of readiness.fuzzCleanRun.targetRuns ?? []) {
      run.shards = 7;
      run.secondsPerShard = 28_800 / 7;
      run.fuzzHours = 8;
    }
    const readinessPath = writeReadiness("stale-workflow-shards", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("workflow shard count");
  });

  test("accepts parallel fuzz evidence with enough aggregate hours", () => {
    const readiness = completeReadiness();
    readiness.fuzzCleanRun.startedAt = "2026-05-26T11:00:00.000Z";
    const readinessPath = writeReadiness("parallel-fuzz-window", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(0);
  });

  test("rejects fuzz evidence with a non-positive workflow window", () => {
    const readiness = completeReadiness();
    readiness.fuzzCleanRun.startedAt = readiness.fuzzCleanRun.completedAt;
    const readinessPath = writeReadiness("zero-fuzz-window", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("completedAt must be after startedAt");
  });

  test("rejects fuzz evidence with a stale corpus hash", () => {
    const readiness = completeReadiness();
    readiness.fuzzCleanRun.corpusSha256 = "0".repeat(64);
    const readinessPath = writeReadiness("stale-corpus-hash", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("fuzzCleanRun.corpusSha256");
  });

  test("rejects community review without a dated audit window", () => {
    const readiness = completeReadiness();
    delete readiness.externalReview.auditOpenedAt;
    const readinessPath = writeReadiness("missing-audit-opened", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("auditOpenedAt");
  });

  test("rejects evidence URLs outside the origin repo", () => {
    const readiness = completeReadiness();
    readiness.fuzzCleanRun.workflowRunUrl = "https://github.com/example/other/actions/runs/1";
    readiness.externalReview.artifactUrl = "https://github.com/example/other/pull/1";
    const readinessPath = writeReadiness("wrong-repo-url", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain(`github.com/${expectedGithubRepoSlug()}`);
  });

  test("rejects placeholder fuzz workflow run ids", () => {
    const readiness = completeReadiness();
    readiness.fuzzCleanRun.workflowRunUrl = `https://github.com/${expectedGithubRepoSlug()}/actions/runs/0`;
    const readinessPath = writeReadiness("placeholder-fuzz-run", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("actions run URL");
  });

  test("rejects generic review proof URLs", () => {
    const readiness = completeReadiness();
    const repoBaseUrl = `https://github.com/${expectedGithubRepoSlug()}`;
    readiness.externalReview.artifactUrl = `${repoBaseUrl}/pulls`;
    readiness.externalReview.announcementUrl = `${repoBaseUrl}/issues/0`;
    readiness.externalReview.signoffs = [
      {
        reviewer: "External Reviewer",
        artifactUrl: `${repoBaseUrl}/issues/0`,
        signedOffAt: "2026-05-26T12:00:00.000Z",
      },
    ];
    readiness.externalReview.findingsDisposition = {
      status: "dispositioned",
      changelogPath: "CHANGELOG.md",
      artifactUrl: `${repoBaseUrl}/pulls`,
    };
    const readinessPath = writeReadiness("generic-review-urls", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("externalReview.artifactUrl");
    expect(result.stderr).toContain("externalReview.signoffs[0].artifactUrl");
    expect(result.stderr).toContain("externalReview.findingsDisposition.artifactUrl");
    expect(result.stderr).toContain("externalReview.announcementUrl");
  });

  test("rejects mutable changelog proof URLs", () => {
    const readiness = completeReadiness();
    const repoBaseUrl = `https://github.com/${expectedGithubRepoSlug()}`;
    readiness.externalReview.findingsDisposition = {
      status: "dispositioned",
      changelogPath: "CHANGELOG.md",
      artifactUrl: `${repoBaseUrl}/blob/main/CHANGELOG.md`,
    };
    const readinessPath = writeReadiness("mutable-changelog-proof", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("immutable CHANGELOG.md proof");
  });

  test("rejects final review evidence that reuses pending sentinels", () => {
    const readiness = completeReadiness();
    readiness.externalReview.reviewer = "pending";
    if (readiness.externalReview.signoffs) {
      readiness.externalReview.signoffs[0].reviewer = "pending";
    }
    const readinessPath = writeReadiness("pending-reviewer-name", readiness);
    const result = runChecker(readinessPath);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("externalReview.reviewer must name a reviewer");
    expect(result.stderr).toContain("externalReview.signoffs[0].reviewer must name a reviewer");
  });
});

function runChecker(readinessPath: string): { status: number | null; stdout: string; stderr: string } {
  const result = checkReleaseReadiness({
    allowPending: false,
    readinessPath,
    expectedVersion: releaseVersion,
    expectedCommit: gitHead(),
  });
  return {
    status: result.status === "passed" ? 0 : 1,
    stdout:
      result.status === "passed" ? "seagrass release readiness check passed" : result.status,
    stderr: result.failures.join("\n"),
  };
}

function writeReadiness(name: string, readiness: ReleaseReadiness): string {
  const dir = mkdtempSync(resolve(tmpdir(), `seagrass-${name}-`));
  const path = resolve(dir, "release-readiness.json");
  writeFileSync(path, `${JSON.stringify(readiness, null, 2)}\n`);
  return path;
}

function completeReadiness(): ReleaseReadiness {
  const completedAt = "2026-05-26T12:00:00.000Z";
  const auditOpenedAt = "2026-04-26T12:00:00.000Z";
  const repoBaseUrl = `https://github.com/${expectedGithubRepoSlug()}`;
  return {
    releaseVersion,
    fuzzCleanRun: {
      status: "passed",
      workflowRunUrl: `${repoBaseUrl}/actions/runs/1`,
      commit: gitHead(),
      startedAt: "2026-05-25T12:00:00.000Z",
      completedAt,
      aggregateFuzzHours: 40,
      corpusSha256: corpusTreeSha256(),
      targets: [
        "fuzz_anchor_attr",
        "fuzz_anchor_preflight",
        "fuzz_document_parse",
        "fuzz_manifest_parse",
        "fuzz_semantic_diagnostics",
      ],
      workflowMatrixTargets: [
        "fuzz_anchor_attr",
        "fuzz_anchor_preflight",
        "fuzz_document_parse",
        "fuzz_manifest_parse",
        "fuzz_semantic_diagnostics",
      ],
      targetRuns: [
        targetRun("fuzz_anchor_attr"),
        targetRun("fuzz_anchor_preflight"),
        targetRun("fuzz_document_parse"),
        targetRun("fuzz_manifest_parse"),
        targetRun("fuzz_semantic_diagnostics"),
      ],
    },
    externalReview: {
      status: "signed-off",
      mode: "community",
      reviewer: "External Reviewer",
      artifactUrl: `${repoBaseUrl}/pull/1#issuecomment-1`,
      completedAt,
      auditOpenedAt,
      auditWindowDays: 30,
      announcementUrl: `${repoBaseUrl}/issues/1`,
      signoffs: [
        {
          reviewer: "External Reviewer",
          artifactUrl: `${repoBaseUrl}/pull/1#issuecomment-1`,
          signedOffAt: completedAt,
        },
      ],
      findingsDisposition: {
        status: "dispositioned",
        changelogPath: "CHANGELOG.md",
        artifactUrl: `${repoBaseUrl}/blob/v${releaseVersion}/CHANGELOG.md`,
      },
    },
  };
}

function pendingReadiness(): ReleaseReadiness {
  const pending = completeReadiness();
  pending.fuzzCleanRun.status = "pending";
  pending.fuzzCleanRun.workflowRunUrl = "pending";
  pending.fuzzCleanRun.commit = "pending";
  pending.fuzzCleanRun.startedAt = "pending";
  pending.fuzzCleanRun.completedAt = "pending";
  pending.fuzzCleanRun.aggregateFuzzHours = 0;
  delete pending.fuzzCleanRun.corpusSha256;
  for (const run of pending.fuzzCleanRun.targetRuns ?? []) {
    run.status = "pending";
    run.fuzzHours = 0;
  }
  pending.externalReview.status = "pending";
  pending.externalReview.reviewer = "pending";
  pending.externalReview.artifactUrl = "pending";
  pending.externalReview.completedAt = "pending";
  pending.externalReview.auditOpenedAt = "pending";
  pending.externalReview.auditWindowDays = 0;
  pending.externalReview.announcementUrl = "pending";
  delete pending.externalReview.signoffs;
  delete pending.externalReview.findingsDisposition;
  return pending;
}

function targetRun(target: string): FuzzTargetRun {
  return {
    target,
    status: "passed",
    shards: 8,
    secondsPerShard: 3_600,
    fuzzHours: 8,
  };
}

function gitHead(): string {
  return execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
}

type ReleaseReadiness = {
  releaseVersion: string;
  fuzzCleanRun: {
    status: string;
    workflowRunUrl: string;
    commit: string;
    startedAt: string;
    completedAt: string;
    aggregateFuzzHours: number;
    corpusSha256?: string;
    targets: string[];
    workflowMatrixTargets: string[];
    targetRuns?: FuzzTargetRun[];
  };
  externalReview: {
    status: string;
    mode: string;
    reviewer: string;
    artifactUrl: string;
    completedAt: string;
    auditOpenedAt?: string;
    auditWindowDays: number;
    announcementUrl: string;
    signoffs?: Array<{
      reviewer: string;
      artifactUrl: string;
      signedOffAt: string;
    }>;
    findingsDisposition?: {
      status: string;
      changelogPath: string;
      artifactUrl: string;
    };
  };
};

type FuzzTargetRun = {
  target: string;
  status: string;
  shards: number;
  secondsPerShard: number;
  fuzzHours: number;
};
