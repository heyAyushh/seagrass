#!/usr/bin/env bun

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_PROGRAM_COUNT = 200;
const MIN_PROGRAM_COUNT = 1;
const MAX_PROGRAM_COUNT = 2_000;
const ANCHOR_LANG_RELATIVE_PATH = "../../../../lang";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");

const args = parseArgs(process.argv.slice(2));
const programCount = boundedProgramCount(args.programs ?? DEFAULT_PROGRAM_COUNT);
const outputRoot = resolve(args.output ?? resolve(repoRoot, "target/seagrass-synth-workspace"));

mkdirSync(outputRoot, { recursive: true });
writeWorkspaceManifest(outputRoot, programCount);
Array.from({ length: programCount }, (_, programIndex) => writeProgram(outputRoot, programIndex));

console.log(`synthetic Anchor workspace written: ${outputRoot}`);
console.log(`programs: ${programCount}`);

type Args = {
  programs?: number;
  output?: string;
};

function parseArgs(rawArgs: string[]): Args {
  return parseArgList(rawArgs, {});
}

function parseArgList(rawArgs: string[], parsed: Args): Args {
  const [arg, value, ...remainingArgs] = rawArgs;
  if (!arg) {
    return parsed;
  }
  if (arg === "--programs") {
    return parseArgList(remainingArgs, { ...parsed, programs: Number(requiredArg(value, arg)) });
  }
  if (arg === "--output") {
    return parseArgList(remainingArgs, { ...parsed, output: requiredArg(value, arg) });
  }
  throw new Error(`unknown argument: ${arg}`);
}

function requiredArg(value: string | undefined, name: string): string {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function boundedProgramCount(value: number): number {
  if (!Number.isInteger(value) || value < MIN_PROGRAM_COUNT || value > MAX_PROGRAM_COUNT) {
    throw new Error(
      `--programs must be an integer from ${MIN_PROGRAM_COUNT} to ${MAX_PROGRAM_COUNT}`,
    );
  }
  return value;
}

function writeWorkspaceManifest(root: string, programCount: number): void {
  const members = Array.from(
    { length: programCount },
    (_, programIndex) => `"programs/program_${programIndex}"`,
  ).join(",\n  ");
  writeFileSync(
    resolve(root, "Cargo.toml"),
    `[workspace]\nmembers = [\n  ${members}\n]\nresolver = "2"\n`,
  );
}

function writeProgram(root: string, programIndex: number): void {
  const crateName = `program_${programIndex}`;
  const programRoot = resolve(root, "programs", crateName);
  const sourceRoot = resolve(programRoot, "src");
  mkdirSync(sourceRoot, { recursive: true });
  writeFileSync(
    resolve(programRoot, "Cargo.toml"),
    `[package]\nname = "${crateName}"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\nanchor-lang = { path = "${ANCHOR_LANG_RELATIVE_PATH}" }\n`,
  );
  writeFileSync(resolve(sourceRoot, "lib.rs"), programSource(programIndex));
}

function programSource(programIndex: number): string {
  return `use anchor_lang::prelude::*;\n\ndeclare_id!("11111111111111111111111111111111");\n\n#[program]\npub mod program_${programIndex} {\n    use super::*;\n\n    pub fn initialize(ctx: Context<Initialize${programIndex}>, amount: u64) -> Result<()> {\n        ctx.accounts.state.amount = amount;\n        Ok(())\n    }\n}\n\n#[derive(Accounts)]\npub struct Initialize${programIndex}<'info> {\n    #[account(init, payer = payer, space = 8 + State${programIndex}::INIT_SPACE)]\n    pub state: Account<'info, State${programIndex}>,\n    #[account(mut)]\n    pub payer: Signer<'info>,\n    pub system_program: Program<'info, System>,\n}\n\n#[account]\n#[derive(InitSpace)]\npub struct State${programIndex} {\n    pub amount: u64,\n}\n`;
}
