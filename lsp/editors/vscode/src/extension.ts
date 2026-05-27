import * as path from "path";
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions, type ServerOptions } from "vscode-languageclient/node";

const CLIENT_ID = "seagrass";
const CLIENT_NAME = "Seagrass";
const DIAGNOSTIC_SOURCE = "seagrass";
const STATUS_COMMAND = "seagrass/status";
const ANALYZE_COMMAND = "seagrass/analyze";
const ARTIFACTS_COMMAND = "seagrass/artifacts";
const ERROR_COVERAGE_COMMAND = "seagrass/errorCoverage";
const SUPPORT_MATRIX_COMMAND = "seagrass/supportMatrix";
const GENERATOR_PROFILE_COMMAND = "seagrass/generatorProfile";
const LOGS_COMMAND = "seagrass/logs";
const DEFAULT_SERVER_ARGS = ["run", "-p", "seagrass", "--quiet"];
const WATCHED_FILES = [
  "**/src/**/*.rs",
  "**/Anchor.toml",
  "**/Seagrass.toml",
  "**/Cargo.toml",
  "**/Cargo.lock",
  "**/target/deploy/*.so",
  "**/target/deploy/*-keypair.json",
  "**/target/idl/*.json",
  "**/target/types/*.ts",
];
const LAUNCH_CONFIGURATION_KEYS = [
  "seagrass.serverCommand",
  "seagrass.serverArgs",
  "seagrass.serverCwd",
  "seagrass.serverEnv",
  "seagrass.diagnostics.transport",
];
const SERVER_CAPABILITIES = [
  "full document sync",
  "push diagnostics with document versions",
  "pull diagnostics when enabled",
  "completion + completion resolve",
  "hover",
  "signature help",
  "code actions + code action resolve",
  "semantic tokens",
  "inlay hints + inlay hint resolve",
  "document symbols",
  "document links",
  "workspace symbols",
  "definition",
  "references",
  "rename + prepare rename",
  "document highlights",
  "selection ranges",
  "folding ranges",
  "workspace folders",
  "watched Anchor, Seagrass, Pinocchio, native Solana, Cargo, and build artifact files",
  STATUS_COMMAND,
  ANALYZE_COMMAND,
  ARTIFACTS_COMMAND,
  ERROR_COVERAGE_COMMAND,
  SUPPORT_MATRIX_COMMAND,
  GENERATOR_PROFILE_COMMAND,
  LOGS_COMMAND,
];

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let statusBarItem: vscode.StatusBarItem | undefined;
let restartQueue: Promise<void> = Promise.resolve();

type ServerLaunchConfig = {
  command: string;
  args: string[];
  cwd: string;
  env: NodeJS.ProcessEnv;
  diagnosticsTransport: DiagnosticsTransport;
};

type DiagnosticsTransport = "push" | "pull" | "both";

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel(CLIENT_NAME);
  statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 95);
  statusBarItem.name = CLIENT_NAME;
  statusBarItem.command = "seagrass.status";
  updateStatusBar("starting");
  statusBarItem.show();

  const fileWatchers = WATCHED_FILES.map((glob) => vscode.workspace.createFileSystemWatcher(glob));

  context.subscriptions.push(
    outputChannel,
    statusBarItem,
    ...fileWatchers,
    vscode.commands.registerCommand("seagrass.status", showStatus),
    vscode.commands.registerCommand("seagrass.analyze", showAnalysis),
    vscode.commands.registerCommand("seagrass.artifacts", showArtifacts),
    vscode.commands.registerCommand("seagrass.errorCoverage", showErrorCoverage),
    vscode.commands.registerCommand("seagrass.supportMatrix", showSupportMatrix),
    vscode.commands.registerCommand("seagrass.generatorProfile", showGeneratorProfile),
    vscode.commands.registerCommand("seagrass.logs", showLogs),
    vscode.commands.registerCommand("seagrass.restart", () => queueRestart(context, fileWatchers, "manual restart")),
    vscode.commands.registerCommand("seagrass.showOutput", () => outputChannel?.show(true)),
    vscode.window.onDidChangeActiveTextEditor(() => updateStatusBar(client ? "ready" : "stopped")),
    vscode.languages.onDidChangeDiagnostics(() => updateStatusBar(client ? "ready" : "stopped")),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (LAUNCH_CONFIGURATION_KEYS.some((key) => event.affectsConfiguration(key))) {
        void queueRestart(context, fileWatchers, "server launch configuration changed");
      } else if (event.affectsConfiguration("seagrass")) {
        outputChannel?.appendLine("Seagrass settings changed; sent didChangeConfiguration to server.");
      }
    }),
  );

  await startClient(context, fileWatchers);
}

