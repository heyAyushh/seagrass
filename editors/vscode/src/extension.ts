import { execFile } from "node:child_process";
import { promisify } from "node:util";
import * as path from "path";
import * as vscode from "vscode";
import { confidenceTier, registerConfidencePresentation } from "./confidencePresentation";
import {
  formatFalsePositiveReport,
  lintDocUrlFromTopic,
  parseDiagnosticMetadata,
  suppressionSnippet,
} from "./diagnosticActions";
import { registerTridentCoverage, refreshTridentCoverageForEditor } from "./tridentCoverage";
import { type InitializeParams } from "vscode-languageserver-protocol";
import { LanguageClient, type LanguageClientOptions, type ServerOptions } from "vscode-languageclient/node";

const CLIENT_ID = "seagrass";
const CLIENT_NAME = "Seagrass";
const DIAGNOSTIC_SOURCE = "seagrass";
const STATUS_COMMAND = "seagrass/status";
const ANALYZE_COMMAND = "seagrass/analyze";
const ARTIFACTS_COMMAND = "seagrass/artifacts";
const FEEDBACK_COMMAND = "seagrass/feedback";
const ERROR_COVERAGE_COMMAND = "seagrass/errorCoverage";
const SUPPORT_MATRIX_COMMAND = "seagrass/supportMatrix";
const GENERATOR_PROFILE_COMMAND = "seagrass/generatorProfile";
const LOGS_COMMAND = "seagrass/logs";
const SNIPPET_TEXT_EDIT_CAPABILITY = "snippetTextEdit";
const DEFAULT_CARGO_SERVER_ARGS = ["run", "-p", "seagrass-cli", "--quiet"];
const BYTES_PER_MEBIBYTE = 1024 * 1024;
const WORKSPACE_SCAN_MAX_BUFFER_BYTES = 10 * BYTES_PER_MEBIBYTE;
const DIAGNOSTICS_FOUND_EXIT_CODE = 1;
const execFileAsync = promisify(execFile);
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
  "seagrass.dev.useCargoFromCheckout",
];
const SERVER_CAPABILITIES = [
  "incremental document sync",
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
  FEEDBACK_COMMAND,
  ERROR_COVERAGE_COMMAND,
  SUPPORT_MATRIX_COMMAND,
  GENERATOR_PROFILE_COMMAND,
  LOGS_COMMAND,
];

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let statusBarItem: vscode.StatusBarItem | undefined;
let restartQueue: Promise<void> = Promise.resolve();

type JsonObject = Record<string, unknown>;

type ServerLaunchConfig = {
  command: string;
  args: string[];
  cwd: string;
  env: NodeJS.ProcessEnv;
  diagnosticsTransport: DiagnosticsTransport;
  useCargo: boolean;
};

type DiagnosticsTransport = "push" | "pull" | "both";

type ExecFileError = Error & {
  code?: number | string;
  stdout?: string;
  stderr?: string;
};

class SeagrassLanguageClient extends LanguageClient {
  protected override fillInitializeParams(params: InitializeParams): void {
    super.fillInitializeParams(params);
    const experimental = asJsonObject(params.capabilities.experimental);
    params.capabilities.experimental = {
      ...experimental,
      [SNIPPET_TEXT_EDIT_CAPABILITY]: true,
    };
  }
}

