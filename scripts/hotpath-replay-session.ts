#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

type JsonRecord = Record<string, unknown>;

type Position = {
  line: number;
  character: number;
};

type ReplayDocument = {
  name: string;
  relativePath: string;
  text: string;
};

type ReplayRequest = {
  method: "textDocument/diagnostic" | "textDocument/completion" | "textDocument/hover";
  document: string;
  positionAfter?: string;
  positionAt?: string;
  triggerCharacter?: string;
  expectLabel?: string;
  expectContains?: string;
};

type ReplaySession = {
  schemaVersion: 1;
  documents: ReplayDocument[];
  requests: ReplayRequest[];
};

type RuntimeDocument = ReplayDocument & {
  path: string;
  uri: string;
};

type LspMessage = {
  jsonrpc?: "2.0";
  id?: number | string | null;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: {
    code: number;
    message: string;
  };
};

type PendingRequest = {
  resolveResponse: (value: unknown) => void;
  reject: (reason: Error) => void;
  timeout: ReturnType<typeof setTimeout>;
};

const SERVER_EXIT_TIMEOUT_MILLIS = 5_000;
const REQUEST_TIMEOUT_MILLIS = 15_000;
const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");
const defaultSessionPath = resolve(repoRoot, "lsp/fixtures/hotpath-replay-session.json");
const replayRoot = resolve(repoRoot, "target/seagrass-hotpath-replay-session");
const sessionPath = process.argv[2] ? resolve(repoRoot, process.argv[2]) : defaultSessionPath;
const session = parseReplaySession(readFileSync(sessionPath, "utf8"));
const documents = materializeDocuments(session.documents);
const [command, args] = seagrassServerCommand();
const server = spawn(command, args, {
  cwd: repoRoot,
  stdio: ["pipe", "pipe", "inherit"],
  env: process.env,
});

let nextId = 1;
let buffer = Buffer.alloc(0);
const pending = new Map<number | string, PendingRequest>();
let serverExitCode: number | null = null;
let serverExitSignal: NodeJS.Signals | null = null;
let serverExited = false;
const serverExit = new Promise<void>((resolveExit) => {
  server.on("exit", (code, signal) => {
    serverExited = true;
    serverExitCode = code;
    serverExitSignal = signal;
    resolveExit();
  });
});

server.stdout.on("data", (chunk: Buffer) => {
  buffer = Buffer.concat([buffer, chunk]);
  receiveBufferedMessages();
});

try {
  await initialize();
  openDocuments(documents);
  await replayRequests(session.requests, documents);
  await request("shutdown");
  if (process.env.SEAGRASS_WAIT_FOR_EXIT === "1") {
    notify("exit", {});
    server.stdin.end();
    await waitForServerExit();
  } else {
    notify("exit", {});
  }
  console.log(`seagrass hotpath session replay passed: ${sessionPath}`);
} catch (error) {
  throw error;
} finally {
  if (!serverExited) {
    server.kill();
  }
}

function parseReplaySession(text: string): ReplaySession {
  const parsed = JSON.parse(text) as unknown;
  if (!isRecord(parsed) || parsed.schemaVersion !== 1) {
    throw new Error("hotpath replay session must use schemaVersion 1");
  }
  if (!Array.isArray(parsed.documents) || !parsed.documents.every(isReplayDocument)) {
    throw new Error("hotpath replay session documents are invalid");
  }
  if (!Array.isArray(parsed.requests) || !parsed.requests.every(isReplayRequest)) {
    throw new Error("hotpath replay session requests are invalid");
  }
  return {
    schemaVersion: 1,
    documents: parsed.documents,
    requests: parsed.requests,
  };
}

function isReplayDocument(value: unknown): value is ReplayDocument {
  return (
    isRecord(value) &&
    typeof value.name === "string" &&
    typeof value.relativePath === "string" &&
    typeof value.text === "string"
  );
}

