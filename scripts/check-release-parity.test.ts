import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { checkReleaseParity } from "./check-release-parity.ts";

const repoRoot = resolve(import.meta.dir, "..");
const releaseWorkflow = readFileSync(resolve(repoRoot, ".github/workflows/release.yaml"), "utf8");
const installScript = readFileSync(resolve(repoRoot, "scripts/install.sh"), "utf8");

describe("release parity checker", () => {
  test("accepts current release and installer contracts", () => {
    const result = checkReleaseParity({ releaseWorkflow, installScript });

    expect(result.failures).toEqual([]);
  });

  test("fails when a release target string drifts", () => {
    const mutatedWorkflow = releaseWorkflow.replace(
      "x86_64-unknown-linux-gnu",
      "x86_64-unknown-linux-gnu-drift",
    );
    const result = checkReleaseParity({
      releaseWorkflow: mutatedWorkflow,
      installScript,
    });

    expect(result.failures.join("\n")).toContain("release.yaml does not build");
  });

  test("fails when install.sh selects an unknown target", () => {
    const mutatedInstallScript = installScript.replace(
      "x86_64-unknown-linux-musl",
      "x86_64-unknown-linux-musl-drift",
    );
    const result = checkReleaseParity({
      releaseWorkflow,
      installScript: mutatedInstallScript,
    });

    expect(result.failures.join("\n")).toContain("install.sh selects");
  });
});
