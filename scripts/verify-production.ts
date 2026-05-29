#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

import { resolveAnchorSourcePath } from "./anchor-source.ts";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const zedDir = resolve(repoRoot, "editors/zed");
const vscodeDir = resolve(repoRoot, "editors/vscode");
const rootVersion = readReleaseVersion();
const anchorSourcePath = resolveAnchorSourcePath(repoRoot);

checkVersionConsistency();

const steps = [
  {
    name: "Rust formatting",
    cwd: repoRoot,
    command: "cargo",
    args: ["fmt", "-p", "seagrass", "--", "--check"],
  },
  {
    name: "Seagrass source size",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/check-source-size.ts"],
  },
  {
    name: "Seagrass rule hygiene",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/check-rule-hygiene.ts"],
  },
  {
    name: "Rule hygiene checker tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/check-rule-hygiene.test.ts"],
  },
  {
    name: "Seagrass diagnostic topics",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/check-diagnostic-topics.ts"],
  },
  {
    name: "Diagnostic topic checker tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/check-diagnostic-topics.test.ts"],
  },
  {
    name: "Seagrass diagnostic audit",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/check-diagnostic-audit.ts"],
  },
  {
    name: "Diagnostic audit checker tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/check-diagnostic-audit.test.ts"],
  },
  {
    name: "Research citations",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/check-research-citations.ts"],
  },
  {
    name: "Research citation checker tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/check-research-citations.test.ts"],
  },
  {
    name: "Seagrass lint catalog",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/check-lint-catalog.ts"],
  },
  {
    name: "Lint catalog checker tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/check-lint-catalog.test.ts"],
  },
  {
    name: "Seagrass release readiness evidence shape",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/check-release-readiness.ts", "--allow-pending", "--version", rootVersion],
  },
  {
    name: "Release readiness checker tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/check-release-readiness.test.ts"],
  },
  {
    name: "Release readiness evidence apply tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/apply-release-readiness.test.ts"],
  },
  {
    name: "Release workflow tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/release-workflow.test.ts"],
  },
  {
    name: "Release package command tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/package-release.test.ts"],
  },
  {
    name: "Generated Anchor support freshness",
    cwd: repoRoot,
    command: "bun",
    args: [
      "scripts/regen-support.ts",
      "--anchor-path",
      anchorSourcePath,
      "--family",
      "v1",
      "--check",
    ],
  },
  {
    name: "Anchor property tests",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/verify-proptest.ts"],
  },
  {
    name: "Fuzz readiness evidence tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/fuzz-readiness.test.ts"],
  },
  {
    name: "Fuzz artifact import tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/import-fuzz-artifacts.test.ts"],
  },
  {
    name: "Review readiness evidence tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/review-readiness.test.ts"],
  },
  {
    name: "Framework crate tests",
    cwd: repoRoot,
    command: "cargo",
    args: [
      "test",
      "-p",
      "seagrass-cli",
      "-p",
      "seagrass-framework",
      "-p",
      "seagrass-anchor-v1",
      "-p",
      "seagrass-anchor-v2-preview",
      "-p",
      "seagrass-pinocchio",
      "-p",
      "seagrass-native",
      "-p",
      "seagrass-sources",
    ],
  },
  {
    name: "Editor UX parity",
    cwd: repoRoot,
    command: "cargo",
    args: ["test", "-p", "seagrass", "editor_ux_parity", "--", "--nocapture"],
  },
  {
    name: "Seagrass tests",
    cwd: repoRoot,
    command: "cargo",
    args: ["test", "-p", "seagrass"],
  },
  {
    name: "Seagrass binary",
    cwd: repoRoot,
    command: "cargo",
    args: ["build", "-p", "seagrass"],
  },
  {
    name: "Protocol smoke",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/protocol-smoke.ts"],
    env: {
      SEAGRASS_SERVER_BINARY: resolve(repoRoot, "target/debug/seagrass"),
    },
  },
  {
    name: "Hotpath perf budget parser",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/perf-replay.test.ts"],
  },
  {
    name: "Perf workflow guardrail tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/perf-workflow.test.ts"],
  },
  {
    name: "Synthetic workspace generator tests",
    cwd: repoRoot,
    command: "bun",
    args: ["test", "scripts/synth-workspace.test.ts"],
  },
  {
    name: "Hotpath perf replay",
    cwd: repoRoot,
    command: "bun",
    args: ["scripts/perf-replay.ts"],
  },
  {
    name: "N=200 scale benchmark",
    cwd: repoRoot,
    command: "bun",
    args: [
      "scripts/scale-benchmark.ts",
      "--programs",
      "200",
      "--samples",
      "3",
      "--report",
      "target/seagrass-scale-200.json",
    ],
  },
  {
    name: "Shared editor UI contract",
    cwd: repoRoot,
    command: "bun",
    args: ["editors/check-ui-contract.ts"],
  },
  {
    name: "VS Code extension",
    cwd: vscodeDir,
    command: "bun",
    args: ["run", "check"],
  },
  {
    name: "Zed extension tests",
    cwd: zedDir,
    command: "cargo",
    args: ["test"],
  },
  {
    name: "Zed release wasm",
    cwd: zedDir,
    command: "cargo",
    args: ["build", "--target", "wasm32-wasip2", "--release"],
  },
];

