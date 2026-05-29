#!/usr/bin/env bun

import { describe, expect, test } from "bun:test";

import {
  auditCellFailures,
  auditCoverageFailures,
  auditRowsFromMarkdown,
  auditableSourceFailures,
  auditTopicFailures,
  quickfixCoverageFailures,
  quickfixRowsFromMarkdown,
  sourcePathFailures,
  type AuditRow,
  type QuickfixAuditRow,
} from "./check-diagnostic-audit";

const knownTopics = new Set([
  "seagrass/security.owner-check",
  "seagrass/security.type-cosplay",
]);

const concreteRow: AuditRow = {
  line: 10,
  file: "lsp/diagnostics/security/mod.rs",
  provider: "raw owner checks",
  astAware: "visitor",
  regionAware: "handler body",
  confidence: "heuristic",
  topic: "seagrass/security.owner-check",
  substringRisk: "low",
  fixture: "raw account tests",
};

const concreteQuickfixRow: QuickfixAuditRow = {
  line: 30,
  diagnosticCode: "anchor-check-cfg",
  coverage: "covered",
  actionEmitter: "`lsp/actions/features.rs`",
  quickfixTags: "`add-anchor-debug-feature`",
  gaps: "none",
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
    expect(failures.join("\n")).toContain("not present in docs/topics.json");
  });

  test("requires emitted diagnostic topics to appear in the audit", () => {
    const failures = auditTopicFailures(
      [concreteRow],
      knownTopics,
      new Set(["seagrass/security.owner-check", "seagrass/security.type-cosplay"]),
    );

    expect(failures).toContain("seagrass/security.type-cosplay missing from docs/diagnostic-audit.md");
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
    expect(failures.join("\n")).toContain("lsp/diagnostics/security/mod.rs:10");
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
      { ...concreteRow, file: "lsp/completions/mod.rs" },
      { ...concreteRow, file: "lsp/actions/mod.rs" },
    ];

    expect(auditCoverageFailures(rows)).toEqual([
      "docs/diagnostic-audit.md must include at least one lsp/hover/ provider row",
    ]);
  });

  test("requires auditable provider source files to have rows", () => {
    const rows = [
      concreteRow,
      { ...concreteRow, file: "lsp/completions/mod.rs" },
      { ...concreteRow, file: "lsp/hover/mod.rs" },
      { ...concreteRow, file: "lsp/actions/mod.rs" },
    ];
    const sourcePaths = [
      "lsp/diagnostics/security/mod.rs",
      "lsp/completions/mod.rs",
      "lsp/completions/account_fields/context.rs",
      "lsp/hover/mod.rs",
      "lsp/actions/mod.rs",
      "lsp/actions/common.rs",
      "lsp/diagnostics/security/tests/raw_account_tests.rs",
    ];

    expect(auditableSourceFailures(rows, sourcePaths)).toEqual([
      "lsp/completions/account_fields/context.rs is missing from docs/diagnostic-audit.md",
    ]);
  });

  test("allows wildcard rows to cover generated provider directories", () => {
    const rows = [
      concreteRow,
      { ...concreteRow, file: "lsp/diagnostics/constraint_shape/*" },
    ];

    expect(
      auditableSourceFailures(rows, [
        "lsp/diagnostics/security/mod.rs",
        "lsp/diagnostics/constraint_shape/pda.rs",
        "lsp/diagnostics/constraint_shape/support.rs",
      ]),
    ).toEqual([]);
  });

  test("parses both audit tables and validates provider paths", () => {
    const text = `
| file | provider | AST-aware | region-aware | confidence | topic | substring risk | fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| \`lsp/diagnostics/security/mod.rs\` | \`owner\` | visitor | handler body | heuristic | \`seagrass/security.owner-check\` | low | owner tests |

| file | provider branch | AST-aware | region-aware | confidence | topic | substring risk | fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| \`lsp/diagnostics/security/raw_account.rs\` | raw evidence | visitor | body | n/a | \`seagrass/security.type-cosplay\` | low | tests |
`;

    const rows = auditRowsFromMarkdown(text);

    expect(rows.map((row) => row.file)).toEqual([
      "lsp/diagnostics/security/mod.rs",
      "lsp/diagnostics/security/raw_account.rs",
    ]);
    expect(
      sourcePathFailures(rows, "/repo/src", new Set(["/repo/src/lsp/diagnostics/security/mod.rs"])),
    ).toEqual(["lsp/diagnostics/security/raw_account.rs does not exist under /repo/src"]);
  });

  test("parses quickfix coverage matrix rows", () => {
    const text = `
| diagnostic code | quickfix coverage | action emitter | quickfix tags | gaps |
| --- | --- | --- | --- | --- |
| \`anchor-check-cfg\` | covered | \`lsp/actions/features.rs\` | \`add-anchor-debug-feature\` | none |
`;

    const rows = quickfixRowsFromMarkdown(text);

    expect(rows).toEqual([
      {
        line: 4,
        diagnosticCode: "anchor-check-cfg",
        coverage: "covered",
        actionEmitter: "`lsp/actions/features.rs`",
        quickfixTags: "`add-anchor-debug-feature`",
        gaps: "none",
      },
    ]);
  });

  test("requires quickfix matrix coverage for diagnostic codes and tags", () => {
    const failures = quickfixCoverageFailures(
      [concreteQuickfixRow],
      new Set(["anchor-check-cfg", "anchor-project-id"]),
      new Set(["add-anchor-debug-feature", "sync-declare-id"]),
      new Set(["lsp/actions/features.rs"]),
    );

    expect(failures).toContain("anchor-project-id missing from quickfix coverage matrix");
    expect(failures).toContain(
      "sync-declare-id quickfix tag missing from quickfix coverage matrix",
    );
  });

  test("rejects stale quickfix matrix rows", () => {
    const failures = quickfixCoverageFailures(
      [
        {
          ...concreteQuickfixRow,
          diagnosticCode: "anchor-unknown",
          coverage: "maybe",
          actionEmitter: "`lsp/diagnostics/check_cfg/mod.rs`",
          quickfixTags: "`missing-tag`",
          gaps: "none",
        },
        {
          ...concreteQuickfixRow,
          diagnosticCode: "anchor-project-id",
          coverage: "gap",
          actionEmitter: "n/a",
          quickfixTags: "none",
          gaps: "none",
        },
      ],
      new Set(["anchor-check-cfg", "anchor-project-id"]),
      new Set(["add-anchor-debug-feature"]),
      new Set(["lsp/actions/features.rs"]),
    ).join("\n");

    expect(failures).toContain("unknown diagnostic code");
    expect(failures).toContain("quickfix coverage must be covered, partial, guidance, or gap");
    expect(failures).toContain("unknown quickfix tag missing-tag");
    expect(failures).toContain("action emitter lsp/diagnostics/check_cfg/mod.rs does not exist");
    expect(failures).toContain("action emitter lsp/diagnostics/check_cfg/mod.rs must live under lsp/actions/");
    expect(failures).toContain("gap rows must name the missing quickfix");
    expect(failures).toContain("anchor-check-cfg missing from quickfix coverage matrix");
  });
});
