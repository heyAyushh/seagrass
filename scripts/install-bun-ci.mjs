#!/usr/bin/env node

import { execFileSync } from "node:child_process";

const PINNED_BUN_VERSION = "1.3.8";
const VERIFY_METADATA_ONLY_FLAG = "--verify-metadata-only";
const USE_WINDOWS_COMMAND_SHELL = process.platform === "win32";
const PACKAGE_INTEGRITIES = new Map([
  [
    `bun@${PINNED_BUN_VERSION}`,
    "sha512-TmmtMGQSXY3o0enSMMkvR5g7Dw/2n2oylAUprLVdqA8pDPBoKbzgOmmgHOtvnfRznFppl+Gu2cid3vgAjcNoSg==",
  ],
  [
    `@oven/bun-linux-x64@${PINNED_BUN_VERSION}`,
    "sha512-YDgqVx1MI8E0oDbCEUSkAMBKKGnUKfaRtMdLh9Bjhu7JQacQ/ZCpxwi4HPf5Q0O1TbWRrdxGw2tA2Ytxkn7s1Q==",
  ],
  [
    `@oven/bun-darwin-x64@${PINNED_BUN_VERSION}`,
    "sha512-SaWIxsRQYiT/eA60bqA4l8iNO7cJ6YD8ie82RerRp9voceBxPIZiwX4y20cTKy5qNaSGr9LxfYq7vDywTipiog==",
  ],
  [
    `@oven/bun-darwin-aarch64@${PINNED_BUN_VERSION}`,
    "sha512-hPERz4IgXCM6Y6GdEEsJAFceyJMt29f3HlFzsvE/k+TQjChRhar6S+JggL35b9VmFfsdxyCOOTPqgnSrdV0etA==",
  ],
  [
    `@oven/bun-windows-x64@${PINNED_BUN_VERSION}`,
    "sha512-UDI3rowMm/tI6DIynpE4XqrOhr+1Ztk1NG707Wxv2nygup+anTswgCwjfjgmIe78LdoRNFrux2GpeolhQGW6vQ==",
  ],
]);

const version = process.env.BUN_VERSION ?? PINNED_BUN_VERSION;
const verifyMetadataOnly = process.argv.includes(VERIFY_METADATA_ONLY_FLAG);

if (version !== PINNED_BUN_VERSION) {
  throw new Error(
    `BUN_VERSION=${version} does not match pinned installer version ${PINNED_BUN_VERSION}`,
  );
}

verifyPackageIntegrity(`bun@${version}`);
verifyPackageIntegrity(`${platformPackageName()}@${version}`);

if (!verifyMetadataOnly) {
  runCommand("npm", ["install", "--global", `bun@${version}`], {
    stdio: "inherit",
  });
  const installedVersion = runCommand("bun", ["--version"], {
    encoding: "utf8",
  }).trim();
  if (installedVersion !== version) {
    throw new Error(`installed Bun ${installedVersion}, expected ${version}`);
  }
  console.log(`Verified Bun ${installedVersion}`);
}

function verifyPackageIntegrity(packageSpec) {
  const expected = PACKAGE_INTEGRITIES.get(packageSpec);
  if (!expected) {
    throw new Error(`missing pinned integrity for ${packageSpec}`);
  }
  const actual = npmViewJson(packageSpec, "dist.integrity");
  if (actual !== expected) {
    throw new Error(
      `${packageSpec} integrity changed: expected ${expected}, got ${actual}`,
    );
  }
}

function npmViewJson(packageSpec, field) {
  const output = runCommand("npm", ["view", packageSpec, field, "--json"], {
    encoding: "utf8",
  }).trim();
  return JSON.parse(output);
}

function runCommand(command, args, options) {
  return execFileSync(command, args, {
    ...options,
    shell: USE_WINDOWS_COMMAND_SHELL,
  });
}

function platformPackageName() {
  const key = `${process.platform}-${process.arch}`;
  if (key === "linux-x64") {
    return "@oven/bun-linux-x64";
  }
  if (key === "darwin-x64") {
    return "@oven/bun-darwin-x64";
  }
  if (key === "darwin-arm64") {
    return "@oven/bun-darwin-aarch64";
  }
  if (key === "win32-x64") {
    return "@oven/bun-windows-x64";
  }
  throw new Error(`unsupported Bun CI platform ${key}`);
}