for (const step of steps) {
  run(step);
}

const checkedInWasm = resolve(zedDir, "extension.wasm");
const builtWasmCandidates = [
  resolve(zedDir, "target/wasm32-wasip2/release/seagrass_zed.wasm"),
  resolve(repoRoot, "target/wasm32-wasip2/release/seagrass_zed.wasm"),
] as const;
const builtWasm = builtWasmCandidates.find(existsSync);
if (!existsSync(checkedInWasm)) {
  fail(`Zed extension artifact is missing: ${checkedInWasm}`);
}
if (!builtWasm) {
  fail(
    [
      "Zed release wasm was not built. Checked:",
      ...builtWasmCandidates.map((path) => `  ${path}`),
    ].join("\n"),
  );
}

const checkedInHash = sha256(checkedInWasm);
const builtHash = sha256(builtWasm);
if (checkedInHash !== builtHash) {
  fail(
    [
      "Zed extension.wasm is stale.",
      `  extension.wasm: ${checkedInHash}`,
      `  built wasm:     ${builtHash}`,
      "Run:",
      `  cp ${relative(repoRoot, builtWasm)} editors/zed/extension.wasm`,
    ].join("\n"),
  );
}

console.log("seagrass production verification passed");

function run(step) {
  console.log(`\n==> ${step.name}`);
  console.log(`$ ${step.command} ${step.args.join(" ")}`);
  const result = spawnSync(step.command, step.args, {
    cwd: step.cwd,
    stdio: "inherit",
    env: { ...process.env, ...(step.env ?? {}) },
  });

  if (result.error) {
    fail(`${step.name} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`${step.name} failed with exit code ${result.status}`);
  }
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function checkVersionConsistency() {
  const versions = [
    ["VERSION", rootVersion],
    ["seagrass Cargo package", cargoPackageVersion(repoRoot, "seagrass")],
    [
      "VS Code extension package",
      JSON.parse(readFileSync(resolve(vscodeDir, "package.json"), "utf8")).version,
    ],
    ["Zed Cargo package", manifestVersion(resolve(zedDir, "Cargo.toml"))],
    ["Zed extension manifest", manifestVersion(resolve(zedDir, "extension.toml"))],
  ];

  const mismatches = versions.filter(([, version]) => version !== rootVersion);
  if (mismatches.length > 0) {
    fail(
      [
        "Seagrass release version drift detected.",
        `Expected every LSP/editor manifest to match VERSION=${rootVersion}.`,
        ...versions.map(([name, version]) => `  ${name}: ${version}`),
      ].join("\n"),
    );
  }
}

function readReleaseVersion() {
  return readFileSync(resolve(repoRoot, "VERSION"), "utf8").trim();
}

function cargoPackageVersion(cwd, packageName) {
  const result = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
    cwd,
    encoding: "utf8",
  });
  if (result.error) {
    fail(`cargo metadata failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`cargo metadata failed with exit code ${result.status}\n${result.stderr}`);
  }

  const metadata = JSON.parse(result.stdout);
  const pkg = metadata.packages.find((candidate) => candidate.name === packageName);
  if (!pkg) {
    fail(`cargo metadata did not include package '${packageName}'`);
  }
  return pkg.version;
}

function manifestVersion(path) {
  const manifest = readFileSync(path, "utf8");
  const match = manifest.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    fail(`Manifest is missing a top-level version field: ${path}`);
  }
  return match[1];
}

function fail(message) {
  console.error(`\nproduction verification failed:\n${message}`);
  process.exit(1);
}
