#!/usr/bin/env bun

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const serverTypesPath = resolve(repoRoot, "src/runtime/server_types/mod.rs");
const manifestPath = resolve(repoRoot, "editors/recognized-settings.json");
const recognizedSettingsConst = "RECOGNIZED_SETTING_KEYS";
const fixMode = process.argv.includes("--fix");

type SettingsManifest = {
  source: string;
  recognizedSettings: string[];
};

if (import.meta.main) {
  const result = checkRecognizedSettings({ fixMode });
  if (result.failures.length > 0) {
    console.error(`\nrecognized settings check failed:\n${result.failures.join("\n")}`);
    process.exit(1);
  }
  console.log(`recognized settings check passed: ${result.settingCount} keys`);
}

export function checkRecognizedSettings(options: { fixMode?: boolean } = {}): {
  failures: string[];
  settingCount: number;
} {
  const keys = recognizedSettingsFromRust(readFileSync(serverTypesPath, "utf8"));
  const expected = manifestText(keys);
  const actual = existsSync(manifestPath) ? readFileSync(manifestPath, "utf8") : "";

  if (actual !== expected) {
    if (options.fixMode) {
      writeFileSync(manifestPath, expected);
      return { failures: [], settingCount: keys.length };
    }
    return {
      failures: [
        `${relative(repoRoot, manifestPath)} is stale; run bun scripts/check-recognized-settings.ts --fix`,
      ],
      settingCount: keys.length,
    };
  }

  return { failures: [], settingCount: keys.length };
}

export function recognizedSettingsFromRust(source: string): string[] {
  const block = rustStringArrayBlock(source, recognizedSettingsConst);
  const keys = [...block.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
  if (keys.length === 0) {
    throw new Error(`${recognizedSettingsConst} did not contain any string keys`);
  }
  assertSortedUnique(keys, recognizedSettingsConst);
  return keys;
}

function rustStringArrayBlock(source: string, constName: string): string {
  const declaration = new RegExp(
    `pub(?:\\(crate\\))? const ${constName}: &\\[&str\\] = &\\[(?<body>[\\s\\S]*?)\\];`,
  );
  const match = source.match(declaration);
  const body = match?.groups?.body;
  if (!body) {
    throw new Error(`could not find ${constName} in ${relative(repoRoot, serverTypesPath)}`);
  }
  return body;
}

function assertSortedUnique(keys: string[], label: string): void {
  for (let index = 1; index < keys.length; index += 1) {
    if (keys[index - 1] >= keys[index]) {
      throw new Error(`${label} must be sorted and unique near ${keys[index - 1]} / ${keys[index]}`);
    }
  }
}

function manifestText(keys: string[]): string {
  const manifest: SettingsManifest = {
    source: "src/runtime/server_types/mod.rs::RECOGNIZED_SETTING_KEYS",
    recognizedSettings: keys,
  };
  return `${JSON.stringify(manifest, null, 2)}\n`;
}
