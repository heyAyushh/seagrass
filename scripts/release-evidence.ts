import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const MILLISECONDS_PER_SECOND = 1_000;
export const SECONDS_PER_HOUR = 60 * 60;
export const MILLISECONDS_PER_HOUR = SECONDS_PER_HOUR * MILLISECONDS_PER_SECOND;
export const MILLISECONDS_PER_DAY = 24 * MILLISECONDS_PER_HOUR;
export const REQUIRED_FUZZ_HOURS = 32;
export const REQUIRED_COMMUNITY_AUDIT_DAYS = 30;

const scriptDir = dirname(fileURLToPath(import.meta.url));
const GITHUB_POSITIVE_ID_PATTERN = "[1-9][0-9]*";
const IMMUTABLE_CHANGELOG_REF_PATTERN =
  "(?:[a-f0-9]{40}|v[0-9]+\\.[0-9]+\\.[0-9]+(?:-[A-Za-z0-9._-]+)?)";
const CARGO_REPOSITORY_FIELD_PATTERN = /^\s*repository\s*=\s*"([^"]+)"/m;

export const repoRoot = resolve(scriptDir, "..");

const rootManifestPath = resolve(repoRoot, "Cargo.toml");
const fuzzManifestPath = resolve(repoRoot, "fuzz/Cargo.toml");
const fuzzWorkflowPath = resolve(repoRoot, ".github/workflows/fuzz.yaml");
const defaultCorpusRoot = resolve(repoRoot, "fuzz/corpus");

export function gitHead(): string {
  return execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
}

export function expectedFuzzTargets(): string[] {
  const metadataOutput = commandOutput("cargo", [
    "metadata",
    "--no-deps",
    "--format-version",
    "1",
    "--manifest-path",
    fuzzManifestPath,
  ]);
  if (!metadataOutput) {
    return fuzzTargetsFromManifest(readFileSync(fuzzManifestPath, "utf8"));
  }
  const metadata = JSON.parse(metadataOutput) as unknown;
  if (!isRecord(metadata) || !Array.isArray(metadata.packages)) {
    throw new Error("cargo metadata for fuzz did not contain packages");
  }
  const fuzzPackage = metadata.packages.find(
    (pkg) => isRecord(pkg) && stringField(pkg, "name") === "seagrass-fuzz",
  );
  if (!isRecord(fuzzPackage) || !Array.isArray(fuzzPackage.targets)) {
    throw new Error("cargo metadata did not contain the seagrass-fuzz package targets");
  }
  return fuzzPackage.targets
    .filter(isRecord)
    .filter((target) => Array.isArray(target.kind) && target.kind.includes("bin"))
    .map((target) => stringField(target, "name"))
    .sort(compareStrings);
}

export function workflowMatrixFuzzTargets(): string[] {
  return workflowMatrixList(readFileSync(fuzzWorkflowPath, "utf8"), "target");
}

export function workflowMatrixFuzzShards(): number[] {
  return workflowMatrixList(readFileSync(fuzzWorkflowPath, "utf8"), "shard")
    .map((value) => nonNegativeInteger(value, "fuzz shard matrix"))
    .sort((left, right) => left - right);
}

export function workflowMatrixFuzzShardCount(): number {
  const shards = workflowMatrixFuzzShards();
  for (let index = 0; index < shards.length; index += 1) {
    if (shards[index] !== index) {
      throw new Error("fuzz shard matrix must be contiguous from 0");
    }
  }
  return shards.length;
}

export function assertSameStringSet(actual: string[], expected: string[], field: string): void {
  const failures = stringSetFailures(actual, expected, field);
  if (failures.length > 0) {
    throw new Error(failures.join("; "));
  }
}

export function stringSetFailures(actual: string[], expected: string[], field: string): string[] {
  const actualSorted = [...actual].sort(compareStrings);
  const expectedSorted = [...expected].sort(compareStrings);
  const missing = expectedSorted.filter((entry) => !actualSorted.includes(entry));
  const unknown = actualSorted.filter((entry) => !expectedSorted.includes(entry));
  const failures: string[] = [];
  if (missing.length > 0) {
    failures.push(`${field} missing entries: ${missing.join(", ")}`);
  }
  if (unknown.length > 0) {
    failures.push(`${field} contains unknown entries: ${unknown.join(", ")}`);
  }
  return failures;
}

