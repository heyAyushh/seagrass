import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { expectedReleaseAssets } from "./package-release.ts";
import { repoRoot } from "./release-evidence.ts";

const releaseWorkflowPath = resolve(repoRoot, ".github/workflows/release.yaml");
const releasePlzWorkflowPath = resolve(repoRoot, ".github/workflows/release-plz.yaml");
const prWorkflowPath = resolve(repoRoot, ".github/workflows/pr.yaml");
const verifyProductionPath = resolve(repoRoot, "scripts/verify-production.ts");
const packageReleasePath = resolve(repoRoot, "scripts/package-release.ts");
const releaseReadinessDocPath = resolve(repoRoot, "docs/release-readiness.md");
const seagrassWorkflowPaths = [
  ".github/workflows/fuzz.yaml",
  ".github/workflows/perf.yaml",
  ".github/workflows/pr.yaml",
  ".github/workflows/property-tests.yaml",
  ".github/workflows/release-plz.yaml",
  ".github/workflows/release.yaml",
];
const pinnedActionReferencePattern = /^[a-z0-9._-]+\/[a-z0-9._-]+@[a-f0-9]{40}$/i;

describe("release workflow packaging", () => {
  test("publishes release readiness evidence with the fuzz corpus package", () => {
    const script = readFileSync(packageReleasePath, "utf8");

    expect(script).toContain("docs/release-readiness.json");
    expect(script).toContain("release-readiness.json");
  });

  test("packages fuzz replay inputs with the fuzz corpus", () => {
    const script = readFileSync(packageReleasePath, "utf8");

    expect(script).toContain("fuzz/fuzz_targets");
    expect(script).toContain(".github/workflows/fuzz.yaml");
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
    for (const asset of expectedReleaseAssets("${VERSION}")) {
      expect(release).toContain(asset);
    }
    expect(release).toContain("sha256sum -c");
  });

  test("packages VSIX metadata against the immutable release tag", () => {
    const script = readFileSync(packageReleasePath, "utf8");

    expect(script).toContain("blob/${context.options.tag}/editors/vscode");
    expect(script).toContain("raw/${context.options.tag}/editors/vscode");
    expect(script).not.toContain("blob/main/editors/vscode");
    expect(script).not.toContain("raw/main/editors/vscode");
  });

  test("uses the root package command for release artifacts", () => {
    const workflow = readFileSync(releaseWorkflowPath, "utf8");
    const buildServer = workflowSection(
      workflow,
      "  build-server:\n    name: Build server",
      "  build-zed:",
    );
    const buildZed = workflowSection(
      workflow,
      "  build-zed:\n    name: Build Zed extension package",
      "  build-vscode:",
    );
    const buildVsCode = workflowSection(
      workflow,
      "  build-vscode:\n    name: Build VS Code VSIX",
      "  package-fuzz-corpus:",
    );
    const packageFuzzCorpus = workflowSection(workflow, "package-fuzz-corpus:", "release:");

    expect(buildServer).toContain("bun scripts/package-release.ts");
    expect(buildServer).toContain("--server");
    expect(buildServer).toContain("--skip-build");
    expect(buildZed).toContain("bun scripts/package-release.ts");
    expect(buildZed).toContain("--zed");
    expect(buildZed).toContain("--zed-wasm");
    expect(buildZed).toContain("--skip-build");
    expect(buildVsCode).toContain("bun scripts/package-release.ts");
    expect(buildVsCode).toContain("--vscode");
    expect(packageFuzzCorpus).toContain("bun scripts/package-release.ts");
    expect(packageFuzzCorpus).toContain("--fuzz-corpus");
  });

  test("keeps release evidence regressions in PR guardrails", () => {
    const workflow = readFileSync(prWorkflowPath, "utf8");

    expect(workflow).toContain("bun test scripts/check-rule-hygiene.test.ts");
    expect(workflow).toContain("bun test scripts/check-diagnostic-topics.test.ts");
    expect(workflow).toContain("bun scripts/check-diagnostic-audit.ts");
    expect(workflow).toContain("bun test scripts/check-diagnostic-audit.test.ts");
    expect(workflow).toContain("bun scripts/check-research-citations.ts");
    expect(workflow).toContain("bun test scripts/check-research-citations.test.ts");
    expect(workflow).toContain("bun test scripts/check-lint-catalog.test.ts");
    expect(workflow).toContain("bun test scripts/check-release-readiness.test.ts");
    expect(workflow).toContain("bun test scripts/fuzz-readiness.test.ts");
    expect(workflow).toContain("bun test scripts/import-fuzz-artifacts.test.ts");
    expect(workflow).toContain("bun test scripts/review-readiness.test.ts");
    expect(workflow).toContain("bun test scripts/apply-release-readiness.test.ts");
    expect(workflow).toContain("bun test scripts/release-workflow.test.ts");
    expect(workflow).toContain("bun test scripts/package-release.test.ts");
    expect(workflow).toContain(
      'bun scripts/check-release-readiness.ts --allow-pending --version "$(tr -d',
    );
  });

  test("runs PR guardrails when release evidence workflows change", () => {
    const workflow = readFileSync(prWorkflowPath, "utf8");

    expect(workflow).toContain('".github/workflows/fuzz.yaml"');
    expect(workflow).toContain('".github/workflows/perf.yaml"');
    expect(workflow).toContain('".github/workflows/property-tests.yaml"');
    expect(workflow).toContain('".github/workflows/release-plz.yaml"');
    expect(workflow).toContain('".github/workflows/release.yaml"');
    expect(workflow).toContain('"release-plz.toml"');
    expect(workflow).toContain('"VERSION"');
    expect(workflow).toContain('"bump-version.sh"');
    expect(workflow).toContain('"scripts/package-release.ts"');
  });

  test("keeps the local production gate aligned with release evidence gates", () => {
    const script = readFileSync(verifyProductionPath, "utf8");

    expect(script).toContain('"--release"');
    expect(script).toContain("Strict release readiness evidence");
    expect(script).toContain('"--allow-pending"');
    expect(script).toContain('"--version"');
    expect(script).toContain("rootVersion");
    expect(script).toContain('"--commit"');
    expect(script).toContain("git rev-parse");
    expect(script).toContain('"scripts/check-rule-hygiene.test.ts"');
    expect(script).toContain('"scripts/check-diagnostic-topics.test.ts"');
    expect(script).toContain('"scripts/check-diagnostic-audit.ts"');
    expect(script).toContain('"scripts/check-diagnostic-audit.test.ts"');
    expect(script).toContain('"scripts/check-research-citations.ts"');
    expect(script).toContain('"scripts/check-research-citations.test.ts"');
    expect(script).toContain('"scripts/check-lint-catalog.test.ts"');
    expect(script).toContain('"scripts/verify-proptest.ts"');
    expect(script).toContain('"scripts/package-release.test.ts"');
  });

  test("documents the same concrete proof constraints as the validator", () => {
    const releaseReadiness = readFileSync(releaseReadinessDocPath, "utf8");

    expect(releaseReadiness).toContain("bun scripts/verify-production.ts --release");
    expect(releaseReadiness).toContain("concrete GitHub Actions run URL");
    expect(releaseReadiness).toContain("positive run id");
    expect(releaseReadiness).toContain("concrete pull, issue, or discussion URL");
    expect(releaseReadiness).toContain("immutable CHANGELOG.md proof URL");
    expect(releaseReadiness).toContain("not `main` or `master`");
    expect(releaseReadiness).toContain("fuzzCleanRun.corpusSha256");
    expect(releaseReadiness).toContain("rejects placeholder workflow run ids");
    expect(releaseReadiness).toContain("rejects generic review proof URLs");
    expect(releaseReadiness).toContain("mutable changelog proof URLs");
    expect(releaseReadiness).toContain("mutable VSIX release metadata URLs");
    expect(releaseReadiness).toContain("unpinned workflow actions");
    expect(releaseReadiness).toContain("stale corpus hashes");
  });

  test("pins every Seagrass workflow action to an immutable SHA", () => {
    const failures = seagrassWorkflowPaths.flatMap((path) =>
      unpinnedActionReferences(path, readFileSync(resolve(repoRoot, path), "utf8")),
    );

    expect(failures).toEqual([]);
  });

  test("runs workflow commands from the standalone repository root", () => {
    const failures = seagrassWorkflowPaths.flatMap((path) =>
      staleOverlayPathReferences(path, readFileSync(resolve(repoRoot, path), "utf8")),
    );

    expect(failures).toEqual([]);
  });

  test("installs cargo-fuzz with stable cargo before nightly fuzzing", () => {
    const fuzzWorkflow = readFileSync(resolve(repoRoot, ".github/workflows/fuzz.yaml"), "utf8");
    const releaseWorkflow = readFileSync(releaseWorkflowPath, "utf8");

    expect(fuzzWorkflow).toContain("cargo +stable install cargo-fuzz --locked");
    expect(releaseWorkflow).toContain("cargo +stable install cargo-fuzz --locked");
    expect(fuzzWorkflow).not.toContain("run: cargo install cargo-fuzz --locked");
    expect(releaseWorkflow).not.toContain("run: cargo install cargo-fuzz --locked");
  });

  test("keeps release-plz on the repository default branch", () => {
    const workflow = readFileSync(releasePlzWorkflowPath, "utf8");

    expect(workflow).toContain("branches: [master]");
    expect(workflow).not.toContain("branches: [main]");
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

function staleOverlayPathReferences(path: string, contents: string): string[] {
  return contents
    .split(/\r?\n/)
    .flatMap((line, index) => {
      if (line.includes("working-directory: lsp") || /\s-C\s+lsp\b/.test(line)) {
        return [`${path}:${index + 1} uses stale lsp/ overlay path`];
      }
      return [];
    });
}