function isReplayRequest(value: unknown): value is ReplayRequest {
  return (
    isRecord(value) &&
    isReplayMethod(value.method) &&
    typeof value.document === "string" &&
    optionalString(value.positionAfter) &&
    optionalString(value.positionAt) &&
    optionalString(value.triggerCharacter) &&
    optionalString(value.expectLabel) &&
    optionalString(value.expectContains)
  );
}

function isReplayMethod(value: unknown): value is ReplayRequest["method"] {
  return (
    value === "textDocument/diagnostic" ||
    value === "textDocument/completion" ||
    value === "textDocument/hover"
  );
}

function optionalString(value: unknown): boolean {
  return value === undefined || typeof value === "string";
}

function materializeDocuments(replayDocuments: ReplayDocument[]): RuntimeDocument[] {
  mkdirSync(replayRoot, { recursive: true });
  return replayDocuments.map((document) => {
    const path = resolve(replayRoot, document.relativePath);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, document.text);
    return {
      ...document,
      path,
      uri: pathToFileURL(path).href,
    };
  });
}

async function initialize(): Promise<void> {
  await request("initialize", {
    processId: process.pid,
    rootUri: pathToFileURL(replayRoot).href,
    workspaceFolders: [{ uri: pathToFileURL(replayRoot).href, name: "hotpath-replay" }],
    initializationOptions: {
      seagrass: {
        diagnostics: {
          transport: "both",
        },
        workspaceIndex: {
          enabled: true,
        },
      },
    },
    capabilities: {
      workspace: {
        workspaceFolders: true,
      },
      textDocument: {
        diagnostic: { dynamicRegistration: false },
        hover: { dynamicRegistration: false },
        completion: { dynamicRegistration: false },
      },
    },
  });
  notify("initialized", {});
}

function openDocuments(runtimeDocuments: RuntimeDocument[]): void {
  runtimeDocuments.forEach((document) => {
    notify("textDocument/didOpen", {
      textDocument: {
        uri: document.uri,
        languageId: "rust",
        version: 1,
        text: document.text,
      },
    });
  });
}

async function replayRequests(
  replayRequestsToRun: ReplayRequest[],
  runtimeDocuments: RuntimeDocument[],
): Promise<void> {
  for (const replayRequest of replayRequestsToRun) {
    const document = documentByName(runtimeDocuments, replayRequest.document);
    const response = await request(replayRequest.method, requestParams(replayRequest, document));
    assertExpectedResponse(replayRequest, response);
  }
}

function requestParams(replayRequest: ReplayRequest, document: RuntimeDocument): JsonRecord {
  if (replayRequest.method === "textDocument/diagnostic") {
    return {
      textDocument: { uri: document.uri },
      identifier: "seagrass",
      previousResultId: null,
    };
  }

  return {
    textDocument: { uri: document.uri },
    position: requestPosition(replayRequest, document.text),
    ...(replayRequest.triggerCharacter
      ? {
          context: {
            triggerKind: 2,
            triggerCharacter: replayRequest.triggerCharacter,
          },
        }
      : {}),
  };
}

function requestPosition(replayRequest: ReplayRequest, text: string): Position {
  if (replayRequest.positionAfter) {
    return positionAfter(text, replayRequest.positionAfter);
  }
  if (replayRequest.positionAt) {
    return positionAt(text, replayRequest.positionAt);
  }
  throw new Error(`${replayRequest.method} requires positionAfter or positionAt`);
}

function assertExpectedResponse(replayRequest: ReplayRequest, response: unknown): void {
  if (replayRequest.expectLabel && !completionLabels(response).includes(replayRequest.expectLabel)) {
    throw new Error(
      `${replayRequest.method} missing completion label ${replayRequest.expectLabel}: ${JSON.stringify(response)}`,
    );
  }
  if (replayRequest.expectContains && !JSON.stringify(response).includes(replayRequest.expectContains)) {
    throw new Error(
      `${replayRequest.method} missing expected text ${replayRequest.expectContains}: ${JSON.stringify(response)}`,
    );
  }
}

