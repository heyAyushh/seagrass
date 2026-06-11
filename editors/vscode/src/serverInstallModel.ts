export const GITHUB_RELEASE_BASE =
  "https://github.com/heyAyushh/seagrass/releases/download";
export const SERVER_BINARY_NAME = "seagrass";
export const DEFAULT_SERVER_COMMAND = SERVER_BINARY_NAME;

const DARWIN_ARM64_TARGET = "aarch64-apple-darwin";
const DARWIN_X64_TARGET = "x86_64-apple-darwin";
const LINUX_X64_TARGET = "x86_64-unknown-linux-gnu";
const WINDOWS_X64_TARGET = "x86_64-pc-windows-msvc";
const TARBALL_EXTENSION = ".tar.gz";
const ZIP_EXTENSION = ".zip";
const WINDOWS_BINARY_EXTENSION = ".exe";
const SHA256_HEX_LENGTH = 64;

export type ReleasePlatform = {
  target: string;
  archiveExtension: string;
};

export function binaryExtension(platform: string = process.platform): string {
  return platform === "win32" ? WINDOWS_BINARY_EXTENSION : "";
}

export function cachedBinaryName(version: string, platform: string = process.platform): string {
  return `${SERVER_BINARY_NAME}-${version}${binaryExtension(platform)}`;
}

export function releaseArchiveName(
  version: string,
  platform: string = process.platform,
  arch: string = process.arch,
): string {
  const releasePlatform = supportedReleasePlatform(platform, arch);
  if (!releasePlatform) {
    throw new Error(unsupportedPlatformMessage(platform, arch));
  }
  return `${SERVER_BINARY_NAME}-${version}-${releasePlatform.target}${releasePlatform.archiveExtension}`;
}

export function releaseAssetUrl(version: string, archiveName: string): string {
  return `${GITHUB_RELEASE_BASE}/v${version}/${archiveName}`;
}

export function versionOutputMatches(output: string, version: string): boolean {
  return output
    .split(/\s+/)
    .some((token) => token.replace(/^v/, "") === version);
}

export function unsupportedPlatformMessage(platform: string, arch: string): string {
  return `Seagrass: prebuilt binary not available for ${platform}/${arch}. Install from source: cargo install seagrass-cli --locked`;
}

export function supportedReleasePlatform(
  platform: string = process.platform,
  arch: string = process.arch,
): ReleasePlatform | undefined {
  if (platform === "darwin" && (arch === "arm64" || arch === "aarch64")) {
    return { target: DARWIN_ARM64_TARGET, archiveExtension: TARBALL_EXTENSION };
  }
  if (platform === "darwin" && arch === "x64") {
    return { target: DARWIN_X64_TARGET, archiveExtension: TARBALL_EXTENSION };
  }
  if (platform === "linux" && arch === "x64") {
    return { target: LINUX_X64_TARGET, archiveExtension: TARBALL_EXTENSION };
  }
  if (platform === "win32" && arch === "x64") {
    return { target: WINDOWS_X64_TARGET, archiveExtension: ZIP_EXTENSION };
  }
  return undefined;
}

export function expectedSha256(checksumText: string, archiveName: string): string {
  for (const line of checksumText.split(/\r?\n/)) {
    const parts = line.trim().split(/\s+/).filter(Boolean);
    if (parts.length === 0) {
      continue;
    }
    const [hash, fileName] = parts;
    if (isSha256(hash) && (!fileName || fileName.replace(/^\*/, "") === archiveName)) {
      return hash.toLowerCase();
    }
  }
  throw new Error(`checksum file does not contain a sha256 entry for ${archiveName}`);
}

function isSha256(value: string): boolean {
  return value.length === SHA256_HEX_LENGTH && /^[a-f0-9]+$/i.test(value);
}
