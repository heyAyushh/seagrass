export const GITHUB_RELEASE_BASE =
  "https://github.com/heyAyushh/seagrass/releases/download";
export const SERVER_BINARY_NAME = "seagrass";
export const DEFAULT_SERVER_COMMAND = SERVER_BINARY_NAME;

const DARWIN_ARM64_TARGET = "aarch64-apple-darwin";
const DARWIN_X64_TARGET = "x86_64-apple-darwin";
const LINUX_X64_GNU_TARGET = "x86_64-unknown-linux-gnu";
const LINUX_X64_MUSL_TARGET = "x86_64-unknown-linux-musl";
const WINDOWS_X64_TARGET = "x86_64-pc-windows-msvc";
const LINUX_PLATFORM = "linux";
const LINUX_GNU_LIBC = "gnu";
const LINUX_MUSL_LIBC = "musl";
const TARBALL_EXTENSION = ".tar.gz";
const ZIP_EXTENSION = ".zip";
const WINDOWS_BINARY_EXTENSION = ".exe";
const SHA256_HEX_LENGTH = 64;

export type LinuxLibc = typeof LINUX_GNU_LIBC | typeof LINUX_MUSL_LIBC;

export type LinuxLibcReport = {
  header?: {
    glibcVersionRuntime?: unknown;
  };
  sharedObjects?: unknown;
};

export type ReleasePlatform = {
  target: string;
  archiveExtension: string;
};

export type ReleasePlatformMapping = ReleasePlatform & {
  platform: string;
  archs: string[];
  linuxLibc?: LinuxLibc;
};

export const SUPPORTED_RELEASE_PLATFORMS: readonly ReleasePlatformMapping[] = [
  {
    platform: "darwin",
    archs: ["arm64", "aarch64"],
    target: DARWIN_ARM64_TARGET,
    archiveExtension: TARBALL_EXTENSION,
  },
  {
    platform: "darwin",
    archs: ["x64"],
    target: DARWIN_X64_TARGET,
    archiveExtension: TARBALL_EXTENSION,
  },
  {
    platform: LINUX_PLATFORM,
    archs: ["x64"],
    linuxLibc: LINUX_GNU_LIBC,
    target: LINUX_X64_GNU_TARGET,
    archiveExtension: TARBALL_EXTENSION,
  },
  {
    platform: LINUX_PLATFORM,
    archs: ["x64"],
    linuxLibc: LINUX_MUSL_LIBC,
    target: LINUX_X64_MUSL_TARGET,
    archiveExtension: TARBALL_EXTENSION,
  },
  {
    platform: "win32",
    archs: ["x64"],
    target: WINDOWS_X64_TARGET,
    archiveExtension: ZIP_EXTENSION,
  },
];

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
  linuxLibc: LinuxLibc | undefined = detectLinuxLibc(platform),
): string {
  const releasePlatform = supportedReleasePlatform(platform, arch, linuxLibc);
  if (!releasePlatform) {
    throw new Error(unsupportedPlatformMessage(platform, arch, linuxLibc));
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

export function unsupportedPlatformMessage(
  platform: string,
  arch: string,
  linuxLibc: LinuxLibc | undefined = detectLinuxLibc(platform),
): string {
  const platformLabel = platform === LINUX_PLATFORM
    ? `${platform}/${arch}/${linuxLibc ?? "unknown-libc"}`
    : `${platform}/${arch}`;
  return `Seagrass: prebuilt binary not available for ${platformLabel}. Install from source: cargo install seagrass-cli --locked`;
}

export function supportedReleasePlatform(
  platform: string = process.platform,
  arch: string = process.arch,
  linuxLibc: LinuxLibc | undefined = detectLinuxLibc(platform),
): ReleasePlatform | undefined {
  const releasePlatform = SUPPORTED_RELEASE_PLATFORMS.find(
    (entry) =>
      entry.platform === platform &&
      entry.archs.includes(arch) &&
      (entry.platform !== LINUX_PLATFORM || entry.linuxLibc === linuxLibc),
  );
  return releasePlatform
    ? {
        target: releasePlatform.target,
        archiveExtension: releasePlatform.archiveExtension,
      }
    : undefined;
}

export function detectLinuxLibc(platform: string = process.platform): LinuxLibc | undefined {
  if (platform !== LINUX_PLATFORM) {
    return undefined;
  }
  if (process.platform !== LINUX_PLATFORM) {
    return undefined;
  }

  return linuxLibcFromReport(process.report?.getReport());
}

export function linuxLibcFromReport(report: LinuxLibcReport | undefined): LinuxLibc | undefined {
  if (!report) {
    return undefined;
  }

  const glibcVersion = report.header?.glibcVersionRuntime;
  if (typeof glibcVersion === "string" && glibcVersion.length > 0) {
    return LINUX_GNU_LIBC;
  }

  const sharedObjects = Array.isArray(report.sharedObjects) ? report.sharedObjects : [];
  if (
    sharedObjects.some(
      (sharedObject) => typeof sharedObject === "string" && sharedObject.includes("musl"),
    )
  ) {
    return LINUX_MUSL_LIBC;
  }

  return LINUX_MUSL_LIBC;
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
