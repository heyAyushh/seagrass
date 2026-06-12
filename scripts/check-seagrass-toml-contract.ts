#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const suppressionPath = resolve(repoRoot, "src/lsp/diagnostics/suppression.rs");
const scannedRoots = ["docs", "skills", "editors"];
const markdownExtension = ".md";
const seagrassTomlConst = "SEAGRASS_TOML_KEY_PATHS";

if (import.meta.main) {
  const result = checkSeagrassTomlContract();
  if (result.failures.length > 0) {
    console.error(`\nSeagrass.toml contract check failed:\n${result.failures.join("\n")}`);
    process.exit(1);
  }
  console.log(`Seagrass.toml contract check passed: ${result.blockCount} attributed TOML blocks`);
}

export function checkSeagrassTomlContract(): {
  failures: string[];
  blockCount: number;
} {
  const canon = new Set(seagrassTomlKeyPaths(readFileSync(suppressionPath, "utf8")));
  const blocks = scannedRoots
    .flatMap((root) => markdownFiles(resolve(repoRoot, root)))
    .flatMap((path) => attributedTomlBlocks(path));
  const failures = blocks.flatMap((block) => {
    const keyPaths = tomlKeyPaths(block.text);
    return keyPaths
      .filter((keyPath) => !canon.has(keyPath))
      .map((keyPath) => `${block.location}: unparsed Seagrass.toml key path ${keyPath}`);
  });
  return { failures, blockCount: blocks.length };
}

export function seagrassTomlKeyPaths(source: string): string[] {
  const block = rustStringArrayBlock(source, seagrassTomlConst);
  const keys = [...block.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
  if (keys.length === 0) {
    throw new Error(`${seagrassTomlConst} did not contain any key paths`);
  }
  return keys;
}

function markdownFiles(root: string): string[] {
  if (!existsSync(root)) {
    return [];
  }
  return readdirSync(root)
    .flatMap((entry) => {
      const path = join(root, entry);
      const stat = statSync(path);
      if (stat.isDirectory()) {
        return markdownFiles(path);
      }
      return stat.isFile() && extname(path) === markdownExtension ? [path] : [];
    })
    .sort(compareStrings);
}

function attributedTomlBlocks(path: string): { location: string; text: string }[] {
  const lines = readFileSync(path, "utf8").split(/\r?\n/);
  const blocks: { location: string; text: string }[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    if (!/^```toml\s*$/.test(lines[index]) || !isAttributedToSeagrassToml(lines, index)) {
      continue;
    }
    const startLine = index + 1;
    const textLines = [];
    index += 1;
    while (index < lines.length && !/^```\s*$/.test(lines[index])) {
      textLines.push(lines[index]);
      index += 1;
    }
    blocks.push({
      location: `${relative(repoRoot, path)}:${startLine}`,
      text: textLines.join("\n"),
    });
  }
  return blocks;
}

function isAttributedToSeagrassToml(lines: string[], fenceIndex: number): boolean {
  const contextStart = Math.max(0, fenceIndex - 4);
  return lines.slice(contextStart, fenceIndex).join("\n").includes("Seagrass.toml");
}

function tomlKeyPaths(text: string): string[] {
  let section = "";
  const paths = [];
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.replace(/#.*/, "").trim();
    if (line === "") {
      continue;
    }
    const sectionMatch = line.match(/^\[([A-Za-z0-9_.-]+)\]$/);
    if (sectionMatch) {
      section = sectionMatch[1];
      continue;
    }
    const keyMatch = line.match(/^([A-Za-z0-9_.-]+)\s*=/);
    if (keyMatch) {
      paths.push(section ? `${section}.${keyMatch[1]}` : keyMatch[1]);
    }
  }
  return paths;
}

function rustStringArrayBlock(source: string, constName: string): string {
  const declaration = new RegExp(
    `pub(?:\\(crate\\))? const ${constName}: &\\[&str\\] = &\\[(?<body>[\\s\\S]*?)\\];`,
  );
  const match = source.match(declaration);
  const body = match?.groups?.body;
  if (!body) {
    throw new Error(`could not find ${constName} in ${relative(repoRoot, suppressionPath)}`);
  }
  return body;
}

function compareStrings(left: string, right: string): number {
  return left.localeCompare(right);
}
