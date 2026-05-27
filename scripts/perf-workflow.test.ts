import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";

const perfWorkflowPath = resolve(repoRoot, ".github/workflows/lsp-perf.yaml");
const prWorkflowPath = resolve(repoRoot, ".github/workflows/lsp-pr.yaml");
const verifyProductionPath = resolve(repoRoot, "scripts/verify-production.ts");

describe("perf workflow guardrails", () => {
  test("runs hotpath replay with budget enforcement", () => {
    const workflow = readFileSync(perfWorkflowPath, "utf8");

    expect(workflow).toContain("Replay LSP request stream with hotpath budgets");
    expect(workflow).toContain("bun scripts/perf-replay.ts");
    expect(workflow).toContain("target/seagrass-hotpath-report.json");
  });

  test("publishes scale reports for the 10/50/200 workspace sweep", () => {
    const workflow = readFileSync(perfWorkflowPath, "utf8");

    expect(workflow).toContain("programs: [10, 50, 200]");
    expect(workflow).toContain("bun scripts/scale-benchmark.ts");
    expect(workflow).toContain("--programs ${{ matrix.programs }}");
    expect(workflow).toContain("--samples 3");
    expect(workflow).toContain("--report target/seagrass-scale-${{ matrix.programs }}.json");
    expect(workflow).toContain("seagrass-scale-${{ matrix.programs }}-report");
    expect(workflow).toContain("target/seagrass-scale-${{ matrix.programs }}.json");
  });

  test("keeps perf workflow drift in PR and local production guardrails", () => {
    const prWorkflow = readFileSync(prWorkflowPath, "utf8");
    const verifyProduction = readFileSync(verifyProductionPath, "utf8");

    expect(prWorkflow).toContain("bun test scripts/perf-workflow.test.ts");
    expect(verifyProduction).toContain('"scripts/perf-workflow.test.ts"');
    expect(verifyProduction).toContain('"--samples"');
    expect(verifyProduction).toContain('"3"');
    expect(verifyProduction).toContain('"target/seagrass-scale-200.json"');
    expect(verifyProduction).toContain("SEAGRASS_SERVER_BINARY");
    expect(verifyProduction).toContain("target/debug/seagrass");
  });
});