function asJsonObject(value: unknown): JsonObject {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    return value as JsonObject;
  }
  return {};
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel(CLIENT_NAME);
  statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 95);
  statusBarItem.name = CLIENT_NAME;
  statusBarItem.command = "seagrass.status";
  updateStatusBar("starting");
  statusBarItem.show();

  const fileWatchers = WATCHED_FILES.map((glob) => vscode.workspace.createFileSystemWatcher(glob));

  registerConfidencePresentation(context);
  registerTridentCoverage(context);

  context.subscriptions.push(
    outputChannel,
    statusBarItem,
    ...fileWatchers,
    vscode.commands.registerCommand("seagrass.status", showStatus),
    vscode.commands.registerCommand("seagrass.analyze", showAnalysis),
    vscode.commands.registerCommand("seagrass.artifacts", showArtifacts),
    vscode.commands.registerCommand("seagrass.feedback", showFeedback),
    vscode.commands.registerCommand("seagrass.errorCoverage", showErrorCoverage),
    vscode.commands.registerCommand("seagrass.supportMatrix", showSupportMatrix),
    vscode.commands.registerCommand("seagrass.generatorProfile", showGeneratorProfile),
    vscode.commands.registerCommand("seagrass.logs", showLogs),
    vscode.commands.registerCommand("seagrass.restart", () => queueRestart(context, fileWatchers, "manual restart")),
    vscode.commands.registerCommand("seagrass.showOutput", () => outputChannel?.show(true)),
    vscode.commands.registerCommand("seagrass.explainDiagnostic", explainDiagnostic),
    vscode.commands.registerCommand("seagrass.suppressDiagnostic", copySuppression),
    vscode.commands.registerCommand("seagrass.openLintDoc", openLintDoc),
    vscode.commands.registerCommand("seagrass.copySuppression", copySuppression),
    vscode.commands.registerCommand("seagrass.reportFalsePositive", reportFalsePositive),
    vscode.commands.registerCommand("seagrass.scanWorkspace", scanWorkspace),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      updateStatusBar(client ? "ready" : "stopped");
      refreshTridentCoverageForEditor(editor);
    }),
    vscode.languages.onDidChangeDiagnostics(() => {
      updateStatusBar(client ? "ready" : "stopped");
      refreshTridentCoverageForEditor(vscode.window.activeTextEditor);
    }),
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

  client = new SeagrassLanguageClient(CLIENT_ID, CLIENT_NAME, serverOptions(launch), clientOptions(fileWatchers, workspaceFolders));
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

  const confidenceSummary = summarizeConfidence(diagnostics);
  statusBarItem.tooltip =
    errors + warnings > 0
      ? `Seagrass: ${errors} errors, ${warnings} warnings in the active file${confidenceSummary}`
      : "Seagrass: no Anchor diagnostics in the active file";
}

function summarizeConfidence(diagnostics: readonly vscode.Diagnostic[]): string {
  const counts = new Map<string, number>();
  for (const diagnostic of diagnostics) {
    const label = confidenceTier(diagnostic);
    if (!label) {
      continue;
    }
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  if (counts.size === 0) {
    return "";
  }
  const parts = [...counts.entries()].map(([label, count]) => `${count} ${label}`);
  return ` (${parts.join(", ")})`;
}

function activeSeagrassDiagnostic(): vscode.Diagnostic | undefined {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "rust") {
    return undefined;
  }
  const position = editor.selection.active;
  const diagnostics = vscode.languages
    .getDiagnostics(editor.document.uri)
    .filter((diagnostic) => diagnostic.source === DIAGNOSTIC_SOURCE);
  return diagnostics.find((diagnostic) => diagnostic.range.contains(position))
    ?? diagnostics.find((diagnostic) => diagnostic.range.start.line <= position.line && diagnostic.range.end.line >= position.line);
}

function diagnosticDocsTarget(diagnostic: vscode.Diagnostic): vscode.Uri | undefined {
  const code = diagnostic.code;
  if (typeof code === "object" && code !== null && "target" in code) {
    const target = (code as { target?: unknown }).target;
    if (target instanceof vscode.Uri) {
      return target;
    }
    if (typeof target === "string") {
      return vscode.Uri.parse(target);
    }
  }
  return undefined;
}

function diagnosticTopic(diagnostic: vscode.Diagnostic): string | undefined {
  const metadataTopic = diagnosticMetadata(diagnostic).topic;
  if (metadataTopic) {
    return metadataTopic;
  }

  const code = diagnostic.code;
  if (typeof code === "string" && code.startsWith("seagrass/")) {
    return code;
  }
  if (typeof code === "object" && code !== null && "value" in code) {
    const value = String((code as { value: string | number }).value);
    if (value.includes("/")) {
      return value;
    }
  }
  return undefined;
}

function diagnosticMetadata(diagnostic: vscode.Diagnostic) {
  return parseDiagnosticMetadata((diagnostic.relatedInformation ?? []).map((info) => info.message));
}

function diagnosticCodeValue(diagnostic: vscode.Diagnostic): string | undefined {
  const code = diagnostic.code;
  if (typeof code === "string") {
    return code;
  }
  if (typeof code === "object" && code !== null && "value" in code) {
    return String((code as { value: string | number }).value);
  }
  return undefined;
}

function diagnosticLintDocUri(diagnostic: vscode.Diagnostic): vscode.Uri | undefined {
  const target = diagnosticDocsTarget(diagnostic);
  if (target) {
    return target;
  }
  const topic = diagnosticTopic(diagnostic);
  return topic ? vscode.Uri.parse(lintDocUrlFromTopic(topic)) : undefined;
}