function completionLabels(response: unknown): string[] {
  const items = Array.isArray(response)
    ? response
    : isRecord(response) && Array.isArray(response.items)
      ? response.items
      : [];
  return items
    .filter(isRecord)
    .map((item) => item.label)
    .filter((label): label is string => typeof label === "string");
}

function documentByName(
  runtimeDocuments: RuntimeDocument[],
  name: string,
): RuntimeDocument {
  const document = runtimeDocuments.find((candidate) => candidate.name === name);
  if (!document) {
    throw new Error(`hotpath replay document not found: ${name}`);
  }
  return document;
}

function positionAfter(text: string, needle: string): Position {
  const index = text.indexOf(needle);
  if (index < 0) {
    throw new Error(`needle not found: ${needle}`);
  }
  return positionAtOffset(text, index + needle.length);
}

function positionAt(text: string, needle: string): Position {
  const index = text.indexOf(needle);
  if (index < 0) {
    throw new Error(`needle not found: ${needle}`);
  }
  return positionAtOffset(text, index);
}

function positionAtOffset(text: string, offset: number): Position {
  const prefix = text.slice(0, offset);
  const lines = prefix.split("\n");
  return {
    line: lines.length - 1,
    character: lines.at(-1)?.length ?? 0,
  };
}

function receiveBufferedMessages(): void {
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) {
      return;
    }

    const header = buffer.slice(0, headerEnd).toString("utf8");
    const lengthMatch = /Content-Length: (\d+)/i.exec(header);
    if (!lengthMatch) {
      throw new Error(`missing Content-Length header: ${header}`);
    }

    const bodyStart = headerEnd + 4;
    const bodyEnd = bodyStart + Number(lengthMatch[1]);
    if (buffer.length < bodyEnd) {
      return;
    }

    const message = JSON.parse(buffer.slice(bodyStart, bodyEnd).toString("utf8")) as LspMessage;
    buffer = buffer.slice(bodyEnd);
    receive(message);
  }
}

function send(message: LspMessage): void {
  const body = JSON.stringify(message);
  server.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
}

function request<T = unknown>(method: string, params?: unknown): Promise<T> {
  const id = nextId;
  nextId += 1;
  const message: LspMessage = { jsonrpc: "2.0", id, method };
  if (params !== undefined) {
    message.params = params;
  }
  send(message);
  return new Promise((resolveResponse, reject) => {
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`timed out waiting for ${method}`));
    }, REQUEST_TIMEOUT_MILLIS);
    pending.set(id, { resolveResponse, reject, timeout });
  });
}

function notify(method: string, params: unknown): void {
  send({ jsonrpc: "2.0", method, params });
}

function receive(message: LspMessage): void {
  if (message.id === undefined || message.id === null || !pending.has(message.id)) {
    return;
  }

  const entry = pending.get(message.id);
  if (!entry) {
    return;
  }
  pending.delete(message.id);
  clearTimeout(entry.timeout);
  if (message.error) {
    entry.reject(new Error(`${message.error.code}: ${message.error.message}`));
    return;
  }
  entry.resolveResponse(message.result);
}

function seagrassServerCommand(): [string, string[]] {
  if (process.env.SEAGRASS_SERVER_BINARY) {
    return [process.env.SEAGRASS_SERVER_BINARY, []];
  }
  if (process.env.SEAGRASS_HOTPATH === "1") {
    return [
      "cargo",
      ["run", "-p", "seagrass", "--features", "hotpath", "--release", "--quiet"],
    ];
  }
  return ["cargo", ["run", "-p", "seagrass", "--quiet"]];
}

async function waitForServerExit(): Promise<void> {
  await Promise.race([
    serverExit,
    new Promise((_, reject) =>
      setTimeout(
        () => reject(new Error("timed out waiting for seagrass to exit")),
        SERVER_EXIT_TIMEOUT_MILLIS,
      ),
    ),
  ]);
  if (serverExitCode !== 0) {
    throw new Error(
      `seagrass exited unexpectedly: code=${serverExitCode} signal=${serverExitSignal}`,
    );
  }
}

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
