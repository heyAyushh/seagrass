import { describe, expect, test } from "bun:test";

import {
  expectedReleaseAssets,
  parsePackageReleaseArgs,
  releasePackageKinds,
} from "./package-release.ts";

describe("release package command", () => {
  test("defaults to every release package from the root command", () => {
    const options = parsePackageReleaseArgs(["--version", "1.2.3"], {
      GITHUB_REPOSITORY: "owner/repo",
      VSCE_VERSION: "9.9.9",
    });

    expect(options.packages).toEqual([...releasePackageKinds]);
    expect(options.version).toBe("1.2.3");
    expect(options.tag).toBe("v1.2.3");
    expect(options.repository).toBe("owner/repo");
    expect(options.vsceVersion).toBe("9.9.9");
  });

  test("selects concrete package kinds without duplicates", () => {
    const options = parsePackageReleaseArgs([
      "--server",
      "--package",
      "server",
      "--zed",
      "--version",
      "1.2.3",
      "--target",
      "x86_64-unknown-linux-gnu",
      "--binary",
      "target/x86_64-unknown-linux-gnu/release/seagrass",
      "--zed-wasm",
      "target/wasm32-wasip2/release/seagrass_zed.wasm",
      "--skip-build",
      "--force",
    ]);

    expect(options.packages).toEqual(["server", "zed"]);
    expect(options.target).toBe("x86_64-unknown-linux-gnu");
    expect(options.binary).toBe("target/x86_64-unknown-linux-gnu/release/seagrass");
    expect(options.zedWasm).toBe("target/wasm32-wasip2/release/seagrass_zed.wasm");
    expect(options.skipBuild).toBe(true);
    expect(options.force).toBe(true);
  });

  test("accepts vscode-specific packaging args", () => {
    const options = parsePackageReleaseArgs([
      "--vscode",
      "--version",
      "1.2.3",
      "--tag",
      "v1.2.3",
      "--repository",
      "fork/seagrass",
      "--vscode-base-content-url",
      "https://example.com/blob/v1.2.3/editors/vscode",
      "--vscode-base-images-url",
      "https://example.com/raw/v1.2.3/editors/vscode",
      "--vsce-version",
      "4.0.0",
    ]);

    expect(options.packages).toEqual(["vscode"]);
    expect(options.repository).toBe("fork/seagrass");
    expect(options.vscodeBaseContentUrl).toBe(
      "https://example.com/blob/v1.2.3/editors/vscode",
    );
    expect(options.vscodeBaseImagesUrl).toBe(
      "https://example.com/raw/v1.2.3/editors/vscode",
    );
    expect(options.vsceVersion).toBe("4.0.0");
  });

  test("keeps the published release asset inventory explicit", () => {
    expect(expectedReleaseAssets("1.2.3")).toEqual([
      "seagrass-1.2.3-aarch64-apple-darwin.tar.gz",
      "seagrass-1.2.3-x86_64-apple-darwin.tar.gz",
      "seagrass-1.2.3-x86_64-unknown-linux-gnu.tar.gz",
      "seagrass-1.2.3-x86_64-pc-windows-msvc.tar.gz",
      "seagrass-zed-1.2.3.tar.gz",
      "seagrass-vscode-1.2.3.vsix",
      "seagrass-1.2.3-fuzz-corpus.tar.gz",
    ]);
  });

  test("rejects unknown package kinds", () => {
    expect(() => parsePackageReleaseArgs(["--package", "emacs"])).toThrow(
      "unknown release package kind: emacs",
    );
  });
});
