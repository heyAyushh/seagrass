import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";

const verifyProductionPath = resolve(repoRoot, "scripts/verify-production.ts");
const smokeScriptPath = resolve(repoRoot, "scripts/smoke-install.sh");
const readmePath = resolve(repoRoot, "README.md");
const productFramingPath = resolve(repoRoot, "docs/product-framing.md");

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

  test("product framing separates shipped static signals from runtime evidence claims", () => {
    const readme = readFileSync(readmePath, "utf8");
    const productFraming = readFileSync(productFramingPath, "utf8");

    expect(readme).toContain("Codebase intelligence for Solana programs");
    expect(readme).toContain("Runtime intelligence is an evidence-ingestion boundary");
    expect(productFraming).toContain("## Static Layer");
    expect(productFraming).toContain("## Runtime Layer");
    expect(productFraming).toContain("Actual compute-unit usage per instruction");
    expect(productFraming).toContain("Required evidence before claim");
    expect(productFraming).toContain("If any edge case fails this contract");
  });
});
