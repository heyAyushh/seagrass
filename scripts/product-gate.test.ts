import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";

const verifyProductionPath = resolve(repoRoot, "scripts/verify-production.ts");
const smokeScriptPath = resolve(repoRoot, "scripts/smoke-install.sh");
const anchorV2PreviewCorpusScriptPath = resolve(repoRoot, "scripts/check-anchor-v2-preview-corpus.ts");
const readmePath = resolve(repoRoot, "README.md");
const frameworkParityPath = resolve(repoRoot, "docs/framework-parity.md");
const productFramingPath = resolve(repoRoot, "docs/product-framing.md");
const agentSkillHelpPath = resolve(repoRoot, "docs/agent-skill-help.md");
const skillsReadmePath = resolve(repoRoot, "skills/README.md");
const anchorV2PreviewCratePath = resolve(repoRoot, "crates/seagrass-anchor-v2-preview/src/lib.rs");
const anchorV2PreviewManifestPath = resolve(
  repoRoot,
  "crates/seagrass-anchor-v2-preview/src/generated/anchor_support_generated.rs",
);

describe("product gate guardrails", () => {
  test("verify-production runs golden-path smoke and lint index freshness", () => {
    const verifyProduction = readFileSync(verifyProductionPath, "utf8");

    expect(verifyProduction).toContain("Golden-path install smoke");
    expect(verifyProduction).toContain("scripts/smoke-install.sh");
    expect(verifyProduction).toContain("Preflight CLI demo");
    expect(verifyProduction).toContain("scripts/check-preflight-demo.ts");
    expect(verifyProduction).toContain("Lint docs index freshness");
    expect(verifyProduction).toContain("scripts/build-lint-docs-index.ts");
    expect(verifyProduction).toContain('"--check"');
  });

  test("smoke script targets the checked-in fixture", () => {
    const smokeScript = readFileSync(smokeScriptPath, "utf8");

    expect(smokeScript).toContain("fixtures/smoke-broken.rs");
    expect(smokeScript).toContain("target/debug/seagrass");
  });

  test("Anchor v2 preview corpus check is source-driven", () => {
    const corpusScript = readFileSync(anchorV2PreviewCorpusScriptPath, "utf8");

    expect(corpusScript).toContain("scripts/regen-support.ts");
    expect(corpusScript).toContain('"v2-preview"');
    expect(corpusScript).toContain("seagrass-anchor-v2-preview/src/generated");
    expect(corpusScript).toContain("diagnostics");
    expect(corpusScript).toContain("Anchor v2 preview examples should not produce ERROR diagnostics");
  });

  test("product framing separates shipped static signals from runtime evidence claims", () => {
    const readme = readFileSync(readmePath, "utf8");
    const productFraming = readFileSync(productFramingPath, "utf8");

    expect(readme).toContain("Codebase intelligence for Solana programs");
    expect(readme).toContain("Runtime intelligence is an evidence-ingestion boundary");
    expect(productFraming).toContain("## Static Layer");
    expect(productFraming).toContain("## Preflight Layer");
    expect(productFraming).toContain("seagrass preflight fixtures/preflight/anchor-errors.json --json");
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
    expect(skillsReadme).toContain("seagrass preflight fixtures/preflight/anchor-errors.json --json");
    expect(skillsReadme).toContain("version-matched to the installed");
    expect(skillsReadme).toContain("binaryVersion");
    expect(skillsReadme).toContain("docs/agent-skill-help.md");
  });

  test("Anchor v2 preview claim is grounded in framework metadata", () => {
    const readme = readFileSync(readmePath, "utf8");
    const frameworkParity = readFileSync(frameworkParityPath, "utf8");
    const anchorV2PreviewCrate = readFileSync(anchorV2PreviewCratePath, "utf8");
    const anchorV2PreviewManifest = readFileSync(anchorV2PreviewManifestPath, "utf8");

    expect(readme).toContain("Anchor v1 and Anchor v2 preview get the deep treatment");
    expect(frameworkParity).toContain("Anchor v1/v2 preview");
    expect(anchorV2PreviewCrate).toContain('const DISPLAY_NAME: &str = "Anchor v2 preview";');
    expect(anchorV2PreviewCrate).toContain("FrameworkKind::AnchorV2Preview");
    expect(anchorV2PreviewCrate).toContain("SupportLevel::Preview");
    expect(anchorV2PreviewManifest).toContain('anchor_version: "2.0.0"');
    expect(anchorV2PreviewManifest).toContain("support_level: AnchorSupportLevel::AnchorV2Preview");
    expect(anchorV2PreviewManifest).toContain('version_family: "anchor-v2-preview"');
    expect(anchorV2PreviewManifest).toContain("anchor-next parser");
    expect(anchorV2PreviewManifest).not.toContain("newer Anchor v1 releases");
  });
});
