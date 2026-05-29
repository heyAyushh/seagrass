#!/usr/bin/env bun

import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const diagnosticsRoot = resolve(repoRoot, "src/lsp/diagnostics");
const topicsPath = resolve(repoRoot, "docs/topics.json");
export const topicPattern = /^seagrass\/[a-z0-9][a-z0-9.-]*$/;
const sourceTopicEvidenceRegexes = [
  /\bconst\s+[A-Z0-9_]*TOPIC[A-Z0-9_]*\s*:\s*&\s*(?:'static\s+)?str\s*=\s*"(?<topic>seagrass\/[a-z0-9][a-z0-9.-]*)"/g,
  /\bSelf::[A-Za-z0-9_]+\s*=>\s*"(?<topic>seagrass\/[a-z0-9][a-z0-9.-]*)"/g,
  /"topic"\s*:\s*"(?<topic>seagrass\/[a-z0-9][a-z0-9.-]*)"/g,
];

type TopicEntry = {
  name: string;
  description: string;
};

type TopicManifest = {
  schemaVersion: 1;
  topics: TopicEntry[];
};

if (import.meta.main) {
  try {
    const result = checkDiagnosticTopics();
    if (result.failures.length > 0) {
      fail(result.failures.join("\n"));
    }
    console.log(`seagrass diagnostic topic check passed: ${result.topicCount} topics`);
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }
}

export function checkDiagnosticTopics(): { failures: string[]; topicCount: number } {
  const manifest = parseManifest(readFileSync(topicsPath, "utf8"));
  const manifestTopics = manifest.topics.map((topic) => topic.name);
  const sourceTopics = Array.from(sourceDiagnosticTopics()).sort(compareStrings);
  const failures = topicManifestFailures(manifest.topics, sourceTopics);
  return { failures, topicCount: manifestTopics.length };
}

export function parseManifest(text: string): TopicManifest {
  const parsed: unknown = JSON.parse(text);
  if (!isRecord(parsed)) {
    throw new Error("topics manifest must be a JSON object");
  }
  if (parsed.schemaVersion !== 1) {
    throw new Error("topics manifest schemaVersion must be 1");
  }
  if (!Array.isArray(parsed.topics) || !parsed.topics.every(isTopicEntry)) {
    throw new Error("topics manifest must contain topic entries with name and description");
  }
  return {
    schemaVersion: 1,
    topics: parsed.topics,
  };
}

function isTopicEntry(value: unknown): value is TopicEntry {
  return (
    isRecord(value) &&
    typeof value.name === "string" &&
    typeof value.description === "string" &&
    value.description.trim().length > 0
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function topicManifestFailures(topics: TopicEntry[], sourceTopics: string[]): string[] {
  const manifestTopics = topics.map((topic) => topic.name);
  return [
    ...sortedUniqueFailures(manifestTopics),
    ...topicShapeFailures(topics),
    ...topicDriftFailures(manifestTopics, sourceTopics),
  ];
}

export function sortedUniqueFailures(topics: string[]): string[] {
  const sorted = [...topics].sort(compareStrings);
  const failures: string[] = [];
  if (topics.join("\n") !== sorted.join("\n")) {
    failures.push("topics manifest must be sorted by name");
  }
  const duplicates = topics.filter((topic, index) => topics.indexOf(topic) !== index);
  if (duplicates.length > 0) {
    failures.push(`topics manifest contains duplicate topics:\n${duplicates.join("\n")}`);
  }
  return failures;
}

export function topicShapeFailures(topics: TopicEntry[]): string[] {
  const invalid = topics
    .map((topic) => topic.name)
    .filter((topic) => !topicPattern.test(topic));
  if (invalid.length > 0) {
    return [`topics must match ${topicPattern}:\n${invalid.join("\n")}`];
  }
  return [];
}

export function topicDriftFailures(manifestTopics: string[], sourceTopics: string[]): string[] {
  const manifestSet = new Set(manifestTopics);
  const sourceSet = new Set(sourceTopics);
  const missing = sourceTopics.filter((topic) => !manifestSet.has(topic));
  const stale = manifestTopics.filter((topic) => !sourceSet.has(topic));

  if (missing.length === 0 && stale.length === 0) {
    return [];
  }

  return [
    [
      "diagnostic topic manifest drift detected",
      formatTopicList("missing from docs/topics.json", missing),
      formatTopicList("declared but not emitted by diagnostics source", stale),
    ].join("\n"),
  ];
}

function sourceDiagnosticTopics(): Set<string> {
  const topics = new Set<string>();
  for (const file of rustFiles(diagnosticsRoot).filter(isProductionDiagnosticSourcePath)) {
    const text = readFileSync(file, "utf8");
    for (const topic of sourceTopicsFromText(text)) {
      topics.add(topic);
    }
  }
  return topics;
}

export function sourceTopicsFromText(text: string): string[] {
  return [
    ...new Set(
      sourceTopicEvidenceRegexes.flatMap((regex) =>
        [...text.matchAll(regex)].flatMap((match) => match.groups?.topic ?? []),
      ),
    ),
  ].sort(compareStrings);
}

export function isProductionDiagnosticSourcePath(path: string): boolean {
  const relativePath = relative(diagnosticsRoot, path).replaceAll("\\", "/");
  return (
    relativePath.endsWith(".rs") &&
    !relativePath.endsWith("_tests.rs") &&
    !relativePath.endsWith("/tests.rs") &&
    !relativePath.includes("/tests/")
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

function formatTopicList(label: string, topics: string[]): string {
  if (topics.length === 0) {
    return `${label}: none`;
  }
  return `${label}:\n${topics.map((topic) => `  - ${topic}`).join("\n")}`;
}

function compareStrings(left: string, right: string): number {
  return left.localeCompare(right);
}

function fail(message: string): never {
  console.error(`\nseagrass diagnostic topic check failed:\n${message}`);
  process.exit(1);
}