export async function deactivate(): Promise<void> {
  updateStatusBar("stopped");
  await stopClient();
}

async function queueRestart(
  context: vscode.ExtensionContext,
  fileWatchers: vscode.FileSystemWatcher[],
  reason: string,
): Promise<void> {
  restartQueue = restartQueue.then(() => restartClient(context, fileWatchers, reason));
  return restartQueue;
}

async function restartClient(
  context: vscode.ExtensionContext,
  fileWatchers: vscode.FileSystemWatcher[],
  reason: string,
): Promise<void> {
  outputChannel?.appendLine(`Restarting Seagrass: ${reason}`);
  updateStatusBar("starting");
  await stopClient();
  await startClient(context, fileWatchers);
}

async function startClient(
  context: vscode.ExtensionContext,
  fileWatchers: vscode.FileSystemWatcher[],
): Promise<void> {
  if (client) {
    return;
  }

  const launch = readServerLaunchConfig(context);
  const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
  logStartup(launch, workspaceFolders);

  client = new LanguageClient(CLIENT_ID, CLIENT_NAME, serverOptions(launch), clientOptions(fileWatchers, workspaceFolders));
  await client.start();
  updateStatusBar("ready");
}

async function stopClient(): Promise<void> {
  const runningClient = client;
  client = undefined;
  if (runningClient) {
    await runningClient.stop();
  }
  updateStatusBar("stopped");
}

type StatusState = "starting" | "ready" | "stopped";

function updateStatusBar(state: StatusState): void {
  if (!statusBarItem) {
    return;
  }

  if (state === "starting") {
    statusBarItem.text = "$(sync~spin) Seagrass";
    statusBarItem.tooltip = "Seagrass is starting";
    return;
  }

  if (state === "stopped") {
    statusBarItem.text = "$(circle-slash) Seagrass";
    statusBarItem.tooltip = "Seagrass is stopped";
    return;
  }

  const activeUri = vscode.window.activeTextEditor?.document.uri;
  const diagnostics = activeUri
    ? vscode.languages.getDiagnostics(activeUri).filter((diagnostic) => diagnostic.source === DIAGNOSTIC_SOURCE)
    : [];
  const errors = diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error).length;
  const warnings = diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Warning).length;

  if (errors > 0) {
    statusBarItem.text = `$(error) Seagrass ${errors}`;
  } else if (warnings > 0) {
    statusBarItem.text = `$(warning) Seagrass ${warnings}`;
  } else {
    statusBarItem.text = "$(check) Seagrass";
  }

  statusBarItem.tooltip =
    errors + warnings > 0
      ? `Seagrass: ${errors} errors, ${warnings} warnings in the active file`
      : "Seagrass: no Anchor diagnostics in the active file";
}

async function showStatus(): Promise<void> {
  if (!client) {
    outputChannel?.appendLine("Seagrass is not running.");
    outputChannel?.show(true);
    return;
  }

  const status = await client.sendRequest("workspace/executeCommand", {
    command: STATUS_COMMAND,
    arguments: [],
  });
  outputChannel?.appendLine(String(status));
  outputChannel?.show(true);
}

async function showAnalysis(): Promise<void> {
  if (!client) {
    outputChannel?.appendLine("Seagrass is not running.");
    outputChannel?.show(true);
    return;
  }

  const activeEditor = vscode.window.activeTextEditor;
  if (!activeEditor || activeEditor.document.languageId !== "rust") {
    void vscode.window.showInformationMessage("Open a Rust document before running Seagrass: Analyze Document.");
    return;
  }

  const analysis = await client.sendRequest("workspace/executeCommand", {
    command: ANALYZE_COMMAND,
    arguments: [{ uri: activeEditor.document.uri.toString() }],
  });
  outputChannel?.appendLine("Seagrass document analysis:");
  outputChannel?.appendLine(JSON.stringify(analysis, null, 2));
  outputChannel?.show(true);
}

async function showArtifacts(): Promise<void> {
  if (!client) {
    outputChannel?.appendLine("Seagrass is not running.");
    outputChannel?.show(true);
    return;
  }

  const activeEditor = vscode.window.activeTextEditor;
  const args = activeEditor?.document.languageId === "rust"
    ? [{ uri: activeEditor.document.uri.toString() }]
    : [];
  const artifacts = await client.sendRequest("workspace/executeCommand", {
    command: ARTIFACTS_COMMAND,
    arguments: args,
  });
  outputChannel?.appendLine("Seagrass artifacts:");
  outputChannel?.appendLine(JSON.stringify(artifacts, null, 2));
  outputChannel?.show(true);
}

