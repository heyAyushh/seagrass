#!/usr/bin/env bun

import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const GENERATED_FILES = [
  "anchor_error_catalog_generated.rs",
  "anchor_field_completions_generated.rs",
  "anchor_support_generated.rs",
  "constraint_catalog_generated.rs",
  "constraint_keys_generated.rs",
] as const;

type Family = "v1" | "v2-preview";

type Options = {
  anchorPath?: string;
  corpusPath?: string;
  family?: Family;
  outDir: string;
  check: boolean;
  dryRun: boolean;
};

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");
const defaultOutDir = resolve(repoRoot, "lsp/src/generated");
const generatorSource = resolve(repoRoot, "lsp/tools/regen-support/legacy_generator.rs");
const enginePath = resolve(repoRoot, "target/regen-support/regen-support");

const options = parseOptions(process.argv.slice(2));

if (!options.anchorPath) {
  fail(
    "Missing --anchor-path.\nExample: bun lsp/scripts/regen-support.ts --anchor-path . --family v1",
  );
}
if (!options.family) {
  fail(
    "Missing --family.\nExample: bun lsp/scripts/regen-support.ts --anchor-path . --family v1",
  );
}
if (options.check && options.dryRun) {
  fail("--check and --dry-run cannot be combined");
}

const anchorPath = resolve(options.anchorPath);
const corpusPath = options.corpusPath ? resolve(options.corpusPath) : undefined;
const outDir = resolve(options.outDir);
const generationDir =
  options.check || options.dryRun ? mkdtempSync(resolve(tmpdir(), "seagrass-regen-")) : outDir;

validateAnchorPath(anchorPath);
if (corpusPath) {
  assertExists(corpusPath, "--corpus-path");
}
compileEngine();
mkdirSync(generationDir, { recursive: true });
runEngine({ anchorPath, corpusPath, family: options.family, outDir: generationDir });
assertGeneratedFiles(generationDir);

if (options.check) {
  const mismatched = GENERATED_FILES.filter((file) => !sameFile(generationDir, outDir, file));
  rmSync(generationDir, { recursive: true, force: true });
  if (mismatched.length > 0) {
    fail(`Generated support is stale:\n${mismatched.map((file) => `- ${file}`).join("\n")}`);
  }
  console.log("seagrass support generation check passed");
} else if (options.dryRun) {
  reportGeneratedFiles(generationDir, outDir);
  rmSync(generationDir, { recursive: true, force: true });
} else {
  console.log(`seagrass support generated in ${relative(repoRoot, outDir)}`);
}

function parseOptions(args: string[]): Options {
  const options: Options = {
    outDir: defaultOutDir,
    check: false,
    dryRun: false,
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    if (arg === "--anchor-path") {
      options.anchorPath = requiredValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--corpus-path") {
      options.corpusPath = requiredValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--family") {
      options.family = parseFamily(requiredValue(args, index, arg));
      index += 1;
      continue;
    }
    if (arg === "--out-dir") {
      options.outDir = requiredValue(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--check") {
      options.check = true;
      continue;
    }
    if (arg === "--dry-run") {
      options.dryRun = true;
      continue;
    }
    fail(`Unknown argument: ${arg}`);
  }

  return options;
}

function requiredValue(args: string[], index: number, flag: string): string {
  const value = args[index + 1];
  if (!value || value.startsWith("--")) {
    fail(`${flag} requires a value`);
  }
  return value;
}

function parseFamily(value: string): Family {
  if (value === "v1" || value === "v2-preview") {
    return value;
  }
  fail(`--family must be v1 or v2-preview, got ${value}`);
}

function validateAnchorPath(anchorPath: string): void {
  const requiredInputs = [
    ["Cargo.toml"],
    ["lang/syn/src/parser/accounts/constraints.rs"],
    ["lang/syn/src/parser/accounts/mod.rs"],
    ["lang/error/src/lib.rs", "lang/src/error.rs"],
    ["lang/src/lib.rs"],
    ["spl/src/associated_token.rs"],
    ["spl/src/token.rs"],
    ["spl/src/token_2022.rs"],
    ["spl/src/token_interface.rs"],
  ];
  for (const alternatives of requiredInputs) {
    if (alternatives.some((input) => existsSync(resolve(anchorPath, input)))) {
      continue;
    }
    fail(
      `--anchor-path is missing required input; tried:\n${alternatives
        .map((input) => `- ${resolve(anchorPath, input)}`)
        .join("\n")}`,
    );
  }
}

function compileEngine(): void {
  mkdirSync(dirname(enginePath), { recursive: true });
  execFileSync("rustc", ["--edition=2021", generatorSource, "-o", enginePath], {
    cwd: repoRoot,
    stdio: "inherit",
  });
}

function runEngine(input: {
  anchorPath: string;
  corpusPath: string | undefined;
  family: Family;
  outDir: string;
}): void {
  execFileSync(enginePath, {
    cwd: repoRoot,
    env: {
      ...process.env,
      SEAGRASS_REGEN_ANCHOR_PATH: input.anchorPath,
      SEAGRASS_REGEN_FAMILY: input.family,
      SEAGRASS_REGEN_OUT_DIR: input.outDir,
      ...(input.corpusPath ? { SEAGRASS_REGEN_CORPUS_PATH: input.corpusPath } : {}),
    },
    stdio: "inherit",
  });
}

function assertGeneratedFiles(root: string): void {
  const missing = GENERATED_FILES.filter((file) => !existsSync(resolve(root, file)));
  if (missing.length > 0) {
    fail(`Generator did not write expected files:\n${missing.map((file) => `- ${file}`).join("\n")}`);
  }
}

function sameFile(leftRoot: string, rightRoot: string, file: string): boolean {
  const left = resolve(leftRoot, file);
  const right = resolve(rightRoot, file);
  return existsSync(right) && readFileSync(left, "utf8") === readFileSync(right, "utf8");
}

function reportGeneratedFiles(generatedRoot: string, outDir: string): void {
  console.log(`generated: ${generatedFiles(generatedRoot).join(", ")}`);
  const changed = GENERATED_FILES.filter((file) => !sameFile(generatedRoot, outDir, file));
  console.log(
    changed.length === 0
      ? "dry run: checked-in generated support is current"
      : `dry run: would update ${changed.join(", ")}`,
  );
}

function generatedFiles(root: string): string[] {
  return readdirSync(root)
    .filter((name) => statSync(resolve(root, name)).isFile())
    .sort();
}

function assertExists(path: string, label: string): void {
  if (!existsSync(path)) {
    fail(`${label} does not exist: ${path}`);
  }
}

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

function printHelp(): void {
  console.log(`Regenerate checked-in Seagrass Anchor support catalogs.

Usage:
  bun lsp/scripts/regen-support.ts --anchor-path <path> --family <v1|v2-preview> [options]

Options:
  --anchor-path <path>   Anchor checkout to scrape. Required.
  --family <family>      Support family: v1 or v2-preview. Required.
  --out-dir <path>       Output directory. Default: lsp/src/generated.
  --corpus-path <path>   Optional program corpus root. Defaults to Anchor examples and tests.
  --check                Generate to a temp directory and fail if checked-in files differ.
  --dry-run              Generate to a temp directory and report which files would change.

Examples:
  bun lsp/scripts/regen-support.ts --anchor-path . --family v1
  bun lsp/scripts/regen-support.ts --anchor-path ../anchor-next --family v2-preview --dry-run
  bun lsp/scripts/regen-support.ts --anchor-path . --family v1 --check`);
}
