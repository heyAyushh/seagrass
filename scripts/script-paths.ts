import { existsSync, readdirSync, statSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

type RustFileOptions = {
  ignoredRoots?: ReadonlySet<string>;
};

export function rustFiles(root: string, options: RustFileOptions = {}): string[] {
  if (options.ignoredRoots?.has(root)) {
    return [];
  }
  return readdirSync(root).flatMap((name) => {
    const path = resolve(root, name);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      return rustFiles(path, options);
    }
    return path.endsWith(".rs") ? [path] : [];
  }).sort((left, right) => left.localeCompare(right));
}

export function treeFiles(root: string): string[] {
  if (!existsSync(root)) {
    throw new Error(`path does not exist: ${root}`);
  }
  return readdirSync(root).flatMap((name) => {
    const path = resolve(root, name);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      return treeFiles(path);
    }
    return stat.isFile() ? [path] : [];
  }).sort((left, right) => left.localeCompare(right));
}

export function isProductionDiagnosticSourcePath(root: string, path: string): boolean {
  const sourcePath = relative(root, path).replaceAll("\\", "/");
  return (
    sourcePath.endsWith(".rs") &&
    !sourcePath.endsWith("_tests.rs") &&
    !sourcePath.endsWith("/tests.rs") &&
    !sourcePath.includes("/tests/")
  );
}

export function isInsideRoot(root: string, path: string): boolean {
  return path === root || path.startsWith(`${root}${sep}`);
}