export function expectedGithubRepoSlug(): string {
  const remoteUrl = originRemoteUrl();
  const slug = githubRepoSlug(remoteUrl);
  if (!slug) {
    throw new Error(`origin remote must be a GitHub URL, got ${remoteUrl}`);
  }
  return slug;
}

function originRemoteUrl(): string {
  const directRemoteUrl = commandOutput("git", ["remote", "get-url", "origin"]);
  if (directRemoteUrl) {
    return directRemoteUrl;
  }
  const configRemoteUrl = commandOutput("git", ["config", "--get", "remote.origin.url"]);
  if (configRemoteUrl) {
    return configRemoteUrl;
  }
  const verboseRemoteUrl = originUrlFromVerbose(commandOutput("git", ["remote", "-v"]));
  if (verboseRemoteUrl) {
    return verboseRemoteUrl;
  }
  if (process.env.GITHUB_REPOSITORY) {
    return `https://github.com/${process.env.GITHUB_REPOSITORY}.git`;
  }
  return cargoRepositoryUrl();
}

function cargoRepositoryUrl(): string {
  const manifest = readFileSync(rootManifestPath, "utf8");
  return manifest.match(CARGO_REPOSITORY_FIELD_PATTERN)?.[1] ?? "";
}

function commandOutput(command: string, args: string[]): string {
  try {
    return execFileSync(command, args, {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return "";
  }
}

function fuzzTargetsFromManifest(contents: string): string[] {
  const targets: string[] = [];
  let insideBin = false;
  for (const line of contents.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed === "[[bin]]") {
      insideBin = true;
      continue;
    }
    if (trimmed.startsWith("[") && trimmed !== "[[bin]]") {
      insideBin = false;
      continue;
    }
    if (!insideBin || !trimmed.startsWith("name =")) {
      continue;
    }
    const match = trimmed.match(/^name\s*=\s*"([^"]+)"$/);
    if (match) {
      targets.push(match[1]);
    }
  }
  return targets.sort(compareStrings);
}

function originUrlFromVerbose(output: string): string {
  for (const line of output.split(/\r?\n/)) {
    const fields = line.trim().split(/\s+/);
    if (fields[0] === "origin" && fields[2] === "(fetch)") {
      return fields[1] ?? "";
    }
  }
  return "";
}

export function isExpectedGithubRepoUrl(value: string): boolean {
  const parsed = githubUrl(value);
  if (!parsed) {
    return false;
  }
  return parsed.pathname.toLowerCase().startsWith(`/${expectedGithubRepoSlug()}/`);
}

export function isExpectedActionsRunUrl(value: string): boolean {
  return githubPathMatches(value, `actions/runs/${GITHUB_POSITIVE_ID_PATTERN}`);
}

export function isExpectedReviewProofUrl(value: string): boolean {
  return (
    githubPathMatches(value, `pull/${GITHUB_POSITIVE_ID_PATTERN}`) ||
    githubPathMatches(value, `issues/${GITHUB_POSITIVE_ID_PATTERN}`) ||
    githubPathMatches(value, `discussions/${GITHUB_POSITIVE_ID_PATTERN}`)
  );
}

export function isExpectedChangelogProofUrl(value: string): boolean {
  return githubPathMatches(
    value,
    `blob/${IMMUTABLE_CHANGELOG_REF_PATTERN}/CHANGELOG\\.md`,
  );
}

export function corpusTreeSha256(root: string = defaultCorpusRoot): string {
  const files = treeFiles(root);
  if (files.length === 0) {
    throw new Error(`fuzz corpus has no files: ${root}`);
  }
  const treeHash = createHash("sha256");
  for (const file of files) {
    const relativePath = relative(root, file).replaceAll("\\", "/");
    const bytes = readFileSync(file);
    treeHash.update(relativePath);
    treeHash.update("\0");
    treeHash.update(createHash("sha256").update(bytes).digest("hex"));
    treeHash.update("\0");
    treeHash.update(String(bytes.length));
    treeHash.update("\0");
  }
  return treeHash.digest("hex");
}

