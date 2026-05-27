#!/usr/bin/env bun

import { describe, expect, test } from "bun:test";

import {
  auditCellFailures,
  auditCoverageFailures,
  auditRowsFromMarkdown,
  auditableSourceFailures,
  auditTopicFailures,
  sourcePathFailures,
  type AuditRow,
} from "./check-diagnostic-audit";

const knownTopics = new Set([
  "seagrass/security.owner-check",
  "seagrass/security.type-cosplay",
]);

const concreteRow: AuditRow = {
  line: 10,
  file: "diagnostics/security.rs",
  provider: "raw owner checks",
  astAware: "visitor",
  regionAware: "handler body",
  confidence: "heuristic",
  topic: "seagrass/security.owner-check",
  substringRisk: "low",
  fixture: "raw account tests",
};

describe("diagnostic audit checker", () => {
  test("rejects vague wildcard and unknown topic cells", () => {
    const rows = [
      concreteRow,
      { ...concreteRow, provider: "wildcard", topic: "seagrass/security.*" },
      { ...concreteRow, provider: "vague", topic: "owner/type topics" },
      { ...concreteRow, provider: "unknown", topic: "seagrass/security.signer-authorization" },
    ];

    const failures = auditTopicFailures(rows, knownTopics, new Set(["seagrass/security.owner-check"]));

    expect(failures.join("\n")).toContain("wildcard topic cells are not allowed");
    expect(failures.join("\n")).toContain("must use concrete seagrass topics or n/a");
    expect(failures.join("\n")).toContain("not present in lsp/docs/topics.json");
  });

  test("requires emitted diagnostic topics to appear in the audit", () => {
    const failures = auditTopicFailures(
      [concreteRow],
      knownTopics,
      new Set(["seagrass/security.owner-check", "seagrass/security.type-cosplay"]),
    );

    expect(failures).toContain("seagrass/security.type-cosplay missing from lsp/docs/diagnostic-audit.md");
  });

  test("rejects vague audit cells and non-taxonomy confidence labels", () => {
    const failures = auditCellFailures([
      {
        ...concreteRow,
        astAware: "yes",
        regionAware: "yes",
        confidence: "high",
        substringRisk: "maybe",
        fixture: "tests",
      },
    ]);

    expect(failures.join("\n")).toContain("AST-aware cell must name concrete evidence");
    expect(failures.join("\n")).toContain("region-aware cell must name concrete evidence");
    expect(failures.join("\n")).toContain("confidence must use structured authoritative, derived, heuristic");
    expect(failures.join("\n")).toContain("substring risk must be low, n/a, or fixed:");
    expect(failures.join("\n")).toContain("fixture cell must name concrete coverage");
    expect(failures.join("\n")).toContain("diagnostics/security.rs:10");
  });

  test("accepts concrete audit evidence cells", () => {
    expect(auditCellFailures([concreteRow])).toEqual([]);
    expect(
      auditCellFailures([
        {
          ...concreteRow,
          confidence: "authoritative/heuristic",
          substringRisk: "fixed: comments/strings",
          fixture: "`ignores_comments_and_strings`",
        },
      ]),
    ).toEqual([]);
  });

  test("requires concrete false-positive coverage for fixed substring risks", () => {
    const failures = auditCellFailures([
      {
        ...concreteRow,
        substringRisk: "fixed: comments/strings",
        fixture: "security core tests",
      },
    ]);

    expect(failures.join("\n")).toContain(
      "fixture cell for fixed substring risk must name a false-positive regression",
    );
    expect(
      auditCellFailures([
        {
          ...concreteRow,
          substringRisk: "fixed: comments/strings",
          fixture: "`reports_owner_check`, false-positive tests",
        },
      ]).join("\n"),
    ).toContain("fixture cell for fixed substring risk must name a false-positive regression");
    expect(
      auditCellFailures([
        {
          ...concreteRow,
          substringRisk: "fixed: comments/strings",
          fixture: "`ignores_comments_and_strings`, false-positive tests",
        },
      ]),
    ).toEqual([]);
  });

  test("requires fixed substring fixtures to reference existing Rust tests", () => {
    expect(
      auditCellFailures(
        [
          {
            ...concreteRow,
            substringRisk: "fixed: comments/strings",
            fixture: "`ignores_missing_case`",
          },
        ],
        { rustTestNames: new Set(["ignores_comments_and_strings"]) },
      ).join("\n"),
    ).toContain("references missing false-positive Rust tests: ignores_missing_case");
    expect(
      auditCellFailures(
        [
          {
            ...concreteRow,
            substringRisk: "fixed: comments/strings",
            fixture: "`ignores_comments_and_strings`",
          },
        ],
        { rustTestNames: new Set(["ignores_comments_and_strings"]) },
      ),
    ).toEqual([]);
    expect(
      auditCellFailures(
        [
          {
            ...concreteRow,
            substringRisk: "fixed: comments/strings",
            fixture: "`ignores_comments_and_strings`, `ignores_missing_case`",
          },
        ],
        { rustTestNames: new Set(["ignores_comments_and_strings"]) },
      ).join("\n"),
    ).toContain("ignores_missing_case");
  });

  test("requires audit coverage for diagnostics and editor providers", () => {
    const rows = [
      concreteRow,
      { ...concreteRow, file: "completions/mod.rs" },
      { ...concreteRow, file: "actions/mod.rs" },
    ];

    expect(auditCoverageFailures(rows)).toEqual([
      "lsp/docs/diagnostic-audit.md must include at least one hover/ provider row",
    ]);
  });

  test("requires auditable provider source files to have rows", () => {
    const rows = [
      concreteRow,
      { ...concreteRow, file: "completions/mod.rs" },
      { ...concreteRow, file: "hover/mod.rs" },
      { ...concreteRow, file: "actions/mod.rs" },
    ];
    const sourcePaths = [
      "diagnostics/security.rs",
      "completions/mod.rs",
      "completions/account_fields/context.rs",
      "hover/mod.rs",
      "actions/mod.rs",
      "actions/common.rs",
      "diagnostics/security/tests/raw_account_tests.rs",
    ];

    expect(auditableSourceFailures(rows, sourcePaths)).toEqual([
      "completions/account_fields/context.rs is missing from lsp/docs/diagnostic-audit.md",
    ]);
  });

  test("allows wildcard rows to cover generated provider directories", () => {
    const rows = [
      concreteRow,
      { ...concreteRow, file: "diagnostics/constraint_shape/*" },
    ];

    expect(
      auditableSourceFailures(rows, [
        "diagnostics/security.rs",
        "diagnostics/constraint_shape/pda.rs",
        "diagnostics/constraint_shape/support.rs",
      ]),
    ).toEqual([]);
  });

  test("parses both audit tables and validates provider paths", () => {
    const text = `
| file | provider | AST-aware | region-aware | confidence | topic | substring risk | fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| \`diagnostics/security.rs\` | \`owner\` | visitor | handler body | heuristic | \`seagrass/security.owner-check\` | low | owner tests |

| file | provider branch | AST-aware | region-aware | confidence | topic | substring risk | fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| \`diagnostics/security/raw_account.rs\` | raw evidence | visitor | body | n/a | \`seagrass/security.type-cosplay\` | low | tests |
`;

    const rows = auditRowsFromMarkdown(text);

    expect(rows.map((row) => row.file)).toEqual([
      "diagnostics/security.rs",
      "diagnostics/security/raw_account.rs",
    ]);
    expect(sourcePathFailures(rows, "/repo/lsp/src", new Set(["/repo/lsp/src/diagnostics/security.rs"]))).toEqual([
      "diagnostics/security/raw_account.rs does not exist under /repo/lsp/src",
    ]);
  });
});
