#!/usr/bin/env bun

import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const topicsPath = resolve(repoRoot, "docs/topics.json");
const lintsRoot = resolve(repoRoot, "docs/lints");
const fixMode = process.argv.includes("--fix");
const GENERATED_BOILERPLATE = [
  "This page documents the user-facing diagnostic topic.",
  "topic-shaped text in comments or strings",
  "unrelated identifiers that contain topic words",
  "missing semantic evidence for this topic",
  "parsed evidence matching this topic's invariant",
];

type TopicEntry = {
  name: string;
  description: string;
};

type TopicManifest = {
  topics: TopicEntry[];
};

if (import.meta.main) {
  const result = checkLintCatalog({ fixMode });
  if (fixMode && result.fixedCount > 0) {
    console.log(`seagrass lint catalog updated ${result.fixedCount} pages`);
  }

  if (result.failures.length > 0) {
    console.error(`\nseagrass lint catalog check failed:\n${result.failures.join("\n")}`);
    process.exit(1);
  }

  console.log(`seagrass lint catalog check passed: ${result.topicCount} topics`);
}

export function checkLintCatalog(options: { fixMode?: boolean } = {}): {
  failures: string[];
  fixedCount: number;
  topicCount: number;
} {
  const manifest = JSON.parse(readFileSync(topicsPath, "utf8")) as TopicManifest;
  const manifestTopics = manifest.topics.map((topic) => topic.name);
  let fixedCount = 0;
  const failures = [
    ...manifest.topics.flatMap((topic) => {
      const path = resolve(lintsRoot, `${topicSlug(topic.name)}.md`);
      if (!existsSync(path)) {
        if (options.fixMode) {
          mkdirSync(lintsRoot, { recursive: true });
          writeFileSync(path, lintPage(topic));
          fixedCount += 1;
          return [];
        }
        return [`${topic.name}: missing ${path}`];
      }

      const originalText = readFileSync(path, "utf8");
      const fixedText = fixedLintPage(topic, originalText);
      if (options.fixMode && fixedText !== originalText) {
        writeFileSync(path, fixedText);
        fixedCount += 1;
        return [];
      }

      return pageContentFailures(topic, path, originalText);
    }),
  ];

  const pageTopics = lintPages().flatMap((path) => topicsDeclaredByPage(path, failures));
  const documentedTopics = pageTopics.reduce((topicsByName, entry) => {
    topicsByName.set(entry.topic, [...(topicsByName.get(entry.topic) ?? []), entry.path]);
    return topicsByName;
  }, new Map<string, string[]>());

  failures.push(
    ...manifestTopics.flatMap((topic) => {
      const paths = documentedTopics.get(topic) ?? [];
      return paths.length === 1
        ? []
        : [`${topic} must appear in exactly one lint page, found ${paths.length}`];
    }),
  );

  return { failures, fixedCount, topicCount: manifest.topics.length };
}

export function pageContentFailures(topic: TopicEntry, path: string, text: string): string[] {
  return [
    ...requiredTextFailures(topic.name, path, text, [
      `Topic: \`${topic.name}\``,
      "## False-Positive Matrix",
      "## Suppression",
      `// seagrass-allow: ${topic.name}`,
      `// seagrass-allow-file: ${topic.name}`,
      "Item or block suppression:",
      `#[seagrass(allow("${topic.name}"))]`,
      "[lints]",
      `allow = ["${topic.name}"]`,
    ]),
    ...generatedBoilerplateFailures(topic.name, path, text),
  ];
}

function topicSlug(topic: string): string {
  return topic.replace("seagrass/", "seagrass-").replaceAll(".", "-").replaceAll("/", "-");
}

function lintPages(): string[] {
  if (!existsSync(lintsRoot)) {
    return [];
  }
  return readdirSync(lintsRoot)
    .filter((name) => name.endsWith(".md"))
    .map((name) => resolve(lintsRoot, name))
    .sort(compareStrings);
}

function declaredTopics(text: string): string[] {
  const lines = text.split(/\r?\n/);
  const topicLineIndex = lines.findIndex((line) => /^Topics?:/.test(line));
  if (topicLineIndex === -1) {
    return [];
  }
  const topicLines = lines
    .slice(topicLineIndex)
    .slice(0, continuationLineCount(lines.slice(topicLineIndex)));
  return [...new Set(topicLines.join("\n").match(/seagrass\/[a-z0-9][a-z0-9.-]*/g) ?? [])];
}

