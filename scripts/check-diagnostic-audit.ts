#!/usr/bin/env bun

import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const sourceRoot = resolve(repoRoot, "src");
const cratesRoot = resolve(repoRoot, "crates");
const diagnosticsRoot = resolve(sourceRoot, "lsp/diagnostics");
const frameworkNativeRulesRoot = resolve(cratesRoot, "seagrass-framework/src/native_rules");
const registryPath = resolve(diagnosticsRoot, "registry.rs");
const auditPath = resolve(repoRoot, "docs/diagnostic-audit.md");
const topicsPath = resolve(repoRoot, "docs/topics.json");
const tableColumnCount = 8;
const quickfixTableColumnCount = 5;
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
const diagnosticCodeConstRegex =
  /\bpub\s+const\s+[A-Z0-9_]+_CODE\s*:\s*&\s*(?:'static\s+)?str\s*=\s*"(?<code>[^"]+)"/g;
const rustStringConstRegex =
  /\b(?:pub\s+)?const\s+(?<name>[A-Z0-9_]+)\s*:\s*&\s*(?:'static\s+)?str\s*=\s*"(?<value>[^"]+)"/g;
const diagnosticQuickfixRegex =
  /"quickfix"\s*:\s*(?:"(?<literal>[^"]+)"|(?<identifier>[A-Z0-9_]+))/g;
const quickfixTagRegex = /`(?<tag>[^`]+)`/g;
const requiredProviderPrefixes = [
  "lsp/diagnostics/",
  "lsp/completions/",
  "lsp/hover/",
  "lsp/actions/",
];
const quickfixCoverageValues = new Set(["covered", "partial", "guidance", "gap"]);
const specialQuickfixTags = new Set([
  "code-routed",
  "none",
  "parser-rule:duplicate",
  "parser-rule:ordering",
]);
const providerSourceIgnorePatterns = [
  /(^|\/)tests?(\/|\.rs$)/,
  /(^|\/)[a-z0-9_]+_tests(\/|\.rs$)/,
  /(^|\/)common\.rs$/,
  /(^|\/)mod\.rs$/,
  /(^|\/)support\.rs$/,
  /^lsp\/diagnostics\/arbitration\.rs$/,
  /^lsp\/diagnostics\/engine\.rs$/,
  /^lsp\/diagnostics\/lint\.rs$/,
  /^lsp\/diagnostics\/registry\.rs$/,
  /^lsp\/diagnostics\/rules\.rs$/,
  /^lsp\/diagnostics\/suppression\.rs$/,
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

export type QuickfixAuditRow = {
  line: number;
  diagnosticCode: string;
  coverage: string;
  actionEmitter: string;
  quickfixTags: string;
  gaps: string;
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
  const quickfixRows = quickfixRowsFromMarkdown(auditText);
  const manifestTopics = parseManifestTopics(readFileSync(topicsPath, "utf8"));
  const emittedTopics = sourceDiagnosticTopics(diagnosticsRoot, [frameworkNativeRulesRoot]);
  const diagnosticCodes = registryDiagnosticCodes(registryPath);
  const emittedQuickfixTags = sourceDiagnosticQuickfixTags(sourceRoot, diagnosticsRoot, [
    frameworkNativeRulesRoot,
  ]);
  const coverage = auditCoverageFromSourceRoots([sourceRoot, cratesRoot]);
  const existingPaths = new Set(rustFiles(sourceRoot));
  const existingWorkspacePaths = new Set([...existingPaths, ...rustFiles(cratesRoot)]);
  const sourcePaths = [...existingPaths]
    .map((path) => path.slice(`${sourceRoot}${sep}`.length).replaceAll(sep, "/"))
    .sort(compareStrings);
  const failures = [
    ...sourcePathFailures(rows, sourceRoot, existingPaths, repoRoot, existingWorkspacePaths),
    ...auditTopicFailures(rows, manifestTopics, emittedTopics),
    ...auditCellFailures(rows, coverage),
    ...auditCoverageFailures(rows),
    ...auditableSourceFailures(rows, sourcePaths),
    ...quickfixCoverageFailures(
      quickfixRows,
      diagnosticCodes,
      emittedQuickfixTags,
      new Set(sourcePaths),
    ),
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

export function quickfixRowsFromMarkdown(text: string): QuickfixAuditRow[] {
  const rows: QuickfixAuditRow[] = [];
  let insideQuickfixTable = false;

  for (const [index, line] of text.split(/\r?\n/).entries()) {
    const cells = splitMarkdownRow(line);
    if (cells.length === 0) {
      if (insideQuickfixTable) {
        break;
      }
      continue;
    }

    if (isQuickfixHeaderRow(cells)) {
      insideQuickfixTable = true;
      continue;
    }
    if (!insideQuickfixTable) {
      continue;
    }
    if (stripMarkdown(cells[0]) === "---") {
      continue;
    }
    if (cells.length !== quickfixTableColumnCount) {
      break;
    }

    rows.push({
      line: index + 1,
      diagnosticCode: stripMarkdown(cells[0]),
      coverage: stripMarkdown(cells[1]),
      actionEmitter: cells[2].trim(),
      quickfixTags: cells[3].trim(),
      gaps: stripMarkdown(cells[4]),
    });
  }

  return rows;
}

export function quickfixCoverageFailures(
  rows: QuickfixAuditRow[],
  diagnosticCodes: Set<string>,
  emittedQuickfixTags: Set<string>,
  sourcePaths: Set<string>,
): string[] {
  const failures: string[] = [];
  if (rows.length === 0) {
    failures.push("docs/diagnostic-audit.md must include a quickfix coverage matrix");
    return failures;
  }

  const seen = new Map<string, QuickfixAuditRow[]>();
  for (const row of rows) {
    seen.set(row.diagnosticCode, [...(seen.get(row.diagnosticCode) ?? []), row]);
    failures.push(
      ...quickfixRowFailures(row, diagnosticCodes, emittedQuickfixTags, sourcePaths),
    );
  }

  for (const code of [...diagnosticCodes].sort(compareStrings)) {
    if (!seen.has(code)) {
      failures.push(`${code} missing from quickfix coverage matrix`);
    }
  }
  for (const [code, matches] of seen) {
    if (matches.length > 1) {
      failures.push(`${code} appears multiple times in quickfix coverage matrix`);
    }
  }

  const auditedTags = new Set(rows.flatMap((row) => quickfixTags(row.quickfixTags)));
  for (const tag of [...emittedQuickfixTags].sort(compareStrings)) {
    if (!auditedTags.has(tag)) {
      failures.push(`${tag} quickfix tag missing from quickfix coverage matrix`);
    }
  }

  return failures;
}

export function sourcePathFailures(
  rows: AuditRow[],
  root: string,
  existingSourcePaths: Set<string>,
  workspaceRoot = root,
  existingWorkspacePaths = existingSourcePaths,
): string[] {
  return rows.flatMap((row) => {
    const isWorkspaceRelative = row.file.startsWith("crates/");
    const rowRoot = isWorkspaceRelative ? workspaceRoot : root;
    const existingPaths = isWorkspaceRelative ? existingWorkspacePaths : existingSourcePaths;
    if (row.file.endsWith("/*")) {
      return wildcardPathFailures(row.file, rowRoot, existingPaths);
    }
    const resolvedPath = resolve(rowRoot, row.file);
    if (!isInsideRoot(rowRoot, resolvedPath)) {
      return [`${row.file} escapes ${rowRoot}`];
    }
    return existingPaths.has(resolvedPath)
      ? []
      : [`${row.file} does not exist under ${rowRoot}`];
  });
}

export function registryDiagnosticCodes(path: string): Set<string> {
  return new Set(
    [...readFileSync(path, "utf8").matchAll(diagnosticCodeConstRegex)]
      .flatMap((match) => match.groups?.code ?? [])
      .sort(compareStrings),
  );
}

export function sourceDiagnosticQuickfixTags(
  sourceRootPath: string,
  diagnosticsRootPath: string,
  extraDiagnosticRootPaths: string[] = [],
): Set<string> {
  const constants = rustStringConstants(sourceRootPath);
  for (const rootPath of extraDiagnosticRootPaths) {
    for (const [name, value] of rustStringConstants(rootPath)) {
      constants.set(name, value);
    }
  }
  const tags = new Set<string>();

  for (const rootPath of [diagnosticsRootPath, ...extraDiagnosticRootPaths]) {
    for (const file of rustFiles(rootPath).filter((path) =>
      isProductionDiagnosticSourcePath(rootPath, path),
    )) {
      const text = readFileSync(file, "utf8");
      for (const match of text.matchAll(diagnosticQuickfixRegex)) {
        const literal = match.groups?.literal;
        const identifier = match.groups?.identifier;
        const tag = literal ?? (identifier ? constants.get(identifier) : undefined);
        if (tag) {
          tags.add(tag);
        }
      }
    }
  }

  return tags;
}

function rustStringConstants(root: string): Map<string, string> {
  const constants = new Map<string, string>();
  for (const file of rustFiles(root)) {
    const text = readFileSync(file, "utf8");
    for (const match of text.matchAll(rustStringConstRegex)) {
      const name = match.groups?.name;
      const value = match.groups?.value;
      if (name && value) {
        constants.set(name, value);
      }
    }
  }
  return constants;
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
      .map((topic) => `${topic} missing from docs/diagnostic-audit.md`),
  ];
}

export function auditCoverageFailures(rows: AuditRow[]): string[] {
  return requiredProviderPrefixes.flatMap((prefix) =>
    rows.some((row) => row.file.startsWith(prefix))
      ? []
      : [`docs/diagnostic-audit.md must include at least one ${prefix} provider row`],
  );
}

export function auditableSourceFailures(rows: AuditRow[], sourcePaths: string[]): string[] {
  return sourcePaths
    .filter(isAuditableProviderSource)
    .filter((sourcePath) => !auditRowsCoverSourcePath(rows, sourcePath))
    .map((sourcePath) => `${sourcePath} is missing from docs/diagnostic-audit.md`);
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

function isQuickfixHeaderRow(cells: string[]): boolean {
  return (
    cells.length === quickfixTableColumnCount &&
    stripMarkdown(cells[0]) === "diagnostic code" &&
    stripMarkdown(cells[1]) === "quickfix coverage"
  );
}

function quickfixRowFailures(
  row: QuickfixAuditRow,
  diagnosticCodes: Set<string>,
  emittedQuickfixTags: Set<string>,
  sourcePaths: Set<string>,
): string[] {
  const failures: string[] = [];
  const coverage = row.coverage.trim().toLowerCase();
  const tags = quickfixTags(row.quickfixTags);

  if (!diagnosticCodes.has(row.diagnosticCode)) {
    failures.push(`${quickfixRowLabel(row)}: unknown diagnostic code`);
  }
  if (!quickfixCoverageValues.has(coverage)) {
    failures.push(
      `${quickfixRowLabel(row)}: quickfix coverage must be covered, partial, guidance, or gap`,
    );
  }
  if (coverage === "gap") {
    if (normalizedCell(row.gaps) === "none") {
      failures.push(`${quickfixRowLabel(row)}: gap rows must name the missing quickfix`);
    }
    if (normalizedCell(row.quickfixTags) !== "none") {
      failures.push(`${quickfixRowLabel(row)}: gap rows must use none for quickfix tags`);
    }
    return failures;
  }

  if (tags.length === 0 || normalizedCell(row.quickfixTags) === "none") {
    failures.push(`${quickfixRowLabel(row)}: non-gap rows must name quickfix tags`);
  }
  for (const tag of tags) {
    if (!specialQuickfixTags.has(tag) && !emittedQuickfixTags.has(tag)) {
      failures.push(`${quickfixRowLabel(row)}: unknown quickfix tag ${tag}`);
    }
  }
  if (normalizedCell(row.actionEmitter) === "n/a") {
    failures.push(`${quickfixRowLabel(row)}: non-gap rows must name an action emitter`);
  }
  for (const path of actionEmitterPaths(row.actionEmitter)) {
    if (!sourcePaths.has(path)) {
      failures.push(`${quickfixRowLabel(row)}: action emitter ${path} does not exist`);
    }
    if (!path.startsWith("lsp/actions/")) {
      failures.push(`${quickfixRowLabel(row)}: action emitter ${path} must live under lsp/actions/`);
    }
  }
  if (coverage === "covered" && normalizedCell(row.gaps) !== "none") {
    failures.push(`${quickfixRowLabel(row)}: covered rows must use none for gaps`);
  }

  return failures;
}

function quickfixTags(value: string): string[] {
  return [
    ...new Set([...value.matchAll(quickfixTagRegex)].flatMap((match) => match.groups?.tag ?? [])),
  ].sort(compareStrings);
}

function actionEmitterPaths(value: string): string[] {
  return [...new Set(quickfixTags(value).filter((tag) => tag.endsWith(".rs")))].sort(
    compareStrings,
  );
}

function normalizedCell(value: string): string {
  return value.trim().toLowerCase();
}

function quickfixRowLabel(row: QuickfixAuditRow): string {
  return `${row.diagnosticCode}:${row.line}`;
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
    failures.push(
      `${row.file} ${row.provider}: topic cell must use concrete seagrass topics or n/a`,
    );
  }
  failures.push(
    ...topics
      .filter((topic) => !manifestTopics.has(topic))
      .map((topic) => `${row.file} ${row.provider}: ${topic} is not present in docs/topics.json`),
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

function auditCoverageFromSourceRoots(roots: string[]): AuditCoverage {
  return {
    rustTestNames: new Set(
      roots.flatMap((root) =>
        rustFiles(root).flatMap((path) => rustTestFunctionNames(readFileSync(path, "utf8"))),
      ),
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

function sourceDiagnosticTopics(root: string, extraRoots: string[] = []): Set<string> {
  const topics = new Set<string>();
  for (const rootPath of [root, ...extraRoots]) {
    for (const file of rustFiles(rootPath).filter((path) =>
      isProductionDiagnosticSourcePath(rootPath, path),
    )) {
      const text = readFileSync(file, "utf8");
      for (const topic of sourceTopicsFromText(text)) {
        topics.add(topic);
      }
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
