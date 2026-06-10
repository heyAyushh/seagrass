#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { requiredArgValue as requiredArg } from "./cli-args.ts";

export const releasePackageKinds = ["server", "zed", "vscode", "fuzz-corpus"] as const;
export const releaseBinaryTargets = [
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
  "x86_64-unknown-linux-gnu",
  "x86_64-unknown-linux-musl",
  "x86_64-pc-windows-msvc",
] as const;

type ReleasePackageKind = (typeof releasePackageKinds)[number];
type PackageSelection = ReleasePackageKind | "all";

type PackageReleaseOptions = {
  packages: ReleasePackageKind[];
  version: string;
  tag: string;
  repository: string;
  target?: string;
  binary?: string;
  zedWasm?: string;
  vscodeBaseContentUrl?: string;
  vscodeBaseImagesUrl?: string;
  outDir: string;
  stagingDir: string;
  skipBuild: boolean;
  force: boolean;
  vsceVersion: string;
};

type RunOptions = {
  cwd?: string;
  env?: Record<string, string | undefined>;
};

type PackageContext = {
  repoRoot: string;
  options: PackageReleaseOptions;
};

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const defaultVsceVersion = "3.9.1";
const defaultRepository = "heyAyushh/seagrass";
const serverCargoPackage = "seagrass-cli";
const linuxMuslTarget = "x86_64-unknown-linux-musl";
const linuxMuslCompiler = "musl-gcc";

if (import.meta.main) {
  try {
    const options = parsePackageReleaseArgs(process.argv.slice(2), process.env);
    const outputs = packageRelease({ repoRoot, options });
    console.log("seagrass release packaging passed:");
    for (const output of outputs) {
      console.log(`- ${relative(repoRoot, output)}`);
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(message);
    process.exit(1);
  }
}

export function parsePackageReleaseArgs(
  args: string[],
  env: Record<string, string | undefined> = process.env,
): PackageReleaseOptions {
  const selections: PackageSelection[] = [];
  let version = readReleaseVersion(repoRoot);
  let tag: string | undefined;
  let repository = env.GITHUB_REPOSITORY ?? defaultRepository;
  let target: string | undefined;
  let binary: string | undefined;
  let zedWasm: string | undefined;
  let vscodeBaseContentUrl: string | undefined;
  let vscodeBaseImagesUrl: string | undefined;
  let outDir = "artifacts";
  let stagingDir = "target/seagrass-release-staging";
  let skipBuild = false;
  let force = false;
  let vsceVersion = env.VSCE_VERSION ?? defaultVsceVersion;

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    if (arg === "--all") {
      selections.push("all");
      continue;
    }
    if (arg === "--server") {
      selections.push("server");
      continue;
    }
    if (arg === "--zed") {
      selections.push("zed");
      continue;
    }
    if (arg === "--vscode") {
      selections.push("vscode");
      continue;
    }
    if (arg === "--fuzz-corpus") {
      selections.push("fuzz-corpus");
      continue;
    }
    if (arg === "--skip-build") {
      skipBuild = true;
      continue;
    }
    if (arg === "--force") {
      force = true;
      continue;
    }
    if (arg === "--package" || arg === "--kind") {
      selections.push(packageSelection(requiredArg(args, index, arg)));
      index += 1;
      continue;
    }
    if (arg === "--version") {
      version = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--tag") {
      tag = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--repository") {
      repository = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--target") {
      target = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--binary") {
      binary = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--zed-wasm") {
      zedWasm = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--vscode-base-content-url") {
      vscodeBaseContentUrl = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--vscode-base-images-url") {
      vscodeBaseImagesUrl = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--out-dir") {
      outDir = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--staging-dir") {
      stagingDir = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--vsce-version") {
      vsceVersion = requiredArg(args, index, arg);
      index += 1;
      continue;
    }
    throw new Error(`unknown option: ${arg}`);
  }

  return {
    packages: expandPackageSelections(selections),
    version,
    tag: tag ?? `v${version}`,
    repository,
    target,
    binary,
    zedWasm,
    vscodeBaseContentUrl,
    vscodeBaseImagesUrl,
    outDir,
    stagingDir,
    skipBuild,
    force,
    vsceVersion,
  };
}

