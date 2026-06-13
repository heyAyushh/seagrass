import { describe, expect, test } from "bun:test";

import {
  SUPPORTED_RELEASE_PLATFORMS,
  cachedBinaryName,
  detectLinuxLibc,
  expectedSha256,
  linuxLibcFromReport,
  releaseArchiveName,
  releaseAssetUrl,
  supportedReleasePlatform,
  versionOutputMatches,
} from "./src/serverInstallModel.ts";

describe("server install model", () => {
  test("maps supported platforms to release archive names", () => {
    expect(SUPPORTED_RELEASE_PLATFORMS.map((platform) => platform.target)).toEqual([
      "aarch64-apple-darwin",
      "x86_64-apple-darwin",
      "x86_64-unknown-linux-gnu",
      "x86_64-unknown-linux-musl",
      "x86_64-pc-windows-msvc",
    ]);
    expect(releaseArchiveName("0.1.2", "darwin", "arm64")).toBe(
      "seagrass-0.1.2-aarch64-apple-darwin.tar.gz",
    );
    expect(releaseArchiveName("0.1.2", "darwin", "x64")).toBe(
      "seagrass-0.1.2-x86_64-apple-darwin.tar.gz",
    );
    expect(releaseArchiveName("0.1.2", "linux", "x64", "gnu")).toBe(
      "seagrass-0.1.2-x86_64-unknown-linux-gnu.tar.gz",
    );
    expect(releaseArchiveName("0.1.2", "linux", "x64", "musl")).toBe(
      "seagrass-0.1.2-x86_64-unknown-linux-musl.tar.gz",
    );
    expect(releaseArchiveName("0.1.2", "win32", "x64")).toBe(
      "seagrass-0.1.2-x86_64-pc-windows-msvc.zip",
    );
  });

  test("rejects platforms outside the release matrix", () => {
    expect(supportedReleasePlatform("linux", "arm64")).toBeUndefined();
    expect(supportedReleasePlatform("linux", "x64", undefined)).toBeUndefined();
    expect(() => releaseArchiveName("0.1.2", "linux", "arm64")).toThrow(
      "prebuilt binary not available",
    );
  });

  test("detects Linux libc from Node diagnostic reports", () => {
    expect(
      linuxLibcFromReport({
        header: { glibcVersionRuntime: "2.39" },
        sharedObjects: [],
      }),
    ).toBe("gnu");
    expect(
      linuxLibcFromReport({
        header: {},
        sharedObjects: ["/lib/ld-musl-x86_64.so.1"],
      }),
    ).toBe("musl");
    expect(linuxLibcFromReport({ header: {}, sharedObjects: [] })).toBe("musl");
    expect(detectLinuxLibc("darwin")).toBeUndefined();
  });

  test("derives release URLs and cache binary names", () => {
    const archiveName = "seagrass-0.1.2-x86_64-unknown-linux-gnu.tar.gz";
    expect(releaseAssetUrl("0.1.2", archiveName)).toBe(
      `https://github.com/heyAyushh/seagrass/releases/download/v0.1.2/${archiveName}`,
    );
    expect(cachedBinaryName("0.1.2", "linux")).toBe("seagrass-0.1.2");
    expect(cachedBinaryName("0.1.2", "win32")).toBe("seagrass-0.1.2.exe");
  });

  test("parses release checksum files for the selected archive", () => {
    const hash = "a".repeat(64);
    const archiveName = "seagrass-0.1.2-x86_64-unknown-linux-gnu.tar.gz";

    expect(expectedSha256(`${hash}  ${archiveName}\n`, archiveName)).toBe(hash);
    expect(() => expectedSha256(`${hash}  other.tar.gz\n`, archiveName)).toThrow(
      "checksum file does not contain",
    );
  });

  test("matches server version output by token", () => {
    expect(versionOutputMatches("seagrass 0.1.2", "0.1.2")).toBe(true);
    expect(versionOutputMatches("seagrass v0.1.2", "0.1.2")).toBe(true);
    expect(versionOutputMatches("seagrass 0.1.20", "0.1.2")).toBe(false);
  });
});
