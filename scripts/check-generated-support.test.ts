import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

import {
  currentGeneratedSupportLock,
  generatedSupportLockFailures,
  type GeneratedSupportRoot,
} from "./check-generated-support.ts";

const generatedRoot: GeneratedSupportRoot = {
  id: "anchor-test",
  path: "generated",
  source: "test generated root",
  expectedFiles: ["anchor_support_generated.rs", "constraint_catalog_generated.rs"],
};

describe("generated support checker", () => {
  test("accepts an unchanged generated support lock", () => {
    withTempRepo((repoRoot) => {
      writeGeneratedFile(repoRoot, "anchor_support_generated.rs", "manifest");
      writeGeneratedFile(repoRoot, "constraint_catalog_generated.rs", "catalog");

      const lock = currentGeneratedSupportLock({ repoRoot, roots: [generatedRoot] });

      expect(generatedSupportLockFailures(lock, lock)).toEqual([]);
    });
  });

  test("rejects a hand-edited generated file", () => {
    withTempRepo((repoRoot) => {
      writeGeneratedFile(repoRoot, "anchor_support_generated.rs", "manifest");
      writeGeneratedFile(repoRoot, "constraint_catalog_generated.rs", "catalog");
      const lock = currentGeneratedSupportLock({ repoRoot, roots: [generatedRoot] });

      writeGeneratedFile(repoRoot, "constraint_catalog_generated.rs", "edited catalog");
      const edited = currentGeneratedSupportLock({ repoRoot, roots: [generatedRoot] });

      expect(generatedSupportLockFailures(lock, edited).join("\n")).toContain(
        "anchor-test/constraint_catalog_generated.rs: sha256 mismatch",
      );
    });
  });

  test("rejects unexpected generated Rust files", () => {
    withTempRepo((repoRoot) => {
      writeGeneratedFile(repoRoot, "anchor_support_generated.rs", "manifest");
      writeGeneratedFile(repoRoot, "constraint_catalog_generated.rs", "catalog");
      writeGeneratedFile(repoRoot, "extra_generated.rs", "extra");

      expect(() => currentGeneratedSupportLock({ repoRoot, roots: [generatedRoot] })).toThrow(
        "anchor-test: unexpected extra_generated.rs",
      );
    });
  });
});

function withTempRepo(run: (repoRoot: string) => void): void {
  const repoRoot = mkdtempSync(resolve(tmpdir(), "seagrass-generated-support-"));
  try {
    mkdirSync(resolve(repoRoot, generatedRoot.path), { recursive: true });
    run(repoRoot);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
}

function writeGeneratedFile(repoRoot: string, file: string, text: string): void {
  writeFileSync(resolve(repoRoot, generatedRoot.path, file), text);
}