export function expectedReleaseAssets(version: string): string[] {
  return [
    ...releaseBinaryTargets.map(
      (target) => `seagrass-${version}-${target}${serverArchiveExtension(target)}`,
    ),
    `seagrass-zed-${version}.tar.gz`,
    `seagrass-vscode-${version}.vsix`,
    `seagrass-${version}-fuzz-corpus.tar.gz`,
  ];
}

export function packageRelease(context: PackageContext): string[] {
  const outputs: string[] = [];
  const outDir = resolveInsideRepo(context.repoRoot, context.options.outDir, "--out-dir");
  const stagingDir = resolveInsideRepo(
    context.repoRoot,
    context.options.stagingDir,
    "--staging-dir",
  );
  mkdirSync(outDir, { recursive: true });
  mkdirSync(stagingDir, { recursive: true });

  for (const packageKind of context.options.packages) {
    if (packageKind === "server") {
      outputs.push(...packageServer(context, stagingDir, outDir));
    } else if (packageKind === "zed") {
      outputs.push(...packageZed(context, stagingDir, outDir));
    } else if (packageKind === "vscode") {
      outputs.push(...packageVsCode(context, outDir));
    } else if (packageKind === "fuzz-corpus") {
      outputs.push(...packageFuzzCorpus(context, stagingDir, outDir));
    }
  }

  return outputs;
}

function packageServer(context: PackageContext, stagingDir: string, outDir: string): string[] {
  const target = context.options.target ?? hostTarget();
  const binaryExt = target.includes("windows") ? ".exe" : "";
  const binaryPath = resolve(
    context.repoRoot,
    context.options.binary ?? `target/${target}/release/seagrass${binaryExt}`,
  );

  if (!context.options.skipBuild) {
    const env = serverBuildEnv(target);
    runChecked(
      "cargo",
      ["build", "--package", serverCargoPackage, "--release", "--locked", "--target", target],
      { cwd: context.repoRoot, env },
    );
  }

  const packageName = `seagrass-${context.options.version}-${target}`;
  const packageDir = preparePackageDir(stagingDir, packageName, context.options.force);
  const destinationBinary = resolve(packageDir, `seagrass${binaryExt}`);
  copyRequired(binaryPath, destinationBinary);
  if (!target.includes("windows")) {
    chmodSync(destinationBinary, 0o755);
  }
  copyRequired(resolve(context.repoRoot, "README.md"), resolve(packageDir, "README.md"));
  copyRequired(resolve(context.repoRoot, "LICENSE"), resolve(packageDir, "LICENSE"));

  return archiveServerPackage(context, stagingDir, outDir, packageName, target);
}

function serverBuildEnv(target: string): Record<string, string> | undefined {
  const env: Record<string, string> = {};
  if (process.platform === "darwin") {
    env.CARGO_PROFILE_RELEASE_LTO = "off";
    env.RUSTFLAGS = "-C embed-bitcode=no";
  }
  if (target === linuxMuslTarget) {
    env.CC_x86_64_unknown_linux_musl = linuxMuslCompiler;
  }
  return Object.keys(env).length > 0 ? env : undefined;
}

function packageZed(context: PackageContext, stagingDir: string, outDir: string): string[] {
  if (!context.options.skipBuild) {
    runChecked("cargo", ["build", "--target", "wasm32-wasip2", "--release", "--locked"], {
      cwd: resolve(context.repoRoot, "editors/zed"),
    });
  }

  const builtWasm = context.options.zedWasm
    ? resolve(context.repoRoot, context.options.zedWasm)
    : zedBuiltWasmCandidates(context.repoRoot).find(existsSync);
  if (!builtWasm) {
    throw new Error(
      [
        "Zed wasm is missing. Checked:",
        ...zedBuiltWasmCandidates(context.repoRoot).map((path) => `  ${relative(context.repoRoot, path)}`),
      ].join("\n"),
    );
  }

  const packageName = `seagrass-zed-${context.options.version}`;
  const packageDir = preparePackageDir(stagingDir, packageName, context.options.force);
  copyRequired(resolve(context.repoRoot, "editors/zed/extension.toml"), resolve(packageDir, "extension.toml"));
  copyRequired(builtWasm, resolve(packageDir, "extension.wasm"));
  copyRequired(resolve(context.repoRoot, "editors/zed/README.md"), resolve(packageDir, "README.md"));
  copyRequired(resolve(context.repoRoot, "LICENSE"), resolve(packageDir, "LICENSE"));

  return archivePackage(context, stagingDir, outDir, packageName);
}