async function explainDiagnostic(): Promise<void> {
  const diagnostic = activeSeagrassDiagnostic();
  if (!diagnostic) {
    void vscode.window.showInformationMessage("Place the cursor on a Seagrass diagnostic first.");
    return;
  }

  const target = diagnosticLintDocUri(diagnostic);
  if (target) {
    await vscode.env.openExternal(target);
    return;
  }

  outputChannel?.appendLine(diagnostic.message);
  outputChannel?.show(true);
}

async function openLintDoc(): Promise<void> {
  const diagnostic = activeSeagrassDiagnostic();
  if (!diagnostic) {
    void vscode.window.showInformationMessage("Place the cursor on a Seagrass diagnostic first.");
    return;
  }

  const target = diagnosticLintDocUri(diagnostic);
  if (!target) {
    void vscode.window.showInformationMessage("This Seagrass diagnostic has no lint document yet.");
    return;
  }
  await vscode.env.openExternal(target);
}

async function copySuppression(): Promise<void> {
  const diagnostic = activeSeagrassDiagnostic();
  if (!diagnostic) {
    void vscode.window.showInformationMessage("Place the cursor on a Seagrass diagnostic first.");
    return;
  }

  const topic = diagnosticTopic(diagnostic);
  if (!topic) {
    void vscode.window.showInformationMessage("This Seagrass diagnostic has no suppression topic.");
    return;
  }

  const snippet = suppressionSnippet(topic);
  await vscode.env.clipboard.writeText(snippet);
  void vscode.window.showInformationMessage(`Copied suppression to clipboard: ${snippet}`);
}

async function reportFalsePositive(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  const diagnostic = activeSeagrassDiagnostic();
  if (!editor || !diagnostic) {
    void vscode.window.showInformationMessage("Place the cursor on a Seagrass diagnostic first.");
    return;
  }

  const docsUri = diagnosticLintDocUri(diagnostic);
  const report = formatFalsePositiveReport({
    file: vscode.workspace.asRelativePath(editor.document.uri, false),
    range: diagnosticRangeLabel(diagnostic.range),
    message: diagnostic.message,
    sourceLine: diagnosticSourceLine(editor.document, diagnostic.range.start.line),
    code: diagnosticCodeValue(diagnostic),
    docsUrl: docsUri?.toString(),
    metadata: diagnosticMetadata(diagnostic),
  });
  await vscode.env.clipboard.writeText(report);
  outputChannel?.appendLine("Copied Seagrass false-positive report:");
  outputChannel?.appendLine(report);
  outputChannel?.show(true);

  const feedback = await feedbackLink();
  if (!feedback) {
    void vscode.window.showInformationMessage("Copied false-positive report to clipboard.");
    return;
  }
  outputChannel?.appendLine(`Opening ${feedback.label}: ${feedback.url}`);
  await vscode.env.openExternal(vscode.Uri.parse(feedback.url));
}

function diagnosticRangeLabel(range: vscode.Range): string {
  return `${range.start.line + 1}:${range.start.character + 1}-${range.end.line + 1}:${range.end.character + 1}`;
}

function diagnosticSourceLine(document: vscode.TextDocument, line: number): string | undefined {
  if (line < 0 || line >= document.lineCount) {
    return undefined;
  }
  return document.lineAt(line).text;
}

async function scanWorkspace(): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) {
    void vscode.window.showInformationMessage("Open a workspace folder before scanning.");
    return;
  }

  const launch = readServerLaunchConfigFromWorkspace();
  const command = launch.useCargo ? "cargo" : launch.command;
  const args = launch.useCargo
    ? [...launch.args, "--", "diagnostics", folder.uri.fsPath, "--json"]
    : ["diagnostics", folder.uri.fsPath, "--json"];

  try {
    const { stdout, stderr } = await execFileAsync(command, args, {
      cwd: launch.cwd,
      env: launch.env,
      maxBuffer: WORKSPACE_SCAN_MAX_BUFFER_BYTES,
    });
    showWorkspaceScanResult(folder.name, stdout, stderr, false);
  } catch (error) {
    if (isExecFileError(error) && error.code === DIAGNOSTICS_FOUND_EXIT_CODE) {
      showWorkspaceScanResult(folder.name, error.stdout ?? "", error.stderr ?? "", true);
      return;
    }

    const message = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(`Seagrass workspace scan failed: ${message}`);
  }
}

