import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dir, "..");
const testRoot = resolve(repoRoot, "target/synth-workspace-tests");
const expectedAnchorLangDependency =
  'anchor-lang = { git = "https://github.com/otter-sec/anchor.git", rev = "4addac53" }';

describe("synthetic Anchor workspace", () => {
  test("uses the pinned upstream anchor-lang dependency", () => {
    mkdirSync(testRoot, { recursive: true });
    const outputRoot = mkdtempSync(resolve(testRoot, "workspace-"));

    const result = spawnSync(
      "bun",
      ["scripts/synth-workspace.ts", "--programs", "1", "--output", outputRoot],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(0);
    const manifest = readFileSync(resolve(outputRoot, "programs/program_0/Cargo.toml"), "utf8");
    expect(manifest).toContain(expectedAnchorLangDependency);
  });
});
