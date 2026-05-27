#!/usr/bin/env bun

import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");
const sourceRoot = resolve(repoRoot, "lsp/src");
const diagnosticsRoot = resolve(sourceRoot, "diagnostics");
const auditPath = resolve(repoRoot, "lsp/docs/diagnostic-audit.md");
const topicsPath = resolve(repoRoot, "lsp/docs/topics.json");
const tableColumnCount = 8;
const sourceTopicEvidenceRegexes = [
  /\bconst\s+[A-Z0-9_]*TOPIC[A-Z0-9_]*\s*:\s*&\s*(?:'static\s+)?str\s*=\s*"(?<topic>seagrass\/[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?)"/g,
  /\bSelf::[A-Za-z0-9_]+\s*=>\s*"(?<topic>seagrass\/[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?)"/g,
  /"topic"\s*:\s*"(?<topic>seagrass\/[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?)"/g,
];
const auditTopicRegex = /seagrass\/[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?/g;
const astEvidencePattern =
  /\b(AST|Anchor\.toml|CursorContext|RankingContext|visitor|Visit|LintVisitor|parsed|parser|tree-sitter|generated|workspace|metadata|manifest|catalog|evidence|symbols|diagnostic data|structured|project|artifact|ecosystem)\b/i;
const vagueAuditValues = new Set(["yes", "no", "none", "unknown", "tbd", "todo"]);
const fixedSubstringRiskPrefix = "fixed:";
const genericFixtureValues = new Set(["n/a", "none", "tests"]);
const falsePositiveFixtureNameRegex =
  /\b(?:ignores_[a-z0-9_]+|does_not_[a-z0-9_]+|[a-z0-9_]+_does_not_[a-z0-9_]+|accepts_[a-z0-9_]+|allows_[a-z0-9_]+|skips_[a-z0-9_]+)\b/gi;
const rustTestFunctionRegex = /#\s*\[\s*test\s*\][\s\S]*?\bfn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(/g;
const requiredProviderPrefixes = ["diagnostics/", "completions/", "hover/", "actions/"];
const providerSourceIgnorePatterns = [
  /(^|\/)tests?(\/|\.rs$)/,
  /(^|\/)[a-z0-9_]+_tests(\/|\.rs$)/,
  /(^|\/)common\.rs$/,
  /(^|\/)mod\.rs$/,
  /(^|\/)support\.rs$/,
  /^diagnostics\/arbitration\.rs$/,
  /^diagnostics\/engine\.rs$/,
  /^diagnostics\/lint\.rs$/,
  /^diagnostics\/registry\.rs$/,
  /^diagnostics\/rules\.rs$/,
  /^diagnostics\/suppression\.rs$/,
];

export type AuditRow = {
  line: number;
  file: string;
  provider: string;
  astAware: string;
  regionAware: string;
  confidence: string;
  topic: string;
  substringRisk: string;
  fixture: string;
};

export type AuditCoverage = {
  rustTestNames: Set<string>;
};

type TopicEntry = {
  name: string;
  description: string;
};

type TopicManifest = {
  topics: TopicEntry[];
};

if (import.meta.main) {
  const result = checkDiagnosticAudit();
  if (result.failures.length > 0) {
    console.error(`\nseagrass diagnostic audit check failed:\n${result.failures.join("\n")}`);
    process.exit(1);
  }

  console.log(`seagrass diagnostic audit check passed: ${result.rowCount} rows`);
}

export function checkDiagnosticAudit(): { failures: string[]; rowCount: number } {
  const auditText = readFileSync(auditPath, "utf8");
  const rows = auditRowsFromMarkdown(auditText);
  const manifestTopics = parseManifestTopics(readFileSync(topicsPath, "utf8"));
  const emittedTopics = sourceDiagnosticTopics(diagnosticsRoot);
  const coverage = auditCoverageFromSource(sourceRoot);
  const existingPaths = new Set(rustFiles(sourceRoot));
  const sourcePaths = [...existingPaths]
    .map((path) => path.slice(`${sourceRoot}${sep}`.length).replaceAll(sep, "/"))
    .sort(compareStrings);
  const failures = [
    ...sourcePathFailures(rows, sourceRoot, existingPaths),
    ...auditTopicFailures(rows, manifestTopics, emittedTopics),
    ...auditCellFailures(rows, coverage),
    ...auditCoverageFailures(rows),
    ...auditableSourceFailures(rows, sourcePaths),
  ];

  if (rows.length === 0) {
    failures.push(`${auditPath} has no audit table rows`);
  }

  return { failures, rowCount: rows.length };
}

export function auditCellFailures(rows: AuditRow[], coverage?: AuditCoverage): string[] {
  return rows.flatMap((row) => [
    ...requiredDetailFailures(row, "AST-aware", row.astAware, astEvidencePattern),
    ...requiredDetailFailures(row, "region-aware", row.regionAware, /\b(n\/a|attribute|attributes|attrs|body|bodies|constraint|constraints|field|fields|range|ranges|span|spans|signature|signatures|struct|structs|manifest|project|file|files|syntax|token|tokens|workspace|declaration|declarations|usage|usages|cursor|request|completion|diagnostic|cross-provider|per-function)\b/i),
    ...confidenceCellFailures(row),
    ...substringRiskFailures(row),
    ...fixtureCellFailures(row, coverage),
  ]);
}

export function auditRowsFromMarkdown(text: string): AuditRow[] {
  return text
    .split(/\r?\n/)
    .map((line, index) => ({ line: index + 1, cells: splitMarkdownRow(line) }))
    .filter((row) => row.cells.length >= tableColumnCount)
    .filter((row) => isAuditDataRow(row.cells))
    .map(({ line, cells }) => ({
      line,
      file: stripMarkdown(cells[0]),
      provider: stripMarkdown(cells[1]),
      astAware: stripMarkdown(cells[2]),
      regionAware: stripMarkdown(cells[3]),
      confidence: stripMarkdown(cells[4]),
      topic: stripMarkdown(cells[5]),
      substringRisk: stripMarkdown(cells[6]),
      fixture: stripMarkdown(cells[7]),
    }));
}

export function sourcePathFailures(
  rows: AuditRow[],
  root: string,
  existingSourcePaths: Set<string>,
): string[] {
  return rows.flatMap((row) => {
    if (row.file.endsWith("/*")) {
      return wildcardPathFailures(row.file, root, existingSourcePaths);
    }
    const resolvedPath = resolve(root, row.file);
    if (!isInsideRoot(root, resolvedPath)) {
      return [`${row.file} escapes ${root}`];
    }
    return existingSourcePaths.has(resolvedPath) ? [] : [`${row.file} does not exist under ${root}`];
  });
}

export function auditTopicFailures(
  rows: AuditRow[],
  manifestTopics: Set<string>,
  emittedTopics: Set<string>,
): string[] {
  const failures = rows.flatMap((row) => rowTopicFailures(row, manifestTopics));
  const auditedTopics = new Set(rows.flatMap((row) => topicTokens(row.topic)));
  return [
    ...failures,
    ...[...emittedTopics]
      .filter((topic) => !auditedTopics.has(topic))
      .sort(compareStrings)
      .map((topic) => `${topic} missing from lsp/docs/diagnostic-audit.md`),
  ];
}

export function auditCoverageFailures(rows: AuditRow[]): string[] {
  return requiredProviderPrefixes.flatMap((prefix) =>
    rows.some((row) => row.file.startsWith(prefix))
      ? []
      : [`lsp/docs/diagnostic-audit.md must include at least one ${prefix} provider row`],
  );
}

export function auditableSourceFailures(rows: AuditRow[], sourcePaths: string[]): string[] {
  return sourcePaths
    .filter(isAuditableProviderSource)
    .filter((sourcePath) => !auditRowsCoverSourcePath(rows, sourcePath))
    .map((sourcePath) => `${sourcePath} is missing from lsp/docs/diagnostic-audit.md`);
}

function isAuditableProviderSource(sourcePath: string): boolean {
  return (
    requiredProviderPrefixes.some((prefix) => sourcePath.startsWith(prefix)) &&
    !providerSourceIgnorePatterns.some((pattern) => pattern.test(sourcePath))
  );
}

function auditRowsCoverSourcePath(rows: AuditRow[], sourcePath: string): boolean {
  return rows.some((row) => row.file === sourcePath || wildcardRowCovers(row.file, sourcePath));
}

function wildcardRowCovers(rowPath: string, sourcePath: string): boolean {
  if (!rowPath.endsWith("/*")) {
    return false;
  }
  const directory = rowPath.slice(0, -"/*".length);
  return sourcePath.startsWith(`${directory}/`);
}

function rowTopicFailures(row: AuditRow, manifestTopics: Set<string>): string[] {
  const topicCell = row.topic.trim();
  const topics = topicTokens(topicCell);
  const failures: string[] = [];

  if (topicCell.includes("*")) {
    failures.push(`${row.file} ${row.provider}: wildcard topic cells are not allowed`);
  }
  if (topics.length === 0 && topicCell !== "n/a") {
    failures.push(`${row.file} ${row.provider}: topic cell must use concrete seagrass topics or n/a`);
  }
  failures.push(
    ...topics
      .filter((topic) => !manifestTopics.has(topic))
      .map((topic) => `${row.file} ${row.provider}: ${topic} is not present in lsp/docs/topics.json`),
  );

  return failures;
}

function requiredDetailFailures(
  row: AuditRow,
  field: string,
  value: string,
  evidencePattern: RegExp,
): string[] {
  const normalized = value.trim().toLowerCase();
  if (vagueAuditValues.has(normalized)) {
    return [`${rowLabel(row)}: ${field} cell must name concrete evidence, got ${value}`];
  }
  if (!evidencePattern.test(value)) {
    return [`${rowLabel(row)}: ${field} cell must name concrete evidence, got ${value}`];
  }
  return [];
}

function confidenceCellFailures(row: AuditRow): string[] {
  const value = row.confidence.trim().toLowerCase();
  if (/^(?:n\/a|(?:authoritative|derived|heuristic)(?:\s*[/,]\s*(?:authoritative|derived|heuristic))*)$/.test(value)) {
    return [];
  }
  return [
    `${rowLabel(row)}: confidence must use structured authoritative, derived, heuristic, or n/a tokens, got ${row.confidence}`,
  ];
}

function substringRiskFailures(row: AuditRow): string[] {
  const value = row.substringRisk.trim().toLowerCase();
  if (value === "low" || value === "n/a" || /^fixed:\s*[a-z0-9_.,/ =`'()-]+$/.test(value)) {
    return [];
  }
  return [
    `${rowLabel(row)}: substring risk must be low, n/a, or fixed: <named false-positive class>, got ${row.substringRisk}`,
  ];
}

function fixtureCellFailures(row: AuditRow, coverage?: AuditCoverage): string[] {
  const value = row.fixture.trim().toLowerCase();
  const failures: string[] = [];
  if (value.length === 0 || genericFixtureValues.has(value)) {
    failures.push(`${rowLabel(row)}: fixture cell must name concrete coverage`);
  }
  if (isFixedSubstringRisk(row.substringRisk)) {
    const fixtureNames = falsePositiveFixtureNames(row.fixture);
    if (fixtureNames.length === 0) {
      failures.push(
        `${rowLabel(row)}: fixture cell for fixed substring risk must name a false-positive regression`,
      );
    } else if (coverage) {
      const missingNames = fixtureNames.filter((name) => !coverage.rustTestNames.has(name));
      if (missingNames.length > 0) {
        failures.push(
          `${rowLabel(row)}: fixture cell for fixed substring risk references missing false-positive Rust tests: ${missingNames.join(", ")}`,
        );
      }
    }
  }
  return failures;
}

function isFixedSubstringRisk(value: string): boolean {
  return value.trim().toLowerCase().startsWith(fixedSubstringRiskPrefix);
}

function falsePositiveFixtureNames(value: string): string[] {
  return [...new Set([...value.matchAll(falsePositiveFixtureNameRegex)].map((match) => match[0]))].sort(
    compareStrings,
  );
}

function auditCoverageFromSource(root: string): AuditCoverage {
  return {
    rustTestNames: new Set(
      rustFiles(root).flatMap((path) => rustTestFunctionNames(readFileSync(path, "utf8"))),
    ),
  };
}

function rustTestFunctionNames(text: string): string[] {
  return [...text.matchAll(rustTestFunctionRegex)].map((match) => match[1]).sort(compareStrings);
}

function rowLabel(row: AuditRow): string {
  return `${row.file}:${row.line} ${row.provider}`;
}

function topicTokens(text: string): string[] {
  return [...new Set(text.match(auditTopicRegex) ?? [])].sort(compareStrings);
}

function wildcardPathFailures(
  rowPath: string,
  root: string,
  existingSourcePaths: Set<string>,
): string[] {
  const directory = resolve(root, rowPath.slice(0, -"/*".length));
  if (!isInsideRoot(root, directory)) {
    return [`${rowPath} escapes ${root}`];
  }
  const hasCoveredFile = [...existingSourcePaths].some((sourcePath) =>
    sourcePath.startsWith(`${directory}${sep}`),
  );
  return hasCoveredFile ? [] : [`${rowPath} does not cover any Rust source under ${root}`];
}

function splitMarkdownRow(line: string): string[] {
  const trimmed = line.trim();
  if (!trimmed.startsWith("|") || !trimmed.endsWith("|")) {
    return [];
  }
  return trimmed
    .slice(1, -1)
    .split("|")
    .map((cell) => cell.trim());
}

function isAuditDataRow(cells: string[]): boolean {
  const firstCell = stripMarkdown(cells[0]);
  return firstCell !== "file" && firstCell !== "---" && firstCell.length > 0;
}

function stripMarkdown(text: string): string {
  return text.replaceAll("`", "").trim();
}

function parseManifestTopics(text: string): Set<string> {
  const parsed = JSON.parse(text) as TopicManifest;
  return new Set(parsed.topics.map((topic) => topic.name));
}

function sourceDiagnosticTopics(root: string): Set<string> {
  const topics = new Set<string>();
  for (const file of rustFiles(root).filter((path) => isProductionDiagnosticSourcePath(root, path))) {
    const text = readFileSync(file, "utf8");
    for (const topic of sourceTopicsFromText(text)) {
      topics.add(topic);
    }
  }
  return topics;
}

function sourceTopicsFromText(text: string): string[] {
  return [
    ...new Set(
      sourceTopicEvidenceRegexes.flatMap((regex) =>
        [...text.matchAll(regex)].flatMap((match) => match.groups?.topic ?? []),
      ),
    ),
  ].sort(compareStrings);
}

function isProductionDiagnosticSourcePath(root: string, path: string): boolean {
  const sourcePath = relative(root, path).replaceAll("\\", "/");
  return (
    sourcePath.endsWith(".rs") &&
    !sourcePath.endsWith("_tests.rs") &&
    !sourcePath.endsWith("/tests.rs") &&
    !sourcePath.includes("/tests/")
  );
}

function rustFiles(root: string): string[] {
  return readdirSync(root)
    .map((name) => resolve(root, name))
    .flatMap((path) => {
      if (statSync(path).isDirectory()) {
        return rustFiles(path);
      }
      return path.endsWith(".rs") ? [path] : [];
    });
}

function isInsideRoot(root: string, path: string): boolean {
  return path === root || path.startsWith(`${root}${sep}`);
}

function compareStrings(left: string, right: string): number {
  return left.localeCompare(right);
}
