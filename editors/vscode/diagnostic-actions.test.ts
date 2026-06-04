import { describe, expect, test } from "bun:test";

import {
  formatFalsePositiveReport,
  lintDocUrlFromTopic,
  parseDiagnosticMetadata,
  suppressionSnippet,
  topicSlug,
} from "./src/diagnosticActions.ts";

describe("diagnostic actions", () => {
  test("parses complete server metadata summaries", () => {
    const metadata = parseDiagnosticMetadata([
      "Seagrass confidence: heuristic; topic: seagrass/security.owner-check; applicability: MachineApplicable; quickfix: add-owner-check",
    ]);

    expect(metadata).toEqual({
      confidence: "heuristic",
      topic: "seagrass/security.owner-check",
      applicability: "MachineApplicable",
      quickfix: "add-owner-check",
    });
  });

  test("builds lint doc links and supported suppression syntax", () => {
    expect(topicSlug("seagrass/security.owner-check")).toBe("seagrass-security-owner-check");
    expect(lintDocUrlFromTopic("seagrass/security.owner-check")).toBe(
      "https://github.com/heyAyushh/seagrass/blob/main/docs/lints/seagrass-security-owner-check.md",
    );
    expect(suppressionSnippet("seagrass/security.owner-check")).toBe(
      "// seagrass-allow: seagrass/security.owner-check",
    );
  });

  test("formats false-positive reports with actionable context", () => {
    const report = formatFalsePositiveReport({
      file: "programs/demo/src/lib.rs",
      range: "12:5-12:19",
      message: "owner check is missing",
      code: "seagrass/security.owner-check",
      docsUrl: "https://example.test/lint.md",
      metadata: {
        confidence: "heuristic",
        topic: "seagrass/security.owner-check",
        applicability: "MachineApplicable",
        quickfix: "add-owner-check",
      },
      sourceLine: "let account = &ctx.accounts.account;",
    });

    expect(report).toContain("topic: seagrass/security.owner-check");
    expect(report).toContain("quickfix: add-owner-check");
    expect(report).toContain("source:\nlet account = &ctx.accounts.account;");
  });
});