async function showErrorCoverage(): Promise<void> {
  if (!client) {
    outputChannel?.appendLine("Seagrass is not running.");
    outputChannel?.show(true);
    return;
  }

  const coverage = await client.sendRequest("workspace/executeCommand", {
    command: ERROR_COVERAGE_COMMAND,
    arguments: [],
  });
  outputChannel?.appendLine("Anchor ErrorCode coverage:");
  outputChannel?.appendLine(JSON.stringify(coverage, null, 2));
  outputChannel?.show(true);
}

async function showSupportMatrix(): Promise<void> {
  if (!client) {
    outputChannel?.appendLine("Seagrass is not running.");
    outputChannel?.show(true);
    return;
  }

  const support = await client.sendRequest("workspace/executeCommand", {
    command: SUPPORT_MATRIX_COMMAND,
    arguments: [],
  });
  outputChannel?.appendLine("Anchor support matrix:");
  outputChannel?.appendLine(JSON.stringify(support, null, 2));
  outputChannel?.show(true);
}

async function showGeneratorProfile(): Promise<void> {
  if (!client) {
    outputChannel?.appendLine("Seagrass is not running.");
    outputChannel?.show(true);
    return;
  }

  const profile = await client.sendRequest("workspace/executeCommand", {
    command: GENERATOR_PROFILE_COMMAND,
    arguments: [],
  });
  outputChannel?.appendLine("Anchor support generator profile:");
  outputChannel?.appendLine(JSON.stringify(profile, null, 2));
  outputChannel?.show(true);
}

async function showLogs(): Promise<void> {
  if (!client) {
    outputChannel?.appendLine("Seagrass is not running.");
    outputChannel?.show(true);
    return;
  }

  const logs = await client.sendRequest("workspace/executeCommand", {
    command: LOGS_COMMAND,
    arguments: [],
  });
  outputChannel?.appendLine("Seagrass recent logs:");
  outputChannel?.appendLine(JSON.stringify(logs, null, 2));
  outputChannel?.show(true);
}

function serverOptions(launch: ServerLaunchConfig): ServerOptions {
  return {
    command: launch.command,
    args: launch.args,
    options: {
      cwd: launch.cwd,
      env: launch.env,
    },
  };
}

function clientOptions(
  fileWatchers: vscode.FileSystemWatcher[],
  workspaceFolders: readonly vscode.WorkspaceFolder[],
): LanguageClientOptions {
  const options: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "rust" },
      { scheme: "untitled", language: "rust" },
    ],
    outputChannel,
    initializationOptions: {
      seagrass: {
        diagnostics: {
          transport: readDiagnosticsTransport(),
        },
      },
    },
    synchronize: {
      configurationSection: "seagrass",
      fileEvents: fileWatchers,
    },
  };

  if (workspaceFolders[0]) {
    options.workspaceFolder = workspaceFolders[0];
  }

  return options;
}

function readServerLaunchConfig(context: vscode.ExtensionContext): ServerLaunchConfig {
  const config = vscode.workspace.getConfiguration("seagrass");
  const command = config.get<string>("serverCommand") || "cargo";
  const args = config.get<string[]>("serverArgs") ?? DEFAULT_SERVER_ARGS;
  const configuredCwd = config.get<string>("serverCwd")?.trim();
  const cwd = configuredCwd || path.resolve(context.extensionPath, "../../..");
  const serverEnv = config.get<Record<string, string>>("serverEnv") ?? {};

  return {
    command,
    args,
    cwd,
    env: {
      ...process.env,
      ...stringEnv(serverEnv),
    },
    diagnosticsTransport: readDiagnosticsTransport(),
  };
}

function readDiagnosticsTransport(): DiagnosticsTransport {
  const value = vscode.workspace
    .getConfiguration("seagrass")
    .get<DiagnosticsTransport>("diagnostics.transport", "push");
  return value === "pull" || value === "both" ? value : "push";
}

function stringEnv(env: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(env).filter((entry): entry is [string, string] => typeof entry[1] === "string"));
}

function logStartup(launch: ServerLaunchConfig, workspaceFolders: readonly vscode.WorkspaceFolder[]): void {
  outputChannel?.appendLine("Seagrass");
  outputChannel?.appendLine(`server: ${launch.command} ${launch.args.join(" ")}`);
  outputChannel?.appendLine(`cwd: ${launch.cwd}`);
  outputChannel?.appendLine("sync: full");
  outputChannel?.appendLine(`diagnostics: ${launch.diagnosticsTransport}`);
  outputChannel?.appendLine("rust tooling: standalone");
  outputChannel?.appendLine(
    workspaceFolders.length > 0
      ? `workspaces: ${workspaceFolders.map((folder) => folder.uri.fsPath).join(", ")}`
      : "workspaces: none; only opened Rust files will be analyzed",
  );
  outputChannel?.appendLine(`features: ${SERVER_CAPABILITIES.join(", ")}`);
}
