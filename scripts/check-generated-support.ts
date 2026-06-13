#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const GENERATED_LOCK_VERSION = 1;
const SHA256_HEX_LENGTH = 64;
const GENERATED_RUST_EXTENSION = ".rs";
const V2_PREVIEW_SOURCE_ENV = "SEAGRASS_ANCHOR_V2_PREVIEW_PATH";
const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const lockPath = resolve(repoRoot, "scripts/generated-support.lock.json");

const FULL_CATALOG_FILES = [
  "anchor_error_catalog_generated.rs",
  "anchor_field_completions_generated.rs",
  "anchor_support_generated.rs",
  "constraint_catalog_generated.rs",
  "constraint_keys_generated.rs",
] as const;

const MANIFEST_ONLY_FILES = ["anchor_support_generated.rs"] as const;

export type GeneratedSupportRoot = {
  id: string;
  path: string;
  source: string;
  expectedFiles: readonly string[];
};

export type GeneratedSupportLock = {
  schemaVersion: number;
  generatedRoots: GeneratedSupportLockRoot[];
};

export type GeneratedSupportLockRoot = {
  id: string;
  path: string;
  source: string;
  files: Record<string, string>;
};

export const GENERATED_SUPPORT_ROOTS: readonly GeneratedSupportRoot[] = [
  {
    id: "anchor-v1-main",
    path: "src/anchor/generated",
    source: "Anchor v1 generated catalog from the pinned anchor-syn checkout.",
    expectedFiles: FULL_CATALOG_FILES,
  },
  {
    id: "anchor-v1-manifest",
    path: "crates/seagrass-anchor-v1/src/generated",
    source: "Anchor v1 framework package manifest only; dead duplicate catalogs were intentionally removed.",
    expectedFiles: MANIFEST_ONLY_FILES,
  },
  {
    id: "anchor-v2-preview",
    path: "crates/seagrass-anchor-v2-preview/src/generated",
    source: "Anchor v2 preview generated catalog from an anchor-next checkout.",
    expectedFiles: FULL_CATALOG_FILES,
  },
] as const;

if (import.meta.main) {
  const options = parseOptions(process.argv.slice(2));
  const actual = currentLockOrExit();

  if (options.update) {
    writeGeneratedSupportLock(lockPath, actual);
    console.log(`updated ${relative(repoRoot, lockPath)}`);
    process.exit(0);
  }

  const expected = readGeneratedSupportLock(lockPath);
  const failures = [
    ...generatedSupportLockFailures(expected, actual),
    ...optionalV2PreviewFreshnessFailures(repoRoot),
  ];

  if (failures.length > 0) {
    console.error(`\nseagrass generated support check failed:\n${failures.join("\n")}`);
    process.exit(1);
  }

  console.log(`seagrass generated support check passed: ${actual.generatedRoots.length} roots`);
}

function currentLockOrExit(): GeneratedSupportLock {
  try {
    return currentGeneratedSupportLock({ repoRoot, roots: GENERATED_SUPPORT_ROOTS });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`\nseagrass generated support check failed:\n${message}`);
    process.exit(1);
  }
}

export function currentGeneratedSupportLock(input: {
  repoRoot: string;
  roots: readonly GeneratedSupportRoot[];
}): GeneratedSupportLock {
  return {
    schemaVersion: GENERATED_LOCK_VERSION,
    generatedRoots: input.roots.map((root) => currentGeneratedRoot(input.repoRoot, root)),
  };
}

export function generatedSupportLockFailures(
  expected: GeneratedSupportLock,
  actual: GeneratedSupportLock,
): string[] {
  const failures: string[] = [];
  if (expected.schemaVersion !== GENERATED_LOCK_VERSION) {
    failures.push(
      `schemaVersion must be ${GENERATED_LOCK_VERSION}, got ${expected.schemaVersion}`,
    );
  }
  if (actual.schemaVersion !== GENERATED_LOCK_VERSION) {
    failures.push(
      `actual schemaVersion must be ${GENERATED_LOCK_VERSION}, got ${actual.schemaVersion}`,
    );
  }

  const expectedRoots = rootsById(expected.generatedRoots);
  const actualRoots = rootsById(actual.generatedRoots);
  for (const id of sortedUnion(expectedRoots.keys(), actualRoots.keys())) {
    const expectedRoot = expectedRoots.get(id);
    const actualRoot = actualRoots.get(id);
    if (!expectedRoot) {
      failures.push(`${id}: unexpected generated support root`);
      continue;
    }
    if (!actualRoot) {
      failures.push(`${id}: missing generated support root`);
      continue;
    }
    failures.push(...generatedRootFailures(expectedRoot, actualRoot));
  }

  return failures;
}

