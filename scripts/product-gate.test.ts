import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";

const verifyProductionPath = resolve(repoRoot, "scripts/verify-production.ts");
const smokeScriptPath = resolve(repoRoot, "scripts/smoke-install.sh");
const readmePath = resolve(repoRoot, "README.md");
const productFramingPath = resolve(repoRoot, "docs/product-framing.md");
const agentSkillHelpPath = resolve(repoRoot, "docs/agent-skill-help.md");
const skillsReadmePath = resolve(repoRoot, "skills/README.md");

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

  test("agent skill discovery is served by the installed CLI", () => {
    const readme = readFileSync(readmePath, "utf8");
    const agentSkillHelp = readFileSync(agentSkillHelpPath, "utf8");
    const skillsReadme = readFileSync(skillsReadmePath, "utf8");

    expect(readme).toContain("seagrass skills list --json");
    expect(readme).toContain("seagrass skills get audit --full");
    expect(readme).toContain("docs/agent-skill-help.md");
    expect(agentSkillHelp).toContain("seagrass skills list --json");
    expect(agentSkillHelp).toContain("seagrass skills get lint --full --json");
    expect(agentSkillHelp).toContain("binaryVersion");
    expect(agentSkillHelp).toContain("src/app/cli/skills.rs");
    expect(agentSkillHelp).toContain("bun scripts/verify-production.ts");
    expect(skillsReadme).toContain("seagrass skills get lint --full");
    expect(skillsReadme).toContain("version-matched to the installed");
    expect(skillsReadme).toContain("binaryVersion");
    expect(skillsReadme).toContain("docs/agent-skill-help.md");
  });
});
