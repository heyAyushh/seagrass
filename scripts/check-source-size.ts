#!/usr/bin/env bun

import { readFileSync } from "node:fs";
import { resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

import { rustFiles } from "./script-paths.ts";

const MAX_SOURCE_LINES = 800;
const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const sourceRoot = resolve(repoRoot, "src");
const generatedRoots = new Set([resolve(sourceRoot, "anchor/generated")]);

const oversized = rustFiles(sourceRoot, { ignoredRoots: generatedRoots })
  .map((path) => ({ path, lines: lineCount(path) }))
  .filter((entry) => entry.lines > MAX_SOURCE_LINES)
  .sort((left, right) => right.lines - left.lines);

if (oversized.length > 0) {
  console.error(`LSP source files must stay at or below ${MAX_SOURCE_LINES} LOC.`);
  console.error(
    oversized.map((entry) => `${relative(repoRoot, entry.path)}: ${entry.lines}`).join("\n"),
  );
  process.exit(1);
}

console.log(`seagrass source size check passed: max ${MAX_SOURCE_LINES} LOC`);

function lineCount(path) {
  const text = readFileSync(path, "utf8");
  const lines = text.split("\n").length;
  return text.endsWith("\n") ? lines - 1 : lines;
}
