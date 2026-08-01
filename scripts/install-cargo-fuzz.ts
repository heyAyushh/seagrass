#!/usr/bin/env bun

/**
 * Shared constants and pure helpers for the deterministic cargo-fuzz installer.
 * CI and local operators should run `scripts/install-cargo-fuzz.sh`.
 */

export const PINNED_CARGO_FUZZ_VERSION = "0.13.2";
export const INSTALL_TOOLCHAIN = "nightly";
export const MIN_INSTALL_RUSTC_MINOR = 91;

export function installCommand(options: { force?: boolean } = {}): string[] {
  const args = [
    "cargo",
    `+${INSTALL_TOOLCHAIN}`,
    "install",
    "cargo-fuzz",
    "--locked",
    "--version",
    PINNED_CARGO_FUZZ_VERSION,
  ];
  if (options.force) {
    args.push("--force");
  }
  return args;
}

export function parseRustcMinor(versionOutput: string): number {
  const match = /rustc\s+(\d+)\.(\d+)\./.exec(versionOutput);
  if (!match) {
    throw new Error(`could not parse rustc version from: ${versionOutput}`);
  }
  const major = Number(match[1]);
  const minor = Number(match[2]);
  if (major !== 1) {
    throw new Error(`expected rustc 1.x, got ${versionOutput.trim()}`);
  }
  return minor;
}

export function assertInstallToolchainSupportsCargoFuzz(versionOutput: string): number {
  const minor = parseRustcMinor(versionOutput);
  if (minor < MIN_INSTALL_RUSTC_MINOR) {
    throw new Error(
      `${INSTALL_TOOLCHAIN} rustc is 1.${minor}, but cargo-fuzz ${PINNED_CARGO_FUZZ_VERSION} requires rustc 1.${MIN_INSTALL_RUSTC_MINOR}+ (cargo-platform MSRV)`,
    );
  }
  return minor;
}