function packageVsCode(context: PackageContext, outDir: string): string[] {
  const vsixName = `seagrass-vscode-${context.options.version}.vsix`;
  const vsixPath = resolve(outDir, vsixName);
  prepareOutputFile(vsixPath, context.options.force);

  if (!context.options.skipBuild) {
    runChecked("bun", ["install", "--frozen-lockfile"], {
      cwd: resolve(context.repoRoot, "editors/vscode"),
    });
    runChecked("bun", ["run", "check"], { cwd: resolve(context.repoRoot, "editors/vscode") });
  }

  runChecked(
    "npm",
    [
      "exec",
      "--yes",
      `@vscode/vsce@${context.options.vsceVersion}`,
      "--",
      "package",
      "--out",
      vsixPath,
      "--allow-missing-repository",
      "--baseContentUrl",
      context.options.vscodeBaseContentUrl ??
        `https://github.com/${context.options.repository}/blob/${context.options.tag}/editors/vscode`,
      "--baseImagesUrl",
      context.options.vscodeBaseImagesUrl ??
        `https://github.com/${context.options.repository}/raw/${context.options.tag}/editors/vscode`,
    ],
    { cwd: resolve(context.repoRoot, "editors/vscode") },
  );
  writeChecksum(vsixPath);
  return [vsixPath, `${vsixPath}.sha256`];
}

function packageFuzzCorpus(context: PackageContext, stagingDir: string, outDir: string): string[] {
  const packageName = `seagrass-${context.options.version}-fuzz-corpus`;
  const packageDir = preparePackageDir(stagingDir, packageName, context.options.force);
  copyRequired(resolve(context.repoRoot, "fuzz/corpus"), resolve(packageDir, "fuzz/corpus"));
  copyRequired(
    resolve(context.repoRoot, "fuzz/fuzz_targets"),
    resolve(packageDir, "fuzz/fuzz_targets"),
  );
  copyRequired(resolve(context.repoRoot, "fuzz/Cargo.toml"), resolve(packageDir, "fuzz/Cargo.toml"));
  copyRequired(
    resolve(context.repoRoot, "docs/release-readiness.json"),
    resolve(packageDir, "docs/release-readiness.json"),
  );
  copyRequired(
    resolve(context.repoRoot, ".github/workflows/fuzz.yaml"),
    resolve(packageDir, ".github/workflows/fuzz.yaml"),
  );
  return archivePackage(context, stagingDir, outDir, packageName);
}

function archivePackage(
  context: PackageContext,
  stagingDir: string,
  outDir: string,
  packageName: string,
): string[] {
  const archivePath = resolve(outDir, `${packageName}.tar.gz`);
  prepareOutputFile(archivePath, context.options.force);
  runChecked("tar", ["-czf", archivePath, "-C", stagingDir, packageName], {
    cwd: context.repoRoot,
  });
  writeChecksum(archivePath);
  return [archivePath, `${archivePath}.sha256`];
}

function archiveServerPackage(
  context: PackageContext,
  stagingDir: string,
  outDir: string,
  packageName: string,
  target: string,
): string[] {
  if (!target.includes("windows")) {
    return archivePackage(context, stagingDir, outDir, packageName);
  }
  const archivePath = resolve(outDir, `${packageName}.zip`);
  prepareOutputFile(archivePath, context.options.force);
  runChecked(
    "powershell",
    [
      "-NoProfile",
      "-Command",
      "Compress-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
      resolve(stagingDir, packageName),
      archivePath,
    ],
    { cwd: context.repoRoot },
  );
  writeChecksum(archivePath);
  return [archivePath, `${archivePath}.sha256`];
}

function serverArchiveExtension(target: string): ".zip" | ".tar.gz" {
  return target.includes("windows") ? ".zip" : ".tar.gz";
}

function zedBuiltWasmCandidates(root: string): string[] {
  return [
    resolve(root, "editors/zed/target/wasm32-wasip2/release/seagrass_zed.wasm"),
    resolve(root, "target/wasm32-wasip2/release/seagrass_zed.wasm"),
  ];
}