export function finiteIsoTimestamp(value: string, field: string): string {
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) {
    throw new Error(`${field} must be an ISO timestamp`);
  }
  return new Date(parsed).toISOString();
}

export function isFiniteDate(value: string): boolean {
  return Number.isFinite(Date.parse(value));
}

export function elapsedHours(start: string, end: string): number {
  const startedAt = Date.parse(start);
  const completedAt = Date.parse(end);
  if (!Number.isFinite(startedAt) || !Number.isFinite(completedAt)) {
    return 0;
  }
  return (completedAt - startedAt) / MILLISECONDS_PER_HOUR;
}

export function elapsedDays(start: string, end: string): number {
  const startedAt = Date.parse(start);
  const completedAt = Date.parse(end);
  if (!Number.isFinite(startedAt) || !Number.isFinite(completedAt)) {
    return 0;
  }
  return Math.floor((completedAt - startedAt) / MILLISECONDS_PER_DAY);
}

export function fuzzHours(shards: number, secondsPerShard: number): number {
  return roundHours((shards * secondsPerShard) / SECONDS_PER_HOUR);
}

export function roundHours(value: number): number {
  return Math.round(value * 100) / 100;
}

export function compareStrings(left: string, right: string): number {
  return left.localeCompare(right);
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function stringField(record: Record<string, unknown>, field: string): string {
  const value = record[field];
  if (typeof value !== "string") {
    throw new Error(`${field} must be a string`);
  }
  return value;
}

function workflowMatrixList(contents: string, key: string): string[] {
  const lines = contents.split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() !== `${key}:`) {
      continue;
    }
    const keyIndent = leadingSpaces(line);
    const values: string[] = [];
    for (let scan = index + 1; scan < lines.length; scan += 1) {
      const candidate = lines[scan];
      if (candidate.trim() === "") {
        continue;
      }
      const candidateIndent = leadingSpaces(candidate);
      if (candidateIndent <= keyIndent) {
        break;
      }
      const trimmed = candidate.trim();
      if (trimmed.startsWith("- ")) {
        values.push(trimmed.slice(2).trim());
      }
    }
    if (values.length > 0) {
      return values.sort(compareStrings);
    }
  }
  throw new Error(`could not find ${key} matrix in ${fuzzWorkflowPath}`);
}

function treeFiles(root: string): string[] {
  if (!existsSync(root)) {
    throw new Error(`path does not exist: ${root}`);
  }
  return readdirSync(root)
    .map((entry) => join(root, entry))
    .flatMap((path) => {
      const stat = statSync(path);
      if (stat.isDirectory()) {
        return treeFiles(path);
      }
      return stat.isFile() ? [path] : [];
    })
    .sort(compareStrings);
}

function nonNegativeInteger(value: string, field: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`${field} must contain non-negative integers`);
  }
  return parsed;
}

function githubRepoSlug(remoteUrl: string): string | undefined {
  const trimmed = remoteUrl.trim().replace(/\.git$/, "");
  const https = trimmed.match(/^https:\/\/github\.com\/([^/]+\/[^/]+)$/);
  if (https) {
    return https[1].toLowerCase();
  }
  const ssh = trimmed.match(/^git@github\.com:([^/]+\/[^/]+)$/);
  if (ssh) {
    return ssh[1].toLowerCase();
  }
  const sshUrl = trimmed.match(/^ssh:\/\/git@github\.com\/([^/]+\/[^/]+)$/);
  if (sshUrl) {
    return sshUrl[1].toLowerCase();
  }
  return undefined;
}

function githubUrl(value: string): URL | undefined {
  try {
    const parsed = new URL(value);
    if (parsed.protocol === "https:" && parsed.hostname.toLowerCase() === "github.com") {
      return parsed;
    }
  } catch {
    // Fall through to the shared undefined return.
  }
  return undefined;
}

function githubPathMatches(value: string, repoRelativePattern: string): boolean {
  const parsed = githubUrl(value);
  if (!parsed) {
    return false;
  }
  const slug = expectedGithubRepoSlug();
  const path = new RegExp(`^/${escapeRegExp(slug)}/${repoRelativePattern}/?$`, "i");
  return path.test(parsed.pathname);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function leadingSpaces(value: string): number {
  const match = value.match(/^ */);
  return match ? match[0].length : 0;
}
