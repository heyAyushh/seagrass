import { describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, resolve } from "node:path";
import { gzipSync } from "node:zlib";

import {
  importFuzzArtifacts,
  validateFuzzArtifactEntries,
} from "./import-fuzz-artifacts.ts";
import {
  corpusTreeSha256,
  expectedFuzzTargets,
  REQUIRED_FUZZ_HOURS,
  workflowMatrixFuzzTargets,
} from "./release-evidence.ts";

const releaseCommit = "abc123def456";
const completedAt = "2026-05-26T12:00:00.000Z";
const startedAt = "2026-05-26T11:00:00.000Z";
const fuzzShardCount = 8;
const secondsPerTargetShard = 3_600;
const tarBlockSize = 512;
const tarChecksumOffset = 148;
const tarChecksumLength = 8;
const tarTypeOffset = 156;
const importWorkflowArtifactsTimeoutMillis = 20_000;

process.env.GITHUB_REPOSITORY = "heyAyushh/seagrass";

describe("import fuzz workflow artifacts", () => {
  test("imports downloaded corpus archives and readiness evidence", () => {
    const dir = mkdtempSync(resolve(tmpdir(), "seagrass-fuzz-import-"));
    const artifactRoot = resolve(dir, "downloaded-artifacts");
    const corpusOut = resolve(dir, "corpus");
    const readinessOut = resolve(dir, "fuzz-readiness.json");
    mkdirSync(artifactRoot, { recursive: true });
    writeReadinessArtifact(artifactRoot);
    for (const target of expectedFuzzTargets()) {
      for (let shard = 0; shard < fuzzShardCount; shard += 1) {
        writeCorpusArchive({ artifactRoot, target, shard });
      }
    }

    const result = importFuzzArtifacts({
      artifactRoot,
      expectedCommit: releaseCommit,
      corpusOut,
      readinessOut,
    });

    expect(result.readinessPath).toBe(readinessOut);
    expect(result.archiveCount).toBe(expectedFuzzTargets().length * fuzzShardCount);
    expect(result.copiedCorpusFiles).toBe(expectedFuzzTargets().length);
    expect(result.corpusSha256).toBe(corpusTreeSha256(corpusOut));
    for (const target of expectedFuzzTargets()) {
      const seedPath = resolve(corpusOut, target, `${target}-seed.txt`);
      expect(readFileSync(seedPath, "utf8")).toBe(`${target}\n`);
    }
    const readiness = JSON.parse(readFileSync(readinessOut, "utf8")) as Record<string, unknown>;
    expect(readiness.fuzzCleanRun).toBeTruthy();
    expect((readiness.fuzzCleanRun as Record<string, unknown>).corpusSha256).toBe(
      corpusTreeSha256(corpusOut),
    );
  }, importWorkflowArtifactsTimeoutMillis);

  test("refuses incomplete artifact sets without writing outputs", () => {
    const dir = mkdtempSync(resolve(tmpdir(), "seagrass-fuzz-import-missing-"));
    const artifactRoot = resolve(dir, "downloaded-artifacts");
    const corpusOut = resolve(dir, "corpus");
    const readinessOut = resolve(dir, "fuzz-readiness.json");
    mkdirSync(artifactRoot, { recursive: true });
    writeReadinessArtifact(artifactRoot);
    writeCorpusArchive({ artifactRoot, target: expectedFuzzTargets()[0], shard: 0 });

    expect(() =>
      importFuzzArtifacts({
        artifactRoot,
        expectedCommit: releaseCommit,
        corpusOut,
        readinessOut,
      }),
    ).toThrow(/missing fuzz corpus archive/);
    expect(existsSync(corpusOut)).toBe(false);
    expect(existsSync(readinessOut)).toBe(false);
  });

  test("rejects unsafe tar members before extraction", () => {
    expect(() =>
      validateFuzzArtifactEntries(
        ["fuzz/corpus/fuzz_document_parse/basic.rs", "../escape"],
        "bad.tar.gz",
      ),
    ).toThrow(/unsafe tar entry/);
  });
});

