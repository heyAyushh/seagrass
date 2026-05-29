import { afterEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

import { resolveAnchorSourcePath } from "./anchor-source.ts";

const requiredAnchorInputs = [
  "Cargo.toml",
  "lang/syn/src/parser/accounts/constraints.rs",
  "lang/syn/src/parser/accounts/mod.rs",
  "lang/error/src/lib.rs",
  "lang/src/lib.rs",
  "spl/src/associated_token.rs",
  "spl/src/token.rs",
  "spl/src/token_2022.rs",
  "spl/src/token_interface.rs",
] as const;

const originalCargoHome = process.env.CARGO_HOME;
const originalAnchorPath = process.env.SEAGRASS_ANCHOR_PATH;

afterEach(() => {
  restoreEnv("CARGO_HOME", originalCargoHome);
  restoreEnv("SEAGRASS_ANCHOR_PATH", originalAnchorPath);
});

describe("Anchor source resolver", () => {
  test("uses the pinned Cargo git checkout instead of sibling mirrors", () => {
    const root = mkdtempSync(resolve(tmpdir(), "seagrass-anchor-source-"));
    const repoRoot = resolve(root, "seagrass");
    const cargoHome = resolve(root, "cargo-home");
    const cargoAnchor = resolve(cargoHome, "git/checkouts/anchor-test/4addac5");
    const siblingMirror = resolve(root, "upstream-anchor");

    mkdirSync(repoRoot, { recursive: true });
    writeFileSync(
      resolve(repoRoot, "Cargo.lock"),
      [
        "[[package]]",
        'name = "anchor-syn"',
        'source = "git+https://github.com/otter-sec/anchor.git?rev=4addac53#4addac5304bf2d2378448e86b5ee374c99037585"',
      ].join("\n"),
    );
    writeAnchorSource(cargoAnchor);
    writeAnchorSource(siblingMirror);

    process.env.CARGO_HOME = cargoHome;
    delete process.env.SEAGRASS_ANCHOR_PATH;

    expect(resolveAnchorSourcePath(repoRoot)).toBe(cargoAnchor);
  });
});

function writeAnchorSource(root: string): void {
  for (const input of requiredAnchorInputs) {
    const path = resolve(root, input);
    mkdirSync(resolve(path, ".."), { recursive: true });
    writeFileSync(path, "");
  }
}

function restoreEnv(name: string, value: string | undefined): void {
  if (value === undefined) {
    delete process.env[name];
  } else {
    process.env[name] = value;
  }
}
