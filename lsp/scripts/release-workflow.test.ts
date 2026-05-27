import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";

const releaseWorkflowPath = resolve(repoRoot, ".github/workflows/lsp-release.yaml");
const prWorkflowPath = resolve(repoRoot, ".github/workflows/lsp-pr.yaml");
const verifyProductionPath = resolve(repoRoot, "lsp/scripts/verify-production.ts");
const releaseReadinessDocPath = resolve(repoRoot, "lsp/docs/release-readiness.md");
const completionAuditPath = resolve(repoRoot, "lsp/docs/10-10-completion-audit.md");
const seagrassWorkflowPaths = [
  ".github/workflows/lsp-fuzz.yaml",
  ".github/workflows/lsp-perf.yaml",
  ".github/workflows/lsp-pr.yaml",
  ".github/workflows/lsp-property-tests.yaml",
  ".github/workflows/lsp-release.yaml",
];
const pinnedActionReferencePattern = /^[a-z0-9._-]+\/[a-z0-9._-]+@[a-f0-9]{40}$/i;

describe("release workflow packaging", () => {
  test("publishes release readiness evidence with the fuzz corpus package", () => {
    const workflow = readFileSync(releaseWorkflowPath, "utf8");
    const packageFuzzCorpus = workflowSection(workflow, "package-fuzz-corpus:", "release:");

    expect(packageFuzzCorpus).toContain("cp lsp/docs/release-readiness.json");
    expect(packageFuzzCorpus).toContain("release-readiness.json");
  });

  test("packages fuzz replay inputs with the fuzz corpus", () => {
    const workflow = readFileSync(releaseWorkflowPath, "utf8");
    const packageFuzzCorpus = workflowSection(workflow, "package-fuzz-corpus:", "release:");

    expect(packageFuzzCorpus).toContain("cp -R lsp/fuzz/fuzz_targets");
    expect(packageFuzzCorpus).toContain("cp .github/workflows/lsp-fuzz.yaml");
  });

  test("verifies the exact release asset inventory before publishing", () => {
    const workflow = readFileSync(releaseWorkflowPath, "utf8");
    const release = workflowSection(
      workflow,
      "  release:\n    name: Publish GitHub release",
      "release_args=()",
    );

    expect(release).toContain("expected_assets=(");
    expect(occurrences(release, "expected_assets=(")).toBe(1);
    expect(release).toContain("seagrass-${VERSION}-aarch64-apple-darwin.tar.gz");
    expect(release).toContain("seagrass-${VERSION}-x86_64-apple-darwin.tar.gz");
    expect(release).toContain("seagrass-${VERSION}-x86_64-unknown-linux-gnu.tar.gz");
    expect(release).toContain("seagrass-${VERSION}-x86_64-pc-windows-msvc.tar.gz");
    expect(release).toContain("seagrass-zed-${VERSION}.tar.gz");
    expect(release).toContain("seagrass-vscode-${VERSION}.vsix");
    expect(release).toContain("seagrass-${VERSION}-fuzz-corpus.tar.gz");
    expect(release).toContain("sha256sum -c");
  });

  test("packages VSIX metadata against the immutable release tag", () => {
    const workflow = readFileSync(releaseWorkflowPath, "utf8");
    const buildVsCode = workflowSection(
      workflow,
      "  build-vscode:\n    name: Build VS Code VSIX",
      "  package-fuzz-corpus:",
    );

    expect(buildVsCode).toContain(
      "blob/${{ needs.verify-tag.outputs.tag }}/lsp/editors/vscode",
    );
    expect(buildVsCode).toContain(
      "raw/${{ needs.verify-tag.outputs.tag }}/lsp/editors/vscode",
    );
    expect(buildVsCode).not.toContain("blob/main/lsp/editors/vscode");
    expect(buildVsCode).not.toContain("raw/main/lsp/editors/vscode");
  });

  test("keeps release evidence regressions in PR guardrails", () => {
    const workflow = readFileSync(prWorkflowPath, "utf8");

    expect(workflow).toContain("bun test lsp/scripts/check-rule-hygiene.test.ts");
    expect(workflow).toContain("bun test lsp/scripts/check-diagnostic-topics.test.ts");
    expect(workflow).toContain("bun lsp/scripts/check-diagnostic-audit.ts");
    expect(workflow).toContain("bun test lsp/scripts/check-diagnostic-audit.test.ts");
    expect(workflow).toContain("bun lsp/scripts/check-research-citations.ts");
    expect(workflow).toContain("bun test lsp/scripts/check-research-citations.test.ts");
    expect(workflow).toContain("bun test lsp/scripts/check-lint-catalog.test.ts");
    expect(workflow).toContain("bun test lsp/scripts/check-release-readiness.test.ts");
    expect(workflow).toContain("bun test lsp/scripts/fuzz-readiness.test.ts");
    expect(workflow).toContain("bun test lsp/scripts/import-fuzz-artifacts.test.ts");
    expect(workflow).toContain("bun test lsp/scripts/review-readiness.test.ts");
    expect(workflow).toContain("bun test lsp/scripts/apply-release-readiness.test.ts");
    expect(workflow).toContain("bun test lsp/scripts/release-workflow.test.ts");
    expect(workflow).toContain(
      'bun lsp/scripts/check-release-readiness.ts --allow-pending --version "$(tr -d',
    );
  });

  test("runs PR guardrails when release evidence workflows change", () => {
    const workflow = readFileSync(prWorkflowPath, "utf8");

    expect(workflow).toContain('".github/workflows/lsp-fuzz.yaml"');
    expect(workflow).toContain('".github/workflows/lsp-perf.yaml"');
    expect(workflow).toContain('".github/workflows/lsp-property-tests.yaml"');
    expect(workflow).toContain('".github/workflows/lsp-release.yaml"');
    expect(workflow).toContain('"VERSION"');
    expect(workflow).toContain('"bump-version.sh"');
  });

  test("keeps the local production gate aligned with release evidence gates", () => {
    const script = readFileSync(verifyProductionPath, "utf8");

    expect(script).toContain('"--version"');
    expect(script).toContain("rootVersion");
    expect(script).toContain('"lsp/scripts/check-rule-hygiene.test.ts"');
    expect(script).toContain('"lsp/scripts/check-diagnostic-topics.test.ts"');
    expect(script).toContain('"lsp/scripts/check-diagnostic-audit.ts"');
    expect(script).toContain('"lsp/scripts/check-diagnostic-audit.test.ts"');
    expect(script).toContain('"lsp/scripts/check-research-citations.ts"');
    expect(script).toContain('"lsp/scripts/check-research-citations.test.ts"');
    expect(script).toContain('"lsp/scripts/check-lint-catalog.test.ts"');
    expect(script).toContain('"lsp/scripts/verify-proptest.ts"');
  });

  test("documents the same concrete proof constraints as the validator", () => {
    const releaseReadiness = readFileSync(releaseReadinessDocPath, "utf8");
    const completionAudit = readFileSync(completionAuditPath, "utf8");

    expect(releaseReadiness).toContain("concrete GitHub Actions run URL");
    expect(releaseReadiness).toContain("positive run id");
    expect(releaseReadiness).toContain("concrete pull, issue, or discussion URL");
    expect(releaseReadiness).toContain("immutable CHANGELOG.md proof URL");
    expect(releaseReadiness).toContain("not `main` or `master`");
    expect(releaseReadiness).toContain("fuzzCleanRun.corpusSha256");
    expect(completionAudit).toContain("rejects placeholder workflow run ids");
    expect(completionAudit).toContain("rejects generic review proof URLs");
    expect(completionAudit).toContain("mutable changelog proof URLs");
    expect(completionAudit).toContain("mutable VSIX release metadata URLs");
    expect(completionAudit).toContain("unpinned workflow actions");
    expect(completionAudit).toContain("stale corpus hashes");
  });

  test("pins every Seagrass workflow action to an immutable SHA", () => {
    const failures = seagrassWorkflowPaths.flatMap((path) =>
      unpinnedActionReferences(path, readFileSync(resolve(repoRoot, path), "utf8")),
    );

    expect(failures).toEqual([]);
  });
});

function workflowSection(contents: string, start: string, end: string): string {
  const startIndex = contents.indexOf(start);
  const endIndex = contents.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    throw new Error(`could not find workflow section ${start}`);
  }
  return contents.slice(startIndex, endIndex);
}

function occurrences(haystack: string, needle: string): number {
  return haystack.split(needle).length - 1;
}

function unpinnedActionReferences(path: string, contents: string): string[] {
  return contents
    .split(/\r?\n/)
    .flatMap((line, index) => {
      const match = line.match(/\buses:\s*([^#\s]+)/);
      if (!match || pinnedActionReferencePattern.test(match[1])) {
        return [];
      }
      return [`${path}:${index + 1} uses ${match[1]}`];
    });
}
