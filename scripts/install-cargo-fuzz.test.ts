import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { repoRoot } from "./release-evidence.ts";
import {
  INSTALL_TOOLCHAIN,
  MIN_INSTALL_RUSTC_MINOR,
  PINNED_CARGO_FUZZ_VERSION,
  assertInstallToolchainSupportsCargoFuzz,
  installCommand,
  parseRustcMinor,
} from "./install-cargo-fuzz.ts";

const fuzzWorkflowPath = resolve(repoRoot, ".github/workflows/fuzz.yaml");
const releaseWorkflowPath = resolve(repoRoot, ".github/workflows/release.yaml");
const qualityKitPath = resolve(repoRoot, "docs/quality-kit.md");
const installScriptPath = resolve(repoRoot, "scripts/install-cargo-fuzz.sh");
const packageScriptPath = resolve(repoRoot, "scripts/package-fuzz-artifacts.sh");

describe("install-cargo-fuzz", () => {
  test("pins cargo-fuzz and installs under nightly for locked MSRV deps", () => {
    const installScript = readFileSync(installScriptPath, "utf8");

    expect(PINNED_CARGO_FUZZ_VERSION).toBe("0.13.2");
    expect(INSTALL_TOOLCHAIN).toBe("nightly");
    expect(MIN_INSTALL_RUSTC_MINOR).toBe(91);
    expect(installCommand()).toEqual([
      "cargo",
      "+nightly",
      "install",
      "cargo-fuzz",
      "--locked",
      "--version",
      "0.13.2",
    ]);
    expect(installCommand({ force: true })).toContain("--force");
    expect(installScript).toContain(`PINNED_CARGO_FUZZ_VERSION="${PINNED_CARGO_FUZZ_VERSION}"`);
    expect(installScript).toContain(`INSTALL_TOOLCHAIN="${INSTALL_TOOLCHAIN}"`);
    expect(installScript).toContain(`MIN_INSTALL_RUSTC_MINOR=${MIN_INSTALL_RUSTC_MINOR}`);
    expect(installScript).toContain("--locked");
    expect(installScript).toContain("--version");
  });

  test("rejects install toolchains below the cargo-platform MSRV floor", () => {
    expect(parseRustcMinor("rustc 1.99.0-nightly (ad3d0bc14 2026-07-31)")).toBe(99);
    expect(assertInstallToolchainSupportsCargoFuzz("rustc 1.91.0 (abc 2026-01-01)")).toBe(91);
    expect(() =>
      assertInstallToolchainSupportsCargoFuzz("rustc 1.89.0 (29483883e 2025-08-04)"),
    ).toThrow(/requires rustc 1\.91/);
  });

  test("wires the deterministic installer into fuzz and release workflows", () => {
    const fuzzWorkflow = readFileSync(fuzzWorkflowPath, "utf8");
    const releaseWorkflow = readFileSync(releaseWorkflowPath, "utf8");
    const runScript = readFileSync(resolve(repoRoot, "scripts/run-fuzz.sh"), "utf8");

    expect(fuzzWorkflow).toContain("bash scripts/install-cargo-fuzz.sh");
    expect(releaseWorkflow).toContain("bash scripts/install-cargo-fuzz.sh");
    expect(fuzzWorkflow).toContain('bash scripts/run-fuzz.sh build "${{ matrix.target }}"');
    expect(fuzzWorkflow).toContain("bash scripts/run-fuzz.sh run");
    expect(releaseWorkflow).toContain('bash scripts/run-fuzz.sh build "${{ matrix.target }}"');
    expect(releaseWorkflow).toContain("bash scripts/run-fuzz.sh run");
    expect(runScript).toContain('cargo +nightly fuzz build -s "${FUZZ_SANITIZER}"');
    expect(runScript).toContain('cargo +nightly fuzz run -s "${FUZZ_SANITIZER}"');
    expect(runScript).toContain('FUZZ_SANITIZER="${FUZZ_SANITIZER:-none}"');
    expect(fuzzWorkflow).not.toContain("cargo +1.89.0 install cargo-fuzz");
    expect(releaseWorkflow).not.toContain("cargo +1.89.0 install cargo-fuzz");
    expect(fuzzWorkflow).not.toContain("run: cargo install cargo-fuzz --locked");
    expect(releaseWorkflow).not.toContain("run: cargo install cargo-fuzz --locked");
    expect(fuzzWorkflow).not.toContain("bun scripts/install-cargo-fuzz.ts");
    expect(releaseWorkflow).not.toContain("bun scripts/install-cargo-fuzz.ts");
  });

  test("defaults fuzz builds to -s none to avoid ASAN __sancov_gen_ link failures", () => {
    const runScript = readFileSync(resolve(repoRoot, "scripts/run-fuzz.sh"), "utf8");
    const docs = readFileSync(qualityKitPath, "utf8");
    const prWorkflow = readFileSync(resolve(repoRoot, ".github/workflows/pr.yaml"), "utf8");
    const fuzzWorkflow = readFileSync(fuzzWorkflowPath, "utf8");

    expect(runScript).toContain('FUZZ_SANITIZER="${FUZZ_SANITIZER:-none}"');
    expect(runScript).toContain("rust-fuzz/cargo-fuzz#404");
    expect(runScript).toContain('FUZZ_RSS_LIMIT_MB="${FUZZ_RSS_LIMIT_MB:-4096}"');
    expect(runScript).toContain('FUZZ_INPUT_TIMEOUT_SEC="${FUZZ_INPUT_TIMEOUT_SEC:-60}"');
    expect(runScript).toContain('FUZZ_IGNORE_OOMS="${FUZZ_IGNORE_OOMS:-1}"');
    expect(runScript).toContain('FUZZ_IGNORE_TIMEOUTS="${FUZZ_IGNORE_TIMEOUTS:-1}"');
    expect(runScript).toContain("-rss_limit_mb=");
    expect(runScript).toContain("-ignore_ooms=");
    expect(fuzzWorkflow).toContain('FUZZ_RSS_LIMIT_MB: "4096"');
    expect(docs).toContain("-s none");
    expect(docs).toContain("__sancov_gen_");
    expect(docs).toContain("Fuzz build smoke");
    expect(docs).toContain("4096 MiB");
    expect(docs).toContain("ignore_ooms");
    expect(prWorkflow).toContain("fuzz-build-smoke:");
    expect(prWorkflow).toContain("bash scripts/run-fuzz.sh build fuzz_document_parse");
  });

  test("packages fuzz artifacts through the resilient shell helper", () => {
    const fuzzWorkflow = readFileSync(fuzzWorkflowPath, "utf8");
    const packageScript = readFileSync(packageScriptPath, "utf8");

    expect(fuzzWorkflow).toContain("bash scripts/package-fuzz-artifacts.sh");
    expect(packageScript).toContain("mkdir -p fuzz/corpus fuzz/artifacts");
    expect(packageScript).toContain("fuzz/corpus");
    expect(packageScript).toContain("fuzz/artifacts");
  });

  test("documents the local installer entrypoint", () => {
    const docs = readFileSync(qualityKitPath, "utf8");
    expect(docs).toContain("bash scripts/install-cargo-fuzz.sh");
    expect(docs).toContain("bash scripts/run-fuzz.sh run");
    expect(docs).not.toContain("cargo +1.89.0 install cargo-fuzz --locked");
  });
});
