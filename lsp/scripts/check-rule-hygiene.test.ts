import { describe, expect, test } from "bun:test";

import { scanDiagnosticSource } from "./check-rule-hygiene.ts";

describe("diagnostic rule hygiene checker", () => {
  test("flags raw source scanning patterns in production rules", () => {
    const findings = scanDiagnosticSource(
      "lsp/src/diagnostics/example.rs",
      `
fn rule(source: &str, expr: syn::Expr) {
    let source_text = source;
    let _ = source.lines().filter(|line| line.contains("token")).count();
    let _ = source.match_indices("token").count();
    let _ = source.rfind(",");
    let _ = expr_text(&expr).contains("token");
    let _ = source_text.contains("token");
}
`,
    );

    expect(findings.map((finding) => finding.pattern)).toContain("raw-source-lines");
    expect(findings.map((finding) => finding.pattern)).toContain("raw-contains");
    expect(findings.map((finding) => finding.pattern)).toContain("raw-match-indices");
    expect(findings.map((finding) => finding.pattern)).toContain("raw-rfind");
    expect(findings.map((finding) => finding.pattern)).toContain("token-stream-contains");
    expect(findings.map((finding) => finding.pattern)).toContain("raw-source-alias-contains");
  });

  test("ignores test files and cfg test modules", () => {
    expect(
      scanDiagnosticSource(
        "lsp/src/diagnostics/example_tests.rs",
        'fn test_fixture(source: &str) { let _ = source.lines().count(); }',
      ),
    ).toEqual([]);

    expect(
      scanDiagnosticSource(
        "lsp/src/diagnostics/example.rs",
        `
#[cfg(test)]
mod tests {
    fn test_fixture(source: &str) {
        let _ = source.lines().count();
        let _ = source.match_indices("token").count();
    }
}
`,
      ),
    ).toEqual([]);
  });
});
