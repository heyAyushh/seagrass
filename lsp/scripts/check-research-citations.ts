#!/usr/bin/env bun

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");
const researchCitationPaths = [resolve(repoRoot, "lsp/docs/lint-patterns-research.md")];
const githubSourceCitationRegex =
  /https:\/\/github\.com\/[^/\s)]+\/[^/\s)]+\/(?:blob|tree|raw)\/([^/\s)]+)(?:\/[^\s)]*)?/g;
const googlesourceCitationRegex =
  /https:\/\/[a-z0-9.-]+\.googlesource\.com\/[^)\s]+\/\+\/([^/\s)]+)(?:\/[^\s)]*)?/g;
const pinnedGithubRefPattern = /^[a-f0-9]{40}$/i;

if (import.meta.main) {
  const failures = checkResearchCitations();
  if (failures.length > 0) {
    console.error(`\nseagrass research citation check failed:\n${failures.join("\n")}`);
    process.exit(1);
  }

  console.log(`seagrass research citation check passed: ${researchCitationPaths.length} documents`);
}

export function checkResearchCitations(): string[] {
  return researchCitationPaths.flatMap((path) =>
    mutableGithubCitationFailures(path, readFileSync(path, "utf8")),
  );
}

export function mutableGithubCitationFailures(path: string, text: string): string[] {
  return [
    ...unpinnedCitationFailures(path, text, githubSourceCitationRegex, "GitHub"),
    ...unpinnedCitationFailures(path, text, googlesourceCitationRegex, "googlesource"),
  ];
}

function unpinnedCitationFailures(
  path: string,
  text: string,
  pattern: RegExp,
  source: string,
): string[] {
  return [...text.matchAll(pattern)].flatMap((match) => {
    const url = match[0];
    const ref = match[1];
    if (pinnedGithubRefPattern.test(ref)) {
      return [];
    }
    return [`${path} uses unpinned ${source} source citation ${url}`];
  });
}