function continuationLineCount(lines: string[]): number {
  const blankLineAfterFirst = lines.slice(1).findIndex((line) => line.trim() === "");
  return blankLineAfterFirst === -1 ? lines.length : blankLineAfterFirst + 1;
}

function topicsDeclaredByPage(path: string, failures: string[]): { path: string; topic: string }[] {
  const text = readFileSync(path, "utf8");
  const topics = declaredTopics(text);
  if (topics.length !== 1) {
    failures.push(`${path} must declare exactly one Topic line, found ${topics.length}`);
    return [];
  }
  const [topic] = topics;
  const expectedPath = resolve(lintsRoot, `${topicSlug(topic)}.md`);
  if (path !== expectedPath) {
    failures.push(`${path} must be named ${expectedPath}`);
  }
  return [{ path, topic }];
}

export function lintPage(topic: TopicEntry): string {
  return `# ${topicTitle(topic.name)}

Topic: \`${topic.name}\`

Source: \`seagrass\`

## What It Catches

${topic.description}

Seagrass should emit this topic only when parsed Anchor, Solana, workspace, or
artifact evidence proves this specific invariant. The diagnostic must not be
derived from raw substring matches.

## What It Does Not Catch

- comments, doc comments, string literals, or unrelated attribute text
- examples where the required semantic evidence is absent or ambiguous
- project states outside this topic's diagnostic contract

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| \`${topic.name}\` appears only in a comment, doc comment, or string literal | no diagnostic |
| an unrelated identifier contains words from this topic | no diagnostic |
| the parsed semantic evidence for this invariant is absent | no diagnostic |
| parsed evidence satisfies ${topicInvariant(topic.name)} | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

\`\`\`rust
// seagrass-allow: ${topic.name}
\`\`\`

File suppression:

\`\`\`rust
// seagrass-allow-file: ${topic.name}
\`\`\`

Item or block suppression:

\`\`\`rust
#[seagrass(allow("${topic.name}"))]
{
  // diagnostic scope
}
\`\`\`

Workspace suppression in \`Seagrass.toml\`:

\`\`\`toml
[lints]
allow = ["${topic.name}"]
\`\`\`

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
`;
}

function fixedLintPage(topic: TopicEntry, text: string): string {
  if (hasGeneratedBoilerplate(text)) {
    return lintPage(topic);
  }
  if (pageContentFailures(topic, "", text).some(isSuppressionFailure)) {
    return replaceSuppressionSection(text, suppressionSection(topic));
  }
  return text;
}

function suppressionSection(topic: TopicEntry): string {
  return lintPage(topic).split("## Suppression\n")[1];
}

function replaceSuppressionSection(text: string, replacement: string): string {
  const marker = "## Suppression\n";
  const index = text.indexOf(marker);
  if (index === -1) {
    return `${text.trimEnd()}\n\n${marker}${replacement}`;
  }
  return `${text.slice(0, index)}${marker}${replacement}`;
}

function isSuppressionFailure(failure: string): boolean {
  return (
    failure.includes("seagrass-allow") ||
    failure.includes("Item or block suppression") ||
    failure.includes("#[seagrass(allow") ||
    failure.includes("[lints]") ||
    failure.includes("allow = [")
  );
}

function generatedBoilerplateFailures(topic: string, path: string, text: string): string[] {
  return GENERATED_BOILERPLATE.filter((snippet) => text.includes(snippet)).map(
    (snippet) => `${topic}: ${path} contains generic boilerplate: ${snippet}`,
  );
}

function hasGeneratedBoilerplate(text: string): boolean {
  return GENERATED_BOILERPLATE.some((snippet) => text.includes(snippet));
}

function topicInvariant(topic: string): string {
  return topic.replace("seagrass/", "").replaceAll(".", " ").replaceAll("-", " ");
}

function topicTitle(topic: string): string {
  return topic
    .replace("seagrass/", "")
    .split(/[.-]/)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function requiredTextFailures(topic: string, path: string, text: string, needles: string[]): string[] {
  return needles
    .filter((needle) => !text.includes(needle))
    .map((needle) => `${topic}: ${path} missing ${needle}`);
}

function compareStrings(left: string, right: string): number {
  return left.localeCompare(right);
}