function showWorkspaceScanResult(
  folderName: string,
  stdout: string,
  stderr: string,
  hasFindings: boolean,
): void {
  const suffix = hasFindings ? " found issues" : "";
  outputChannel?.appendLine(`Seagrass workspace scan (${folderName})${suffix}:`);
  if (stdout.trim().length > 0) {
    outputChannel?.appendLine(stdout);
  }
  if (stderr.trim().length > 0) {
    outputChannel?.appendLine(stderr);
  }
  outputChannel?.show(true);
}

function isExecFileError(error: unknown): error is ExecFileError {
  return error instanceof Error && "code" in error;
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

type FeedbackLink = {
  url: string;
  label: string;
};

async function showFeedback(): Promise<void> {
  const feedback = await feedbackLink();
  if (!feedback) {
    void vscode.window.showWarningMessage("Seagrass did not return a feedback URL.");
    return;
  }

  outputChannel?.appendLine(`Opening ${feedback.label}: ${feedback.url}`);
  await vscode.env.openExternal(vscode.Uri.parse(feedback.url));
}

async function feedbackLink(): Promise<FeedbackLink | undefined> {
  const configuredUrl = vscode.workspace.getConfiguration("seagrass.feedback").get<string>("url", "").trim();
  if (configuredUrl) {
    return { url: configuredUrl, label: "Seagrass feedback" };
  }

  if (!client) {
    outputChannel?.appendLine("Seagrass is not running.");
    outputChannel?.show(true);
    return undefined;
  }

  const response = await client.sendRequest<unknown>("workspace/executeCommand", {
    command: FEEDBACK_COMMAND,
    arguments: [],
  });
  return parseFeedbackResponse(response);
}

function parseFeedbackResponse(value: unknown): FeedbackLink | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }

  const record = value as Record<string, unknown>;
  const url = stringField(record, "url");
  const label = stringField(record, "label") ?? "Join the Seagrass Telegram";
  return url ? { url, label } : undefined;
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" && value.trim().length > 0 ? value : undefined;
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
        agent: {
          mode: readAgentMode(),
        },
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
  return readServerLaunchConfigFromWorkspace(context);
}

function readServerLaunchConfigFromWorkspace(
  context?: vscode.ExtensionContext,
): ServerLaunchConfig {
  const config = vscode.workspace.getConfiguration("seagrass");
  const useCargo = config.get<boolean>("dev.useCargoFromCheckout", false);
  const command = useCargo ? "cargo" : config.get<string>("serverCommand") || "seagrass";
  const configuredArgs = config.get<string[]>("serverArgs") ?? [];
  const args = useCargo && configuredArgs.length === 0 ? DEFAULT_CARGO_SERVER_ARGS : configuredArgs;
  const configuredCwd = config.get<string>("serverCwd")?.trim();
  const cwd =
    configuredCwd ||
    (context ? path.resolve(context.extensionPath, "../../..") : process.cwd());
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
    useCargo,
  };
}

function readDiagnosticsTransport(): DiagnosticsTransport {
  const value = vscode.workspace
    .getConfiguration("seagrass")
    .get<DiagnosticsTransport>("diagnostics.transport", "push");
  return value === "pull" || value === "both" ? value : "push";
}

function readAgentMode(): boolean {
  return vscode.workspace.getConfiguration("seagrass").get<boolean>("agent.mode", false);
}

function stringEnv(env: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(env).filter((entry): entry is [string, string] => typeof entry[1] === "string"));
}

function logStartup(launch: ServerLaunchConfig, workspaceFolders: readonly vscode.WorkspaceFolder[]): void {
  outputChannel?.appendLine("Seagrass");
  outputChannel?.appendLine(`server: ${launch.command} ${launch.args.join(" ")}`);
  outputChannel?.appendLine(`cwd: ${launch.cwd}`);
  outputChannel?.appendLine("sync: incremental");
  outputChannel?.appendLine(`diagnostics: ${launch.diagnosticsTransport}`);
  outputChannel?.appendLine("rust tooling: standalone");
  outputChannel?.appendLine(
    workspaceFolders.length > 0
      ? `workspaces: ${workspaceFolders.map((folder) => folder.uri.fsPath).join(", ")}`
      : "workspaces: none; only opened Rust files will be analyzed",
  );
  outputChannel?.appendLine(`features: ${SERVER_CAPABILITIES.join(", ")}`);
}