function preparePackageDir(stagingDir: string, packageName: string, force: boolean): string {
  const packageDir = resolve(stagingDir, packageName);
  if (existsSync(packageDir)) {
    if (!force) {
      throw new Error(`${packageDir} already exists. Re-run with --force to overwrite package staging.`);
    }
    rmSync(packageDir, { recursive: true, force: true });
  }
  mkdirSync(packageDir, { recursive: true });
  return packageDir;
}

function prepareOutputFile(path: string, force: boolean): void {
  for (const outputPath of [path, `${path}.sha256`]) {
    if (!existsSync(outputPath)) {
      continue;
    }
    if (!force) {
      throw new Error(`${outputPath} already exists. Re-run with --force to overwrite release artifacts.`);
    }
    rmSync(outputPath, { force: true });
  }
}

function copyRequired(from: string, to: string): void {
  if (!existsSync(from)) {
    throw new Error(`required release input is missing: ${from}`);
  }
  mkdirSync(dirname(to), { recursive: true });
  cpSync(from, to, { recursive: true });
}

function writeChecksum(path: string): void {
  writeFileSync(`${path}.sha256`, `${sha256(path)}  ${basename(path)}\n`);
}

function sha256(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function expandPackageSelections(selections: PackageSelection[]): ReleasePackageKind[] {
  if (selections.length === 0 || selections.includes("all")) {
    return [...releasePackageKinds];
  }
  return [...new Set(selections)] as ReleasePackageKind[];
}

function packageSelection(value: string): PackageSelection {
  if (value === "all" || releasePackageKinds.includes(value as ReleasePackageKind)) {
    return value as PackageSelection;
  }
  throw new Error(`unknown release package kind: ${value}`);
}

function resolveInsideRepo(root: string, path: string, label: string): string {
  const resolved = resolve(root, path);
  const relativePath = relative(root, resolved);
  if (relativePath === "" || relativePath.startsWith("..") || isAbsolute(relativePath)) {
    throw new Error(`${label} must stay inside the repository: ${path}`);
  }
  return resolved;
}

function hostTarget(): string {
  const result = spawnSync("rustc", ["-vV"], { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error("rustc -vV failed; pass --target explicitly");
  }
  const host = result.stdout
    .split(/\r?\n/)
    .find((line) => line.startsWith("host: "))
    ?.slice("host: ".length)
    .trim();
  if (!host) {
    throw new Error("rustc -vV did not report a host target; pass --target explicitly");
  }
  return host;
}

function readReleaseVersion(root: string): string {
  return readFileSync(resolve(root, "VERSION"), "utf8").trim();
}

function runChecked(command: string, args: string[], options: RunOptions = {}): void {
  console.log(`$ ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    stdio: "inherit",
    env: { ...process.env, ...(options.env ?? {}) },
  });
  if (result.error) {
    throw new Error(`${command} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`${command} failed with exit code ${result.status}`);
  }
}

function printHelp(): void {
  console.log(`Package Seagrass release artifacts from the repository root.

Usage:
  bun scripts/package-release.ts --all [--force]
  bun scripts/package-release.ts --server --target <triple> [--skip-build]
  bun scripts/package-release.ts --zed [--zed-wasm <path>] [--skip-build]
  bun scripts/package-release.ts --vscode [--vsce-version <version>]

Packages:
  server       Standalone seagrass language-server binary tarball.
  zed          Zed extension tarball with extension.wasm.
  vscode       VS Code VSIX.
  fuzz-corpus  Fuzz corpus and release-readiness evidence tarball.
  all          All packages for the current host target.

Options:
  --version <version>       Release version. Default: VERSION.
  --tag <tag>               Release tag for VSIX metadata. Default: v<version>.
  --repository <owner/repo> Repository for VSIX metadata. Default: ${defaultRepository}.
  --target <triple>         Server target. Default: rustc host target.
  --binary <path>           Existing server binary path for --skip-build.
  --zed-wasm <path>         Existing Zed wasm path for --skip-build.
  --vscode-base-content-url Override VSIX source-link base URL.
  --vscode-base-images-url  Override VSIX image-link base URL.
  --out-dir <path>          Artifact output directory. Default: artifacts.
  --staging-dir <path>      Package staging directory. Default: target/seagrass-release-staging.
  --skip-build              Package existing build outputs.
  --force                   Overwrite package outputs and staging for the selected package names.
  --vsce-version <version>  VS Code packager version. Default: ${defaultVsceVersion}.`);
}
