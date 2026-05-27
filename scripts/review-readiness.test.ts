import { describe, expect, test } from "bun:test";

import { expectedGithubRepoSlug } from "./release-evidence.ts";
import { buildReviewEvidence } from "./review-readiness.ts";

const expectedRepoSlug = "heyAyushh/seagrass";

process.env.GITHUB_REPOSITORY = expectedRepoSlug;

describe("review readiness evidence", () => {
  test("builds signoff and findings disposition evidence", () => {
    const repoBaseUrl = `https://github.com/${expectedGithubRepoSlug()}`;
    const evidence = buildReviewEvidence({
      mode: "community",
      reviewer: "External Reviewer",
      artifactUrl: `${repoBaseUrl}/pull/1#issuecomment-1`,
      findingsArtifactUrl: `${repoBaseUrl}/blob/v1.0.2/CHANGELOG.md`,
      completedAt: "2026-05-26T12:00:00.000Z",
      auditOpenedAt: "2026-04-26T12:00:00.000Z",
      announcementUrl: `${repoBaseUrl}/issues/1`,
    });

    expect(evidence).toEqual({
      status: "signed-off",
      mode: "community",
      reviewer: "External Reviewer",
      artifactUrl: `${repoBaseUrl}/pull/1#issuecomment-1`,
      completedAt: "2026-05-26T12:00:00.000Z",
      auditOpenedAt: "2026-04-26T12:00:00.000Z",
      auditWindowDays: 30,
      announcementUrl: `${repoBaseUrl}/issues/1`,
      signoffs: [
        {
          reviewer: "External Reviewer",
          artifactUrl: `${repoBaseUrl}/pull/1#issuecomment-1`,
          signedOffAt: "2026-05-26T12:00:00.000Z",
        },
      ],
      findingsDisposition: {
        status: "dispositioned",
        changelogPath: "CHANGELOG.md",
        artifactUrl: `${repoBaseUrl}/blob/v1.0.2/CHANGELOG.md`,
      },
    });
  });

  test("rejects non-https review artifacts", () => {
    expect(() =>
      buildReviewEvidence({
        mode: "paid",
        reviewer: "External Reviewer",
        artifactUrl: "http://example.com/review",
        findingsArtifactUrl: `https://github.com/${expectedGithubRepoSlug()}/blob/v1.0.2/CHANGELOG.md`,
        completedAt: "2026-05-26T12:00:00.000Z",
      }),
    ).toThrow(/github\.com/);
  });

  test("rejects generic review proof URLs", () => {
    const repoBaseUrl = `https://github.com/${expectedGithubRepoSlug()}`;

    expect(() =>
      buildReviewEvidence({
        mode: "community",
        reviewer: "External Reviewer",
        artifactUrl: `${repoBaseUrl}/pulls`,
        findingsArtifactUrl: `${repoBaseUrl}/blob/v1.0.2/CHANGELOG.md`,
        completedAt: "2026-05-26T12:00:00.000Z",
        auditOpenedAt: "2026-04-26T12:00:00.000Z",
        announcementUrl: `${repoBaseUrl}/issues/1`,
      }),
    ).toThrow(/concrete/);
  });

  test("rejects short community review windows", () => {
    const repoBaseUrl = `https://github.com/${expectedGithubRepoSlug()}`;
    expect(() =>
      buildReviewEvidence({
        mode: "community",
        reviewer: "External Reviewer",
        artifactUrl: `${repoBaseUrl}/pull/1#issuecomment-1`,
        findingsArtifactUrl: `${repoBaseUrl}/blob/v1.0.2/CHANGELOG.md`,
        completedAt: "2026-05-26T12:00:00.000Z",
        auditOpenedAt: "2026-05-25T12:00:00.000Z",
        announcementUrl: `${repoBaseUrl}/issues/1`,
      }),
    ).toThrow(/at least 30 days/);
  });

  test("rejects mutable changelog proof URLs", () => {
    const repoBaseUrl = `https://github.com/${expectedGithubRepoSlug()}`;

    expect(() =>
      buildReviewEvidence({
        mode: "paid",
        reviewer: "External Reviewer",
        artifactUrl: `${repoBaseUrl}/pull/1#issuecomment-1`,
        findingsArtifactUrl: `${repoBaseUrl}/blob/main/CHANGELOG.md`,
        completedAt: "2026-05-26T12:00:00.000Z",
      }),
    ).toThrow(/immutable CHANGELOG.md proof/);
  });

  test("rejects pending reviewer sentinels", () => {
    const repoBaseUrl = `https://github.com/${expectedGithubRepoSlug()}`;

    expect(() =>
      buildReviewEvidence({
        mode: "paid",
        reviewer: "pending",
        artifactUrl: `${repoBaseUrl}/pull/1#issuecomment-1`,
        findingsArtifactUrl: `${repoBaseUrl}/blob/v1.0.2/CHANGELOG.md`,
        completedAt: "2026-05-26T12:00:00.000Z",
      }),
    ).toThrow(/reviewer must name a reviewer/);
  });
});
