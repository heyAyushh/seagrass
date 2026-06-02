import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";

const verifyProductionPath = resolve(repoRoot, "scripts/verify-production.ts");
const smokeScriptPath = resolve(repoRoot, "scripts/smoke-install.sh");

describe("product gate guardrails", () => {
  test("verify-production runs golden-path smoke and lint index freshness", () => {
    const verifyProduction = readFileSync(verifyProductionPath, "utf8");

    expect(verifyProduction).toContain("Golden-path install smoke");
    expect(verifyProduction).toContain("scripts/smoke-install.sh");
    expect(verifyProduction).toContain("Lint docs index freshness");
    expect(verifyProduction).toContain("scripts/build-lint-docs-index.ts");
    expect(verifyProduction).toContain('"--check"');
  });

  test("smoke script targets the checked-in fixture", () => {
    const smokeScript = readFileSync(smokeScriptPath, "utf8");

    expect(smokeScript).toContain("fixtures/smoke-broken.rs");
    expect(smokeScript).toContain("target/debug/seagrass");
  });
});