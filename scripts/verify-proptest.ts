#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");

const propertyTestSteps = [
  {
    name: "IDL discriminator encoding properties",
    args: [
      "test",
      "-p",
      "anchor-lang-idl",
      "--features",
      "convert",
      "discriminator_is_sha256_prefix_for_generated_names",
    ],
  },
  {
    name: "declare_program discriminator token properties",
    args: [
      "test",
      "-p",
      "anchor-attribute-program",
      "generated_discriminator_tokens_preserve_all_input_bytes",
    ],
  },
  {
    name: "declare_program IDL type token properties",
    args: [
      "test",
      "-p",
      "anchor-attribute-program",
      "supported_idl_types_convert_to_parseable_rust_types",
    ],
  },
  {
    name: "constraint parser properties",
    args: ["test", "-p", "anchor-syn", "parser::accounts::constraints::tests::"],
  },
  {
    name: "lamports conservation properties",
    args: ["test", "-p", "anchor-lang", "lamports_tests::paired_lamports_transfer_preserves_total"],
  },
  {
    name: "IDL spec JSON roundtrip properties",
    args: ["test", "-p", "anchor-lang-idl-spec", "current_idl_spec_roundtrips_through_json"],
  },
  {
    name: "Seagrass account semantics properties",
    args: ["test", "-p", "seagrass", "account_semantics"],
  },
  {
    name: "current IDL conversion JSON roundtrip properties",
    args: [
      "test",
      "-p",
      "anchor-lang-idl",
      "--features",
      "convert",
      "current_spec_json_convert_roundtrips",
    ],
  },
  {
    name: "legacy IDL conversion JSON properties",
    args: [
      "test",
      "-p",
      "anchor-lang-idl",
      "--features",
      "convert",
      "legacy_json_conversion_preserves_instruction_discriminator_and_arg_type",
    ],
  },
];

propertyTestSteps.forEach(runCargoTest);
console.log("anchor proptest verification passed");

function runCargoTest(step) {
  console.log(`\n==> ${step.name}`);
  console.log(`$ cargo ${step.args.join(" ")}`);
  const result = spawnSync("cargo", step.args, {
    cwd: repoRoot,
    stdio: "inherit",
    env: {
      ...process.env,
      PROPTEST_CASES: process.env.PROPTEST_CASES ?? "10000",
    },
  });

  if (result.error) {
    fail(`${step.name} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`${step.name} failed with exit code ${result.status}`);
  }
}

function fail(message) {
  console.error(`\nanchor proptest verification failed:\n${message}`);
  process.exit(1);
}
