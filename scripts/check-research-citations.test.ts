#!/usr/bin/env bun

import { describe, expect, test } from "bun:test";

import { mutableGithubCitationFailures } from "./check-research-citations";

describe("research citation checker", () => {
  test("rejects mutable source links in research notes", () => {
    const text = [
      "https://github.com/rust-lang/rust-analyzer/blob/master/crates/ide-diagnostics/src/lib.rs",
      "https://github.com/astral-sh/ruff/tree/main/crates/ruff_linter/src/rules",
      "https://github.com/biomejs/biome/blob/dev/crates/biome_analyze/src/rule.rs",
      "https://github.com/cordx56/rustowl/tree/release/crates",
      "https://fuchsia.googlesource.com/third_party/rust/+/main/src/tools/clippy/file.rs",
      "https://rust.googlesource.com/rust-clippy/+/stable/book/src/lint_configuration.md",
    ].join("\n");

    const failures = mutableGithubCitationFailures("research.md", text);

    expect(failures).toHaveLength(6);
    expect(failures.join("\n")).toContain("blob/master");
    expect(failures.join("\n")).toContain("tree/main");
    expect(failures.join("\n")).toContain("blob/dev");
    expect(failures.join("\n")).toContain("tree/release");
    expect(failures.join("\n")).toContain("+/main");
    expect(failures.join("\n")).toContain("+/stable");
  });

  test("accepts pinned source links", () => {
    const pinned = [
      "https://github.com/astral-sh/ruff/blob/258ca11050a1b4dca1cc2ed8698261333404b866/crates/ruff_linter/src/checkers/ast/mod.rs",
      "https://rust.googlesource.com/rust-clippy/+/95b24d44a68e3f84c10e392cb19e2db921cbedf8/book/src/lint_configuration.md",
    ].join("\n");

    expect(mutableGithubCitationFailures("research.md", pinned)).toEqual([]);
  });
});
