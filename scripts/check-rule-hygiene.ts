#!/usr/bin/env bun

import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const diagnosticsRoot = resolve(repoRoot, "src/diagnostics");

type Pattern = {
  name: string;
  regex: RegExp;
};

type Finding = {
  relativePath: string;
  lineNumber: number;
  pattern: string;
  text: string;
};

type SourceLine = {
  text: string;
  lineNumber: number;
};

const BANNED_PATTERNS: Pattern[] = [
  {
    name: "raw-source-lines",
    regex: /\bsource\.lines\(\)\./,
  },
  {
    name: "raw-match-indices",
    regex: /\.match_indices\(/,
  },
  {
    name: "raw-contains",
    regex:
      /\b(?:source|text|line|key|value|constraint|parser_message|diagnostic\.message|uncommented)\.contains\("[a-z0-9_:#./ \[\](){},=<>!&-]+"\)/,
  },
  {
    name: "raw-source-alias-contains",
    regex:
      /\b(?:source_text|source_slice|source_line|raw_source|raw_text|raw_line)\.contains\("[a-z0-9_:#./ \[\](){},=<>!&-]+"\)/,
  },
  {
    name: "token-stream-contains",
    regex: /\bexpr_text\([^)]*\)\.contains\("[a-z0-9_:#./ \[\](){},=<>!&-]+"\)/,
  },
  {
    name: "raw-rfind",
    regex: /\.rfind\((?:"[^"]+"|'[^']+')\)/,
  },
];

if (import.meta.main) {
  const findings = checkRuleHygiene();
  if (findings.length > 0) {
    console.error("Seagrass diagnostic rule hygiene failed.");
    console.error("Raw source scans must be replaced with AST, parsed syntax, or explicit helpers.");
    console.error(formatFindings(findings));
    process.exit(1);
  }

  console.log("seagrass rule hygiene check passed: no legacy findings");
}

export function checkRuleHygiene(): Finding[] {
  return rustFiles(diagnosticsRoot).flatMap(scanFile);
}

export function scanDiagnosticSource(relativePath: string, source: string): Finding[] {
  if (isTestFile(relativePath)) {
    return [];
  }
  return productionLines(source).flatMap(({ text, lineNumber }) =>
    BANNED_PATTERNS.filter((pattern) => pattern.regex.test(text)).map((pattern) => ({
      relativePath,
      lineNumber,
      pattern: pattern.name,
      text: text.trim(),
    })),
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

function scanFile(path: string): Finding[] {
  const relativePath = relative(repoRoot, path);
  return scanDiagnosticSource(relativePath, readFileSync(path, "utf8"));
}

function isTestFile(relativePath: string): boolean {
  return (
    relativePath.endsWith("_tests.rs") ||
    relativePath.endsWith("/tests.rs") ||
    relativePath.includes("/tests/")
  );
}

function productionLines(source: string): SourceLine[] {
  const lines = source.split("\n");
  const result: SourceLine[] = [];
  let pendingTestAttribute = false;
  let testModuleBraceDepth = 0;

  lines.forEach((text, index) => {
    const lineNumber = index + 1;
    const trimmed = text.trim();

    if (testModuleBraceDepth > 0) {
      testModuleBraceDepth += braceDelta(text);
      return;
    }

    if (trimmed === "#[cfg(test)]") {
      pendingTestAttribute = true;
      return;
    }

    if (pendingTestAttribute && trimmed.startsWith("mod ") && trimmed.includes("{")) {
      pendingTestAttribute = false;
      testModuleBraceDepth = braceDelta(text);
      return;
    }

    if (!trimmed.startsWith("#[")) {
      pendingTestAttribute = false;
    }

    result.push({ text, lineNumber });
  });

  return result;
}

function braceDelta(line: string): number {
  let delta = 0;
  for (const char of line) {
    if (char === "{") {
      delta += 1;
    } else if (char === "}") {
      delta -= 1;
    }
  }
  return delta;
}

function formatFindings(findingsToFormat: Finding[]): string {
  return findingsToFormat
    .map(
      (finding) =>
        `${finding.relativePath}:${finding.lineNumber} ${finding.pattern}: ${finding.text}`,
    )
    .join("\n");
}