function currentGeneratedRoot(repoRoot: string, root: GeneratedSupportRoot): GeneratedSupportLockRoot {
  const absoluteRoot = resolve(repoRoot, root.path);
  if (!existsSync(absoluteRoot)) {
    throw new Error(`${root.id}: generated root does not exist: ${root.path}`);
  }

  const actualFiles = rustFiles(absoluteRoot);
  const expectedFiles = [...root.expectedFiles].sort(compareStrings);
  const fileSetFailures = fileSetFailuresForRoot(root.id, expectedFiles, actualFiles);
  if (fileSetFailures.length > 0) {
    throw new Error(fileSetFailures.join("\n"));
  }

  return {
    id: root.id,
    path: root.path,
    source: root.source,
    files: Object.fromEntries(
      expectedFiles.map((file) => [file, sha256Hex(readFileSync(resolve(absoluteRoot, file)))]),
    ),
  };
}

function fileSetFailuresForRoot(
  id: string,
  expectedFiles: readonly string[],
  actualFiles: readonly string[],
): string[] {
  const expected = new Set(expectedFiles);
  const actual = new Set(actualFiles);
  return [
    ...expectedFiles.filter((file) => !actual.has(file)).map((file) => `${id}: missing ${file}`),
    ...actualFiles.filter((file) => !expected.has(file)).map((file) => `${id}: unexpected ${file}`),
  ];
}

function generatedRootFailures(
  expectedRoot: GeneratedSupportLockRoot,
  actualRoot: GeneratedSupportLockRoot,
): string[] {
  const failures: string[] = [];
  if (expectedRoot.path !== actualRoot.path) {
    failures.push(`${expectedRoot.id}: path drifted from ${expectedRoot.path} to ${actualRoot.path}`);
  }
  if (expectedRoot.source !== actualRoot.source) {
    failures.push(`${expectedRoot.id}: source note drifted`);
  }

  const expectedFiles = new Set(Object.keys(expectedRoot.files));
  const actualFiles = new Set(Object.keys(actualRoot.files));
  for (const file of sortedUnion(expectedFiles.keys(), actualFiles.keys())) {
    const expectedHash = expectedRoot.files[file];
    const actualHash = actualRoot.files[file];
    if (!expectedHash) {
      failures.push(`${expectedRoot.id}/${file}: unexpected generated file`);
      continue;
    }
    if (!isSha256Hex(expectedHash)) {
      failures.push(`${expectedRoot.id}/${file}: lock sha256 is malformed`);
      continue;
    }
    if (!actualHash) {
      failures.push(`${expectedRoot.id}/${file}: missing generated file`);
      continue;
    }
    if (expectedHash !== actualHash) {
      failures.push(
        `${expectedRoot.id}/${file}: sha256 mismatch, expected ${expectedHash}, got ${actualHash}`,
      );
    }
  }

  return failures;
}

function optionalV2PreviewFreshnessFailures(repoRoot: string): string[] {
  const anchorPath = process.env[V2_PREVIEW_SOURCE_ENV];
  if (!anchorPath) {
    return [];
  }

  const result = spawnSync(
    process.execPath,
    [
      "scripts/regen-support.ts",
      "--anchor-path",
      anchorPath,
      "--family",
      "v2-preview",
      "--out-dir",
      "crates/seagrass-anchor-v2-preview/src/generated",
      "--check",
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  );

  if (result.status === 0) {
    return [];
  }

  const output = [result.stdout, result.stderr].filter(Boolean).join("\n").trim();
  return [
    `${V2_PREVIEW_SOURCE_ENV} regeneration check failed${output ? `:\n${output}` : ""}`,
  ];
}

function parseOptions(args: string[]): { update: boolean } {
  const options = { update: false };
  for (const arg of args) {
    if (arg === "--update") {
      options.update = true;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    throw new Error(`Unknown argument: ${arg}`);
  }
  return options;
}

function readGeneratedSupportLock(path: string): GeneratedSupportLock {
  return JSON.parse(readFileSync(path, "utf8")) as GeneratedSupportLock;
}

function writeGeneratedSupportLock(path: string, lock: GeneratedSupportLock): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(lock, null, 2)}\n`);
}

function rustFiles(root: string): string[] {
  return readdirSync(root)
    .filter((file) => file.endsWith(GENERATED_RUST_EXTENSION))
    .filter((file) => statSync(resolve(root, file)).isFile())
    .sort(compareStrings);
}

function rootsById(roots: GeneratedSupportLockRoot[]): Map<string, GeneratedSupportLockRoot> {
  return roots.reduce((byId, root) => {
    byId.set(root.id, root);
    return byId;
  }, new Map<string, GeneratedSupportLockRoot>());
}

function sortedUnion(left: Iterable<string>, right: Iterable<string>): string[] {
  return [...new Set([...left, ...right])].sort(compareStrings);
}

function sha256Hex(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function isSha256Hex(value: string): boolean {
  return value.length === SHA256_HEX_LENGTH && /^[a-f0-9]+$/.test(value);
}

function compareStrings(left: string, right: string): number {
  return left.localeCompare(right);
}

function printHelp(): void {
  console.log(`Check checked-in generated Anchor support catalogs against their lock.

Usage:
  bun scripts/check-generated-support.ts [--update]

Options:
  --update  Rewrite scripts/generated-support.lock.json from the checked-in files.

Set ${V2_PREVIEW_SOURCE_ENV} to an anchor-next checkout to also run a source-backed
v2-preview regeneration check.`);
}
