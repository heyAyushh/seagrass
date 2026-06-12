#!/usr/bin/env bun

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  SUPPORTED_RELEASE_PLATFORMS,
  releaseArchiveName,
} from "../editors/vscode/src/serverInstallModel.ts";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const releaseWorkflowPath = resolve(repoRoot, ".github/workflows/release.yaml");
const installScriptPath = resolve(repoRoot, "scripts/install.sh");
const sampleVersion = "0.1.2";

type ReleaseTarget = {
  target: string;
  archiveExtension: string;
};

const UNINSTALLABLE_TARGETS: { target: string; reason: string }[] = [
  // linux-aarch64 is intentionally absent from release.yaml today; adding it later
  // should be one release matrix row plus this parity checker staying green.
];

if (import.meta.main) {
  const result = checkReleaseParity({
    releaseWorkflow: readFileSync(releaseWorkflowPath, "utf8"),
    installScript: readFileSync(installScriptPath, "utf8"),
  });
  if (result.failures.length > 0) {
    console.error(`\nrelease parity check failed:\n${result.failures.join("\n")}`);
    process.exit(1);
  }
  console.log(`release parity check passed: ${result.releaseTargets.length} release targets`);
}

export function checkReleaseParity(inputs: {
  releaseWorkflow: string;
  installScript: string;
}): {
  failures: string[];
  releaseTargets: ReleaseTarget[];
} {
  const releaseTargets = releaseMatrixTargets(inputs.releaseWorkflow);
  const releaseByTarget = new Map(
    releaseTargets.map((target) => [target.target, target.archiveExtension]),
  );
  const installTargets = installScriptTargets(inputs.installScript);
  const vscodeTargets = vscodeReleaseTargets();
  const failures = [
    ...unknownInstallerTargets("install.sh", installTargets, releaseByTarget),
    ...unknownInstallerTargets("VS Code", vscodeTargets, releaseByTarget),
    ...archiveNameFailures(vscodeTargets, releaseByTarget),
    ...unreachableReleaseTargets(releaseTargets, installTargets, vscodeTargets),
  ];

  if (UNINSTALLABLE_TARGETS.length > 0) {
    failures.push("UNINSTALLABLE_TARGETS must stay empty after musl reconciliation");
  }

  return { failures, releaseTargets };
}

export function releaseMatrixTargets(workflow: string): ReleaseTarget[] {
  const targets = [];
  let currentTarget: string | undefined;
  for (const line of workflow.split(/\r?\n/)) {
    const targetMatch = line.match(/^\s*-\s+target:\s+([A-Za-z0-9_-]+(?:-[A-Za-z0-9_-]+)+)\s*$/);
    if (targetMatch) {
      currentTarget = targetMatch[1];
      continue;
    }
    const archiveMatch = line.match(/^\s*archive_ext:\s+"([^"]+)"\s*$/);
    if (archiveMatch && currentTarget) {
      targets.push({
        target: currentTarget,
        archiveExtension: archiveMatch[1],
      });
      currentTarget = undefined;
    }
  }
  assertUniqueTargets(targets, "release.yaml build matrix");
  return targets;
}

export function installScriptTargets(script: string): ReleaseTarget[] {
  const targets = [...script.matchAll(/echo "([A-Za-z0-9_-]+(?:-[A-Za-z0-9_-]+)+)"/g)]
    .map((match) => match[1])
    .filter((target) => target.includes("-unknown-") || target.includes("-apple-"))
    .map((target) => ({ target, archiveExtension: ".tar.gz" }));
  assertUniqueTargets(targets, "scripts/install.sh targets");
  return targets;
}

function vscodeReleaseTargets(): ReleaseTarget[] {
  const targets = SUPPORTED_RELEASE_PLATFORMS.map((platform) => ({
    target: platform.target,
    archiveExtension: platform.archiveExtension,
  })).sort(compareTargets);
  assertUniqueTargets(targets, "VS Code install model targets");
  return targets;
}

function unknownInstallerTargets(
  label: string,
  targets: ReleaseTarget[],
  releaseByTarget: Map<string, string>,
): string[] {
  return targets.flatMap((target) => {
    const archiveExtension = releaseByTarget.get(target.target);
    if (!archiveExtension) {
      return [`${label} selects ${target.target}, but release.yaml does not build it`];
    }
    return archiveExtension === target.archiveExtension
      ? []
      : [
          `${label} selects ${target.target}${target.archiveExtension}, but release.yaml builds ${archiveExtension}`,
        ];
  });
}

function archiveNameFailures(
  targets: ReleaseTarget[],
  releaseByTarget: Map<string, string>,
): string[] {
  return SUPPORTED_RELEASE_PLATFORMS.flatMap((platform) => {
    const expectedExtension = releaseByTarget.get(platform.target);
    if (!expectedExtension) {
      return [];
    }
    const archiveName = releaseArchiveName(sampleVersion, platform.platform, platform.archs[0]);
    const expected = `seagrass-${sampleVersion}-${platform.target}${expectedExtension}`;
    return archiveName === expected
      ? []
      : [`VS Code archive name for ${platform.platform}/${platform.archs[0]} was ${archiveName}, expected ${expected}`];
  });
}

function unreachableReleaseTargets(
  releaseTargets: ReleaseTarget[],
  installTargets: ReleaseTarget[],
  vscodeTargets: ReleaseTarget[],
): string[] {
  const installableTargets = new Set([
    ...installTargets.map((target) => target.target),
    ...vscodeTargets.map((target) => target.target),
    ...UNINSTALLABLE_TARGETS.map((target) => target.target),
  ]);
  return releaseTargets
    .filter((target) => !installableTargets.has(target.target))
    .map((target) => `${target.target} is built by release.yaml but no installer can select it`);
}

function assertUniqueTargets(targets: ReleaseTarget[], label: string): void {
  const seenTargets = new Set<string>();
  for (const target of targets) {
    if (seenTargets.has(target.target)) {
      throw new Error(`${label} contains duplicate target ${target.target}`);
    }
    seenTargets.add(target.target);
  }
}

function compareTargets(left: ReleaseTarget, right: ReleaseTarget): number {
  return left.target.localeCompare(right.target);
}
