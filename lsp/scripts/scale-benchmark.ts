#!/usr/bin/env bun

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawn, spawnSync } from "node:child_process";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");
const serverBinary = resolve(repoRoot, "target/debug/seagrass");
const defaultWorkspaceRunId = `${Date.now()}-${process.pid}`;
const defaultWorkspace = resolve(
  repoRoot,
  "target",
  `seagrass-synth-workspace-${defaultWorkspaceRunId}`,
);
const defaultReport = resolve(repoRoot, "target/seagrass-scale-report.json");
const defaultPrograms = 200;
const defaultSamples = 1;
const defaultMaxColdStartMillis = 5_000;
const defaultMaxRssMegabytes = 500;
const statusPollMillis = 100;
const statusTimeoutMillis = 15_000;
const bytesPerKilobyte = 1_024;
const bytesPerMegabyte = 1_024 * 1_024;
const p99Percentile = 99;

type Args = {
  programs: number;
  workspace: string;
  report: string;
  samples: number;
  maxColdStartMillis: number;
  maxRssMegabytes: number;
};

type JsonRpcMessage = {
  id?: number | string;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: unknown;
};

type PendingRequest = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
};

type ScaleReport = {
  programs: number;
  indexedFiles: number;
  coldStartMillis: number;
  rssMegabytes: number;
  p99ColdStartMillis: number;
  p99RssMegabytes: number;
  samples: ScaleSample[];
  workspace: string;
};

type ScaleSample = {
  indexedFiles: number;
  coldStartMillis: number;
  rssMegabytes: number;
};

const args = parseArgs(process.argv.slice(2));

runChecked("bun", [
  "lsp/scripts/synth-workspace.ts",
  "--programs",
  String(args.programs),
  "--output",
  args.workspace,
]);
runChecked("cargo", ["build", "-p", "seagrass"]);

const report = await runScaleBenchmarkSamples(args);
mkdirSync(dirname(args.report), { recursive: true });
writeFileSync(args.report, `${JSON.stringify(report, null, 2)}\n`);

if (report.coldStartMillis > args.maxColdStartMillis) {
  fail(
    `cold start exceeded budget: ${report.coldStartMillis.toFixed(1)} ms > ${args.maxColdStartMillis} ms`,
  );
}
if (report.rssMegabytes > args.maxRssMegabytes) {
  fail(
    `RSS exceeded budget: ${report.rssMegabytes.toFixed(1)} MB > ${args.maxRssMegabytes} MB`,
  );
}

console.log(
  `seagrass scale benchmark passed: ${report.indexedFiles} indexed files, p99 ${report.coldStartMillis.toFixed(1)} ms cold start, p99 ${report.rssMegabytes.toFixed(1)} MB RSS`,
);
console.log(`report: ${args.report}`);

async function runScaleBenchmarkSamples(options: Args): Promise<ScaleReport> {
  const samples = await collectSamples(options, options.samples, []);
  const indexedFiles = Math.max(...samples.map((sample) => sample.indexedFiles));
  const p99ColdStartMillis = percentile(
    samples.map((sample) => sample.coldStartMillis),
    p99Percentile,
  );
  const p99RssMegabytes = percentile(
    samples.map((sample) => sample.rssMegabytes),
    p99Percentile,
  );
  return {
    programs: options.programs,
    indexedFiles,
    coldStartMillis: p99ColdStartMillis,
    rssMegabytes: p99RssMegabytes,
    p99ColdStartMillis,
    p99RssMegabytes,
    samples,
    workspace: options.workspace,
  };
}

async function collectSamples(
  options: Args,
  remainingSamples: number,
  samples: ScaleSample[],
): Promise<ScaleSample[]> {
  if (remainingSamples === 0) {
    return samples;
  }
  const sample = await runScaleBenchmark(options);
  return collectSamples(options, remainingSamples - 1, [...samples, sample]);
}

async function runScaleBenchmark(options: Args): Promise<ScaleSample> {
  const server = spawn(serverBinary, [], {
    cwd: repoRoot,
    stdio: ["pipe", "pipe", "inherit"],
  });
  let buffer = Buffer.alloc(0);
  let nextId = 1;
  const pending = new Map<number | string, PendingRequest>();
  const start = performance.now();

  server.stdout.on("data", (chunk: Buffer) => {
    buffer = Buffer.concat([buffer, chunk]);
    while (true) {
      const headerEnd = buffer.indexOf("\r\n\r\n");
      if (headerEnd === -1) {
        return;
      }
      const header = buffer.slice(0, headerEnd).toString("utf8");
      const length = contentLength(header);
      const messageStart = headerEnd + "\r\n\r\n".length;
      const messageEnd = messageStart + length;
      if (buffer.length < messageEnd) {
        return;
      }
      const payload = buffer.slice(messageStart, messageEnd).toString("utf8");
      buffer = buffer.slice(messageEnd);
      handleMessage(JSON.parse(payload) as JsonRpcMessage);
    }
  });

  function handleMessage(message: JsonRpcMessage): void {
    if (message.id === undefined) {
      return;
    }
    const request = pending.get(message.id);
    if (!request) {
      return;
    }
    pending.delete(message.id);
    if (message.error) {
      request.reject(new Error(JSON.stringify(message.error)));
    } else {
      request.resolve(message.result);
    }
  }

  function send(message: JsonRpcMessage): void {
    const payload = JSON.stringify({ jsonrpc: "2.0", ...message });
    server.stdin.write(`Content-Length: ${Buffer.byteLength(payload, "utf8")}\r\n\r\n${payload}`);
  }

  function request(method: string, params?: unknown): Promise<unknown> {
    const id = nextId;
    nextId += 1;
    send({ id, method, params });
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
    });
  }

  function notify(method: string, params?: unknown): void {
    send({ method, params });
  }

  try {
    await request("initialize", {
      processId: process.pid,
      rootUri: pathToFileURL(options.workspace).href,
      workspaceFolders: [
        {
          uri: pathToFileURL(options.workspace).href,
          name: "seagrass-synth-workspace",
        },
      ],
      capabilities: {
        workspace: {
          workspaceFolders: true,
        },
      },
    });
    notify("initialized", {});

    const status = await waitForIndexedStatus(request, options.programs);
    const coldStartMillis = performance.now() - start;
    const rssMegabytes = residentSetMegabytes(server.pid);
    await request("shutdown");
    notify("exit");

    return {
      indexedFiles: status.indexedFiles,
      coldStartMillis,
      rssMegabytes,
    };
  } finally {
    if (!server.killed) {
      server.kill();
    }
  }
}

