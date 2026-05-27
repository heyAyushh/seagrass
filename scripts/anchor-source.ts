import { existsSync } from "node:fs";
import { resolve } from "node:path";

const ANCHOR_SOURCE_FALLBACKS = ["../upstream-anchor", "../anchor-next"] as const;
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

  const fallback = [repoRoot, ...ANCHOR_SOURCE_FALLBACKS.map((path) => resolve(repoRoot, path))].find(
    isAnchorSourcePath,
  );
  return fallback ?? repoRoot;
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
