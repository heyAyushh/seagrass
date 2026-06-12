#!/usr/bin/env bun

import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { constants as fsConstants } from "node:fs";
import { access, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { promisify } from "node:util";
import {
  SERVER_BINARY_NAME,
  binaryExtension,
  expectedSha256,
  releaseArchiveName,
  releaseAssetUrl,
  versionOutputMatches,
} from "../editors/vscode/src/serverInstallModel.ts";

const execFileAsync = promisify(execFile);
const CHECKSUM_EXTENSION = ".sha256";
const MAX_EXTRACT_SEARCH_DEPTH = 4;

const tag = argumentValue("--tag") ?? process.env.GITHUB_REF_NAME;
if (!tag) {
  console.error("usage: bun scripts/smoke-release-install.ts --tag v0.1.2");
  process.exit(1);
}

const version = tag.replace(/^v/, "");
const tempDir = await mkdtemp(join(tmpdir(), "seagrass-release-smoke-"));

try {
  const archiveName = releaseArchiveName(version);
  const archivePath = join(tempDir, archiveName);
  const checksumPath = join(tempDir, `${archiveName}${CHECKSUM_EXTENSION}`);
  const extractDir = join(tempDir, "extract");

  await downloadReleaseFile(version, archiveName, archivePath);
  await downloadReleaseFile(version, `${archiveName}${CHECKSUM_EXTENSION}`, checksumPath);
  await verifyArchiveChecksum(archivePath, checksumPath, archiveName);
  await extractArchive(archivePath, extractDir);

  const binaryPath = await findExtractedBinary(
    extractDir,
    `${SERVER_BINARY_NAME}${binaryExtension()}`,
  );
  if (!binaryPath) {
    throw new Error(`release archive did not contain ${SERVER_BINARY_NAME}`);
  }
  await assertExecutable(binaryPath);
  const { stdout } = await execFileAsync(binaryPath, ["--version"], { encoding: "utf8" });
  if (!versionOutputMatches(stdout, version)) {
    throw new Error(`${basename(binaryPath)} --version did not contain ${version}: ${stdout}`);
  }
  console.log(`release install smoke passed: ${archiveName}`);
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

function argumentValue(flag: string): string | undefined {
  const index = process.argv.indexOf(flag);
  return index === -1 ? undefined : process.argv[index + 1];
}

async function downloadReleaseFile(
  version: string,
  assetName: string,
  destinationPath: string,
): Promise<void> {
  const response = await fetch(releaseAssetUrl(version, assetName));
  if (!response.ok) {
    throw new Error(`download failed for ${assetName}: HTTP ${response.status}`);
  }
  await writeFile(destinationPath, Buffer.from(await response.arrayBuffer()));
}

async function verifyArchiveChecksum(
  archivePath: string,
  checksumPath: string,
  archiveName: string,
): Promise<void> {
  const actual = createHash("sha256")
    .update(await readFile(archivePath))
    .digest("hex");
  const expected = expectedSha256(await readFile(checksumPath, "utf8"), archiveName);
  if (actual !== expected) {
    throw new Error(`sha256 mismatch for ${archiveName}`);
  }
}

async function extractArchive(archivePath: string, extractDir: string): Promise<void> {
  await mkdir(extractDir, { recursive: true });
  if (process.platform === "win32") {
    await execFileAsync("powershell", [
      "-NoProfile",
      "-Command",
      "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
      archivePath,
      extractDir,
    ]);
    return;
  }
  await execFileAsync("tar", ["-xzf", archivePath, "-C", extractDir]);
}

async function findExtractedBinary(
  root: string,
  binaryName: string,
  depth = 0,
): Promise<string | undefined> {
  if (depth > MAX_EXTRACT_SEARCH_DEPTH) {
    return undefined;
  }
  const entries = await readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = join(root, entry.name);
    if (entry.isFile() && entry.name === binaryName) {
      return entryPath;
    }
    if (entry.isDirectory()) {
      const nested = await findExtractedBinary(entryPath, binaryName, depth + 1);
      if (nested) {
        return nested;
      }
    }
  }
  return undefined;
}

async function assertExecutable(binaryPath: string): Promise<void> {
  const mode = process.platform === "win32" ? fsConstants.F_OK : fsConstants.F_OK | fsConstants.X_OK;
  await access(binaryPath, mode);
}
