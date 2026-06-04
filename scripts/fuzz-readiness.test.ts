import { describe, expect, test } from "bun:test";

import { buildFuzzEvidence } from "./fuzz-readiness.ts";

const expectedRepoSlug = "heyAyushh/seagrass";
const fuzzTargets = [
  "fuzz_anchor_attr",
  "fuzz_document_parse",
  "fuzz_manifest_parse",
  "fuzz_semantic_diagnostics",
];

describe("fuzz readiness evidence", () => {
  test("computes aggregate fuzz hours across targets and shards", () => {
    const evidence = buildFuzzEvidence({
      status: "passed",
      workflowRunUrl: `https://github.com/${expectedRepoSlug}/actions/runs/1`,
      commit: "abc123",
      secondsPerShard: 3_600,
      shards: 8,
      completedAt: "2026-05-26T12:00:00.000Z",
      targets: [
        "fuzz_manifest_parse",
        "fuzz_document_parse",
        "fuzz_anchor_attr",
        "fuzz_semantic_diagnostics",
      ],
      workflowMatrixTargets: [
        "fuzz_document_parse",
        "fuzz_anchor_attr",
        "fuzz_manifest_parse",
        "fuzz_semantic_diagnostics",
      ],
    });

    expect(evidence.status).toBe("passed");
    expect(evidence.aggregateFuzzHours).toBe(32);
    expect(evidence.startedAt).toBe("2026-05-25T04:00:00.000Z");
    expect(evidence.targets).toEqual(fuzzTargets);
    expect(evidence.workflowMatrixTargets).toEqual(fuzzTargets);
    expect(evidence.targetRuns).toEqual([
      {
        target: "fuzz_anchor_attr",
        status: "passed",
        shards: 8,
        secondsPerShard: 3_600,
        fuzzHours: 8,
      },
      {
        target: "fuzz_document_parse",
        status: "passed",
        shards: 8,
        secondsPerShard: 3_600,
        fuzzHours: 8,
      },
      {
        target: "fuzz_manifest_parse",
        status: "passed",
        shards: 8,
        secondsPerShard: 3_600,
        fuzzHours: 8,
      },
      {
        target: "fuzz_semantic_diagnostics",
        status: "passed",
        shards: 8,
        secondsPerShard: 3_600,
        fuzzHours: 8,
      },
    ]);
  });

  test("accepts explicit parallel workflow timestamps", () => {
    const evidence = buildFuzzEvidence({
      status: "passed",
      workflowRunUrl: `https://github.com/${expectedRepoSlug}/actions/runs/1`,
      commit: "abc123",
      secondsPerShard: 3_600,
      shards: 8,
      startedAt: "2026-05-26T11:00:00.000Z",
      completedAt: "2026-05-26T12:00:00.000Z",
      targets: [
        "fuzz_manifest_parse",
        "fuzz_document_parse",
        "fuzz_anchor_attr",
        "fuzz_semantic_diagnostics",
      ],
      workflowMatrixTargets: [
        "fuzz_document_parse",
        "fuzz_anchor_attr",
        "fuzz_manifest_parse",
        "fuzz_semantic_diagnostics",
      ],
    });

    expect(evidence.aggregateFuzzHours).toBe(32);
    expect(evidence.startedAt).toBe("2026-05-26T11:00:00.000Z");
  });

  test("rejects stale workflow target matrices", () => {
    expect(() =>
      buildFuzzEvidence({
        status: "passed",
        workflowRunUrl: "https://github.com/heyAyushh/seagrass/actions/runs/1",
        commit: "abc123",
        secondsPerShard: 3_600,
        shards: 8,
        completedAt: "2026-05-26T12:00:00.000Z",
        targets: [
          "fuzz_manifest_parse",
          "fuzz_document_parse",
          "fuzz_anchor_attr",
          "fuzz_semantic_diagnostics",
        ],
        workflowMatrixTargets: ["fuzz_manifest_parse", "fuzz_document_parse"],
      }),
    ).toThrow(/workflowMatrixTargets/);
  });

  test("rejects shard counts that drift from the workflow matrix", () => {
    expect(() =>
      buildFuzzEvidence({
        status: "passed",
        workflowRunUrl: "https://github.com/heyAyushh/seagrass/actions/runs/1",
        commit: "abc123",
        secondsPerShard: 4_200,
        shards: 7,
        completedAt: "2026-05-26T12:00:00.000Z",
        targets: [
          "fuzz_manifest_parse",
          "fuzz_document_parse",
          "fuzz_anchor_attr",
          "fuzz_semantic_diagnostics",
        ],
        workflowMatrixTargets: [
          "fuzz_manifest_parse",
          "fuzz_document_parse",
          "fuzz_anchor_attr",
          "fuzz_semantic_diagnostics",
        ],
      }),
    ).toThrow(/workflow shard count/);
  });
});
