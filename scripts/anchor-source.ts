import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";

const ANCHOR_GIT_SOURCE_PREFIX = "git+https://github.com/otter-sec/anchor.git";
const CARGO_GIT_CHECKOUTS_DIR = ["git", "checkouts"] as const;
const ANCHOR_SOURCE_REQUIRED_INPUTS = [
  ["Cargo.toml"],
  ["lang/syn/src/parser/accounts/constraints.rs"],
  ["lang/syn/src/parser/accounts/mod.rs"],
  ["lang/error/src/lib.rs", "lang/src/error.rs"],
  ["lang/src/lib.rs"],
  ["spl/src/associated_token.rs"],
  ["spl/src/token.rs"],
  ["spl/src/token_2022.rs"],
  ["spl/src/token_interface.rs"],
] as const;

export function resolveAnchorSourcePath(repoRoot: string): string {
  const override = process.env.SEAGRASS_ANCHOR_PATH;
  if (override) {
    const resolved = resolve(override);
    if (!isAnchorSourcePath(resolved)) {
      fail(`SEAGRASS_ANCHOR_PATH does not point to a supported Anchor checkout: ${resolved}`);
    }
    return resolved;
  }

  if (isAnchorSourcePath(repoRoot)) {
    return repoRoot;
  }

  const cargoGitCheckout = resolveCargoGitAnchorSourcePath(repoRoot);
  if (cargoGitCheckout) {
    return cargoGitCheckout;
  }

  fetchLockedCargoDependencies(repoRoot);
  const fetchedCargoGitCheckout = resolveCargoGitAnchorSourcePath(repoRoot);
  if (fetchedCargoGitCheckout) {
    return fetchedCargoGitCheckout;
  }

  fail(
    [
      "Could not find a supported Anchor source checkout.",
      "Set SEAGRASS_ANCHOR_PATH to an explicit Anchor checkout, or verify `cargo fetch --locked` can fetch the pinned Anchor git dependency.",
    ].join("\n"),
  );
}

export function isAnchorSourcePath(path: string): boolean {
  return ANCHOR_SOURCE_REQUIRED_INPUTS.every((alternatives) =>
    alternatives.some((input) => existsSync(resolve(path, input))),
  );
}

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

function resolveCargoGitAnchorSourcePath(repoRoot: string): string | undefined {
  const commit = anchorCommitFromCargoLock(repoRoot);
  if (!commit) {
    return undefined;
  }

  const checkoutRoot = resolve(cargoHome(), ...CARGO_GIT_CHECKOUTS_DIR);
  if (!existsSync(checkoutRoot)) {
    return undefined;
  }

  const shortCommit = commit.slice(0, 7);
  for (const checkout of safeReadDir(checkoutRoot)) {
    if (!checkout.isDirectory() || !checkout.name.startsWith("anchor-")) {
      continue;
    }
    const candidate = resolve(checkoutRoot, checkout.name, shortCommit);
    if (isAnchorSourcePath(candidate)) {
      return candidate;
    }
  }

  return undefined;
}

function fetchLockedCargoDependencies(repoRoot: string): void {
  const manifestPath = resolve(repoRoot, "Cargo.toml");
  if (!existsSync(manifestPath)) {
    return;
  }
  const result = spawnSync("cargo", ["fetch", "--locked", "--manifest-path", manifestPath], {
    cwd: repoRoot,
    stdio: "inherit",
  });
  if (result.error) {
    fail(`cargo fetch failed to start while resolving Anchor source: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`cargo fetch failed while resolving Anchor source with exit code ${result.status}`);
  }
}

function anchorCommitFromCargoLock(repoRoot: string): string | undefined {
  const lockPath = resolve(repoRoot, "Cargo.lock");
  if (!existsSync(lockPath)) {
    return undefined;
  }

  const lock = readFileSync(lockPath, "utf8");
  const escapedSource = ANCHOR_GIT_SOURCE_PREFIX.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = lock.match(new RegExp(`source = "${escapedSource}[^"]*#([a-f0-9]{7,40})"`));
  return match?.[1];
}

function cargoHome(): string {
  return process.env.CARGO_HOME ? resolve(process.env.CARGO_HOME) : resolve(homedir(), ".cargo");
}

function safeReadDir(path: string) {
  try {
    return readdirSync(path, { withFileTypes: true });
  } catch {
    return [];
  }
}