async function waitForIndexedStatus(
  request: (method: string, params: unknown) => Promise<unknown>,
  expectedPrograms: number,
): Promise<{ indexedFiles: number }> {
  const deadline = performance.now() + statusTimeoutMillis;
  while (performance.now() < deadline) {
    const status = await request("workspace/executeCommand", {
      command: "seagrass/status",
      arguments: [],
    });
    if (typeof status === "string") {
      const indexedFiles = indexedFileCount(status);
      if (indexedFiles >= expectedPrograms) {
        return { indexedFiles };
      }
    }
    await new Promise((resolve) => setTimeout(resolve, statusPollMillis));
  }
  throw new Error(`workspace index did not reach ${expectedPrograms} files before timeout`);
}

function indexedFileCount(status: string): number {
  const match = /indexed files:\s*(\d+)/.exec(status);
  if (!match) {
    return 0;
  }
  return Number(match[1]);
}

function contentLength(header: string): number {
  const match = /Content-Length:\s*(\d+)/i.exec(header);
  if (!match) {
    throw new Error(`missing Content-Length header: ${header}`);
  }
  return Number(match[1]);
}

function residentSetMegabytes(pid: number | undefined): number {
  if (pid === undefined) {
    throw new Error("server pid is unavailable");
  }
  const result = spawnSync("ps", ["-o", "rss=", "-p", String(pid)], {
    encoding: "utf8",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`ps failed with exit code ${result.status}: ${result.stderr}`);
  }
  const rssKilobytes = Number(result.stdout.trim());
  if (!Number.isFinite(rssKilobytes)) {
    throw new Error(`could not parse RSS from ps output: ${result.stdout}`);
  }
  return (rssKilobytes * bytesPerKilobyte) / bytesPerMegabyte;
}

function percentile(values: number[], percentileValue: number): number {
  if (values.length === 0) {
    throw new Error("percentile requires at least one sample");
  }
  const sorted = [...values].sort((left, right) => left - right);
  const rank = Math.ceil((percentileValue / 100) * sorted.length) - 1;
  const boundedRank = Math.min(Math.max(rank, 0), sorted.length - 1);
  return sorted[boundedRank];
}

function parseArgs(rawArgs: string[]): Args {
  return parseArgList(rawArgs, {
    programs: defaultPrograms,
    workspace: defaultWorkspace,
    report: defaultReport,
    samples: defaultSamples,
    maxColdStartMillis: defaultMaxColdStartMillis,
    maxRssMegabytes: defaultMaxRssMegabytes,
  });
}

function parseArgList(rawArgs: string[], parsed: Args): Args {
  const [arg, value, ...remainingArgs] = rawArgs;
  if (!arg) {
    return parsed;
  }
  if (arg === "--programs") {
    return parseArgList(remainingArgs, { ...parsed, programs: numberArg(value, arg) });
  }
  if (arg === "--workspace") {
    return parseArgList(remainingArgs, { ...parsed, workspace: resolve(stringArg(value, arg)) });
  }
  if (arg === "--report") {
    return parseArgList(remainingArgs, { ...parsed, report: resolve(stringArg(value, arg)) });
  }
  if (arg === "--samples") {
    return parseArgList(remainingArgs, { ...parsed, samples: numberArg(value, arg) });
  }
  if (arg === "--max-cold-ms") {
    return parseArgList(remainingArgs, { ...parsed, maxColdStartMillis: numberArg(value, arg) });
  }
  if (arg === "--max-rss-mb") {
    return parseArgList(remainingArgs, { ...parsed, maxRssMegabytes: numberArg(value, arg) });
  }
  throw new Error(`unknown argument: ${arg}`);
}

function stringArg(value: string | undefined, name: string): string {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function numberArg(rawValue: string | undefined, name: string): number {
  const value = Number(stringArg(rawValue, name));
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${name} requires a positive number`);
  }
  return value;
}

function runChecked(command: string, commandArgs: string[]): void {
  const result = spawnSync(command, commandArgs, {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });
  if (result.error) {
    fail(`${command} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`${command} ${commandArgs.join(" ")} failed with exit code ${result.status}`);
  }
}

function fail(message: string): never {
  console.error(`\nseagrass scale benchmark failed:\n${message}`);
  process.exit(1);
}
