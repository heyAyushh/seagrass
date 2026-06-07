#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";

const DEFAULT_EXAMPLES_DIRECTORY = "examples";
const PROGRAMS_DIRECTORY = "programs";
const SOURCE_DIRECTORY = "src";
const RUST_EXTENSION = ".rs";
const EXPECTED_CLEAN_EXIT_CODE = 0;

const anchorPath = requiredOption("--anchor-path");
const resolvedAnchorPath = resolve(anchorPath);
const examplesPath = resolve(resolvedAnchorPath, DEFAULT_EXAMPLES_DIRECTORY);
const outputPath = resolve(repoRoot, "target/anchor-v2-preview-examples-diagnostics.json");

assertAnchorNextCheckout(resolvedAnchorPath);
const programFileCount = countProgramRustFiles(examplesPath);
if (programFileCount === 0) {
  throw new Error(`Anchor v2 preview examples corpus has no program Rust files: ${examplesPath}`);
}

run("bun", [
  "scripts/regen-support.ts",
  "--anchor-path",
  resolvedAnchorPath,
  "--family",
  "v2-preview",
  "--out-dir",
  "crates/seagrass-anchor-v2-preview/src/generated",
  "--check",
]);

const diagnosticsResult = spawnSync(
  "cargo",
  [
    "run",
    "-p",
    "seagrass-cli",
    "--quiet",
    "--",
    "diagnostics",
    examplesPath,
    "--json",
  ],
  {
    cwd: repoRoot,
    encoding: "utf8",
  },
);

if (diagnosticsResult.error) {
  throw new Error(`Anchor v2 preview diagnostics failed to start: ${diagnosticsResult.error.message}`);
}
if (diagnosticsResult.status !== EXPECTED_CLEAN_EXIT_CODE) {
  throw new Error(
    `Anchor v2 preview examples should not produce ERROR diagnostics; exit ${diagnosticsResult.status}\n${diagnosticsResult.stderr}`,
  );
}
await Bun.write(outputPath, diagnosticsResult.stdout);

const diagnostics = JSON.parse(diagnosticsResult.stdout) as Diagnostic[];
const errorCount = diagnostics.filter((diagnostic) => diagnostic.severity === "ERROR").length;
if (errorCount > 0) {
  throw new Error(`Anchor v2 preview examples produced ${errorCount} ERROR diagnostics`);
}

console.log(
  `ok: Anchor v2 preview examples corpus checked ${programFileCount} program files with ${diagnostics.length} non-error diagnostics`,
);

type Diagnostic = {
  severity?: string;
};

function requiredOption(name: string): string {
  const index = process.argv.indexOf(name);
  const value = process.argv[index + 1];
  if (index === -1 || !value || value.startsWith("--")) {
    throw new Error(`Missing ${name}. Example: bun scripts/check-anchor-v2-preview-corpus.ts ${name} target/anchor-next-smoke`);
  }
  return value;
}

function assertAnchorNextCheckout(path: string): void {
  const requiredPaths = [
    "Cargo.toml",
    "lang/syn/src/parser/accounts/constraints.rs",
    "lang/src/lib.rs",
    DEFAULT_EXAMPLES_DIRECTORY,
  ];
  const missing = requiredPaths.filter((requiredPath) => !existsSync(resolve(path, requiredPath)));
  if (missing.length > 0) {
    throw new Error(
      `Anchor v2 preview checkout is missing required paths:\n${missing
        .map((missingPath) => `- ${resolve(path, missingPath)}`)
        .join("\n")}`,
    );
  }
  const manifest = readFileSync(resolve(path, "Cargo.toml"), "utf8");
  if (!manifest.includes('version = "2.0.0"')) {
    throw new Error("Anchor v2 preview checkout must report version 2.0.0 in Cargo.toml");
  }
}

function run(command: string, args: string[]): void {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "inherit",
  });
  if (result.error) {
    throw new Error(`${command} failed to start: ${result.error.message}`);
  }
  if (result.status !== EXPECTED_CLEAN_EXIT_CODE) {
    throw new Error(`${command} ${args.join(" ")} failed with exit ${result.status}`);
  }
}

function countProgramRustFiles(root: string): number {
  return walk(root).filter(isProgramRustFile).length;
}

function walk(path: string): string[] {
  const entries = readdirSync(path, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const childPath = join(path, entry.name);
    if (entry.isDirectory()) {
      return walk(childPath);
    }
    return entry.isFile() ? [childPath] : [];
  });
}

function isProgramRustFile(path: string): boolean {
  const parts = path.split(/[\\/]/);
  return (
    path.endsWith(RUST_EXTENSION) &&
    parts.includes(PROGRAMS_DIRECTORY) &&
    parts.includes(SOURCE_DIRECTORY)
  );
}