function writeReadinessArtifact(artifactRoot: string): void {
  const readinessDir = resolve(artifactRoot, `fuzz-readiness-${releaseCommit}`);
  mkdirSync(readinessDir, { recursive: true });
  writeFileSync(
    resolve(readinessDir, `fuzz-readiness-${releaseCommit}.json`),
    `${JSON.stringify({ fuzzCleanRun: fuzzCleanRun() }, null, 2)}\n`,
  );
}

function fuzzCleanRun(): unknown {
  const targets = expectedFuzzTargets();
  return {
    status: "passed",
    workflowRunUrl: "https://github.com/heyAyushh/seagrass/actions/runs/1",
    commit: releaseCommit,
    startedAt,
    completedAt,
    aggregateFuzzHours: REQUIRED_FUZZ_HOURS,
    targets,
    workflowMatrixTargets: workflowMatrixFuzzTargets(),
    targetRuns: targets.map((target) => ({
      target,
      status: "passed",
      shards: fuzzShardCount,
      secondsPerShard: secondsPerTargetShard,
      fuzzHours: REQUIRED_FUZZ_HOURS / targets.length,
    })),
  };
}

function writeCorpusArchive(input: {
  artifactRoot: string;
  target: string;
  shard: number;
}): void {
  const archiveDir = resolve(
    input.artifactRoot,
    `fuzz-corpus-${releaseCommit}-${input.target}-${input.shard}`,
  );
  const staging = resolve(archiveDir, "staging");
  const corpusDir = resolve(staging, "fuzz", "corpus", input.target);
  mkdirSync(corpusDir, { recursive: true });
  writeFileSync(resolve(corpusDir, `${input.target}-seed.txt`), `${input.target}\n`);
  mkdirSync(resolve(staging, "fuzz", "artifacts", input.target), { recursive: true });
  const archivePath = resolve(archiveDir, `${basename(archiveDir)}.tar.gz`);
  writeFileSync(
    archivePath,
    gzipSync(
      tarArchive([
        directoryEntry("fuzz/"),
        directoryEntry("fuzz/artifacts/"),
        directoryEntry(`fuzz/artifacts/${input.target}/`),
        directoryEntry("fuzz/corpus/"),
        directoryEntry(`fuzz/corpus/${input.target}/`),
        fileEntry(`fuzz/corpus/${input.target}/${input.target}-seed.txt`, `${input.target}\n`),
      ]),
    ),
  );
}

function tarArchive(entries: Buffer[]): Buffer {
  return Buffer.concat([...entries, Buffer.alloc(tarBlockSize), Buffer.alloc(tarBlockSize)]);
}

function directoryEntry(path: string): Buffer {
  return header({ path, size: 0, type: "5" });
}

function fileEntry(path: string, content: string): Buffer {
  const body = Buffer.from(content);
  const paddingLength = (tarBlockSize - (body.length % tarBlockSize)) % tarBlockSize;
  return Buffer.concat([
    header({ path, size: body.length, type: "0" }),
    body,
    Buffer.alloc(paddingLength),
  ]);
}

function header(input: { path: string; size: number; type: "0" | "5" }): Buffer {
  const block = Buffer.alloc(tarBlockSize);
  writeAscii(block, input.path, 0, 100);
  writeAscii(block, "0000777\0", 100, 8);
  writeAscii(block, "0000000\0", 108, 8);
  writeAscii(block, "0000000\0", 116, 8);
  writeAscii(block, octal(input.size, 11), 124, 12);
  writeAscii(block, "00000000000\0", 136, 12);
  block.fill(0x20, tarChecksumOffset, tarChecksumOffset + tarChecksumLength);
  writeAscii(block, input.type, tarTypeOffset, 1);
  writeAscii(block, "ustar\0", 257, 6);
  writeAscii(block, "00", 263, 2);
  writeAscii(block, checksum(block), tarChecksumOffset, tarChecksumLength);
  return block;
}

function writeAscii(block: Buffer, value: string, offset: number, length: number): void {
  block.write(value.slice(0, length), offset, length, "ascii");
}

function octal(value: number, width: number): string {
  return value.toString(8).padStart(width, "0") + "\0";
}

function checksum(block: Buffer): string {
  let sum = 0;
  for (const byte of block) {
    sum += byte;
  }
  return sum.toString(8).padStart(6, "0") + "\0 ";
}
