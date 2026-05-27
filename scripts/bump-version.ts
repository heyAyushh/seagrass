#!/usr/bin/env bun

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const SEMVER_PATTERN = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const WORKSPACE_VERSION_PATTERN = /(^\[workspace\.package\][\s\S]*?^version\s*=\s*)"[^"]+"/m;
const TOML_VERSION_PATTERN = /(^version\s*=\s*)"[^"]+"/m;

type VersionTarget = {
  path: string;
  update: (content: string, version: string) => string;
};

type CliOptions = {
  version: string;
  dryRun: boolean;
};

const versionTargets: VersionTarget[] = [
  {
    path: "VERSION",
    update: (_content, version) => `${version}\n`,
  },
  {
    path: "Cargo.toml",
    update: (content, version) =>
      replaceOrThrow(content, WORKSPACE_VERSION_PATTERN, `$1"${version}"`, "workspace version"),
  },
  {
    path: "editors/vscode/package.json",
    update: updatePackageJson,
  },
  {
    path: "editors/zed/Cargo.toml",
    update: (content, version) =>
      replaceOrThrow(content, TOML_VERSION_PATTERN, `$1"${version}"`, "Zed Cargo version"),
  },
  {
    path: "editors/zed/extension.toml",
    update: (content, version) =>
      replaceOrThrow(content, TOML_VERSION_PATTERN, `$1"${version}"`, "Zed extension version"),
  },
];

if (import.meta.main) {
  const options = parseCliOptions(process.argv.slice(2));
  bumpVersion(options);
}

export function bumpVersion(options: CliOptions): void {
  for (const target of versionTargets) {
    const absolutePath = resolve(repoRoot, target.path);
    const current = readFileSync(absolutePath, "utf8");
    const next = target.update(current, options.version);
    if (next === current) {
      continue;
    }
    if (!options.dryRun) {
      writeFileSync(absolutePath, next);
    }
    console.log(`${options.dryRun ? "would update" : "updated"} ${target.path}`);
  }
}

function parseCliOptions(args: string[]): CliOptions {
  let dryRun = false;
  const positional: string[] = [];

  for (const arg of args) {
    if (arg === "--dry-run") {
      dryRun = true;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      printUsageAndExit(0);
    }
    if (arg.startsWith("--")) {
      throw new Error(`Unknown argument: ${arg}`);
    }
    positional.push(arg);
  }

  if (positional.length !== 1) {
    printUsageAndExit(1);
  }

  const version = positional[0];
  if (!SEMVER_PATTERN.test(version)) {
    throw new Error(`Version must be SemVer, got ${version}`);
  }

  return { version, dryRun };
}

function updatePackageJson(content: string, version: string): string {
  const parsed = JSON.parse(content) as unknown;
  if (!isRecord(parsed)) {
    throw new Error("VS Code package manifest must be a JSON object");
  }
  parsed.version = version;
  return `${JSON.stringify(parsed, null, 2)}\n`;
}

function replaceOrThrow(
  content: string,
  pattern: RegExp,
  replacement: string,
  label: string,
): string {
  const next = content.replace(pattern, replacement);
  if (next === content) {
    throw new Error(`Could not find ${label}`);
  }
  return next;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function printUsageAndExit(code: number): never {
  const stream = code === 0 ? console.log : console.error;
  stream("Usage: bun scripts/bump-version.ts [--dry-run] <version>");
  process.exit(code);
}
