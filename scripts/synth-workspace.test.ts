import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dir, "../..");
const testRoot = resolve(repoRoot, "target/synth-workspace-tests");
const expectedAnchorLangPath = "../../../../lang";

describe("synthetic Anchor workspace", () => {
  test("uses the real workspace anchor-lang path", () => {
    mkdirSync(testRoot, { recursive: true });
    const outputRoot = mkdtempSync(resolve(testRoot, "workspace-"));

    const result = spawnSync(
      "bun",
      ["lsp/scripts/synth-workspace.ts", "--programs", "1", "--output", outputRoot],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );

    expect(result.status).toBe(0);
    const manifest = readFileSync(resolve(outputRoot, "programs/program_0/Cargo.toml"), "utf8");
    expect(manifest).toContain(`anchor-lang = { path = "${expectedAnchorLangPath}" }`);
  });
});
