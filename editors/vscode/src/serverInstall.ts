import { execFile } from "node:child_process";
import { constants as fsConstants } from "node:fs";
import * as fs from "node:fs/promises";
import { createHash } from "node:crypto";
import * as path from "node:path";
import { promisify } from "node:util";
import * as vscode from "vscode";
import {
  DEFAULT_SERVER_COMMAND,
  SERVER_BINARY_NAME,
  binaryExtension,
  cachedBinaryName,
  expectedSha256,
  releaseArchiveName,
  releaseAssetUrl,
  supportedReleasePlatform,
  unsupportedPlatformMessage,
  versionOutputMatches,
} from "./serverInstallModel";

declare const SEAGRASS_VERSION: string;

export const SERVER_VERSION = SEAGRASS_VERSION;

const EXECUTABLE_MODE = 0o755;
const MAX_EXTRACT_SEARCH_DEPTH = 4;
const TEMP_DIR_PREFIX = "download-";
const CHECKSUM_EXTENSION = ".sha256";
const execFileAsync = promisify(execFile);

export async function resolveServerBinary(
  context: vscode.ExtensionContext,
  version: string,
): Promise<string> {
  const config = vscode.workspace.getConfiguration("seagrass");
  if (config.get<boolean>("dev.useCargoFromCheckout", false)) {
    return "cargo";
  }

  const configuredCommand = (config.get<string>("serverCommand") ?? DEFAULT_SERVER_COMMAND).trim();
  if (configuredCommand && configuredCommand !== DEFAULT_SERVER_COMMAND) {
    return configuredCommand;
  }

  if (await pathServerMatchesVersion(version)) {
    return DEFAULT_SERVER_COMMAND;
  }

  const cachedBinary = cachedBinaryPath(context, version);
  if (await cachedBinaryUsable(cachedBinary)) {
    return cachedBinary;
  }

  if (!supportedReleasePlatform()) {
    const message = unsupportedPlatformMessage(process.platform, process.arch);
    void vscode.window.showErrorMessage(message);
    throw new Error(message);
  }

  try {
    return await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: `Seagrass: downloading server v${version}...`,
      },
      () => downloadAndCacheServer(context, version, cachedBinary),
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(`Seagrass server install failed: ${message}`);
    throw error;
  }
}

async function pathServerMatchesVersion(version: string): Promise<boolean> {
  try {
    const stdout = await commandOutput(DEFAULT_SERVER_COMMAND, ["--version"]);
    return versionOutputMatches(stdout, version);
  } catch {
    return false;
  }
}

function cachedBinaryPath(context: vscode.ExtensionContext, version: string): string {
  return path.join(context.globalStorageUri.fsPath, "bin", cachedBinaryName(version));
}

async function cachedBinaryUsable(binaryPath: string): Promise<boolean> {
  try {
    const accessMode = process.platform === "win32"
      ? fsConstants.F_OK
      : fsConstants.F_OK | fsConstants.X_OK;
    await fs.access(binaryPath, accessMode);
    return true;
  } catch {
    return false;
  }
}

async function downloadAndCacheServer(
  context: vscode.ExtensionContext,
  version: string,
  cachedBinary: string,
): Promise<string> {
  const binDir = path.dirname(cachedBinary);
  await fs.mkdir(binDir, { recursive: true });
  const tempDir = await fs.mkdtemp(path.join(binDir, TEMP_DIR_PREFIX));
  const archiveName = releaseArchiveName(version);
  const archivePath = path.join(tempDir, archiveName);
  const checksumPath = path.join(tempDir, `${archiveName}${CHECKSUM_EXTENSION}`);
  const extractDir = path.join(tempDir, "extract");

  try {
    await downloadReleaseAsset(version, archiveName, archivePath);
    await downloadReleaseAsset(version, `${archiveName}${CHECKSUM_EXTENSION}`, checksumPath);
    await verifyArchiveChecksum(archivePath, checksumPath, archiveName);
    await fs.mkdir(extractDir, { recursive: true });
    await extractArchive(archivePath, extractDir);

    const extractedBinary = await findExtractedBinary(
      extractDir,
      `${SERVER_BINARY_NAME}${binaryExtension()}`,
    );
    if (!extractedBinary) {
      throw new Error(`release archive did not contain ${SERVER_BINARY_NAME}`);
    }

    await fs.copyFile(extractedBinary, cachedBinary);
    if (process.platform !== "win32") {
      await fs.chmod(cachedBinary, EXECUTABLE_MODE);
    }
    return cachedBinary;
  } catch (error) {
    await fs.rm(cachedBinary, { force: true });
    await fs.rm(archivePath, { force: true });
    await fs.rm(checksumPath, { force: true });
    throw error;
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
}

async function downloadReleaseAsset(
  version: string,
  assetName: string,
  destinationPath: string,
): Promise<void> {
  const response = await fetch(releaseAssetUrl(version, assetName));
  if (!response.ok) {
    throw new Error(`download failed for ${assetName}: HTTP ${response.status}`);
  }
  await fs.writeFile(destinationPath, Buffer.from(await response.arrayBuffer()));
}

async function verifyArchiveChecksum(
  archivePath: string,
  checksumPath: string,
  archiveName: string,
): Promise<void> {
  const actual = createHash("sha256")
    .update(await fs.readFile(archivePath))
    .digest("hex");
  const expected = expectedSha256(await fs.readFile(checksumPath, "utf8"), archiveName);
  if (actual !== expected) {
    await fs.rm(archivePath, { force: true });
    throw new Error(`sha256 mismatch for ${archiveName}`);
  }
}

async function extractArchive(archivePath: string, extractDir: string): Promise<void> {
  if (process.platform === "win32") {
    await commandOutput("powershell", [
      "-NoProfile",
      "-Command",
      "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
      archivePath,
      extractDir,
    ]);
    return;
  }

  await commandOutput("tar", ["-xzf", archivePath, "-C", extractDir]);
}

async function findExtractedBinary(
  root: string,
  binaryName: string,
  depth = 0,
): Promise<string | undefined> {
  if (depth > MAX_EXTRACT_SEARCH_DEPTH) {
    return undefined;
  }

  const entries = await fs.readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.join(root, entry.name);
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

async function commandOutput(command: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync(command, args, { encoding: "utf8" });
  return stdout;
}
