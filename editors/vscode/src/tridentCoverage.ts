import { readFile } from "node:fs/promises";
import * as path from "path";
import * as vscode from "vscode";
import {
  coverageForDocumentPath,
  summarizeLineCoverage,
  tridentPromotionHint,
  type CoverageReport,
  type LineExecution,
} from "./tridentCoverageModel";

export type { CoverageFile, CoverageReport, CoverageSegment, LineExecution } from "./tridentCoverageModel";
export { coverageForDocumentPath, lineExecutionsForFile, summarizeLineCoverage, tridentPromotionHint } from "./tridentCoverageModel";

const DEFAULT_SEARCH_GLOBS = [
  "**/trident-tests/**/coverage.json",
  "**/trident-tests/**/coverage/**/*.json",
  "**/*-coverage-report/**/*.json",
  "**/llvm-cov/**/*.json",
];

let activeCoverageUri: vscode.Uri | undefined;
let decorationTypes: vscode.TextEditorDecorationType[] = [];
let fileWatcher: vscode.FileSystemWatcher | undefined;

export function registerTridentCoverage(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("seagrass.showTridentCoverage", () => showTridentCoverage(context)),
    vscode.commands.registerCommand("seagrass.closeTridentCoverage", closeTridentCoverage),
  );
}

export async function showTridentCoverage(context: vscode.ExtensionContext): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "rust") {
    void vscode.window.showInformationMessage("Open a Rust source file before showing Trident coverage.");
    return;
  }

  const config = vscode.workspace.getConfiguration("seagrass.tridentCoverage");
  const configuredPath = config.get<string>("reportPath")?.trim();
  const searchGlobs = config.get<string[]>("searchGlobs") ?? DEFAULT_SEARCH_GLOBS;
  const reportPath =
    configuredPath && configuredPath.length > 0
      ? await resolveWorkspacePath(configuredPath)
      : await pickCoverageReport(searchGlobs);

  if (!reportPath) {
    return;
  }

  const report = await loadCoverageReport(reportPath);
  const fileCoverage = coverageForEditorFile(report, editor.document.uri);
  if (!fileCoverage) {
    void vscode.window.showWarningMessage(
      `No Trident coverage entries matched ${editor.document.fileName}. Report: ${reportPath}`,
    );
    return;
  }

  applyCoverageDecorations(editor, fileCoverage, config.get<boolean>("showExecutionCount", true));
  activeCoverageUri = editor.document.uri;
  watchCoverageReport(context, reportPath, editor, config.get<boolean>("showExecutionCount", true));
  const summary = summarizeLineCoverage(fileCoverage);
  const hint = tridentPromotionHint(summary);
  const coverageText = `${summary.coveredLines}/${summary.totalLines} lines covered; ${summary.uncoveredLines} uncovered`;
  void vscode.window.showInformationMessage(
    `Seagrass Trident coverage loaded from ${path.basename(reportPath)}: ${coverageText}.${hint ? ` ${hint}` : ""}`,
  );
}

export function closeTridentCoverage(): void {
  clearCoverageDecorations();
  fileWatcher?.dispose();
  fileWatcher = undefined;
  activeCoverageUri = undefined;
  void vscode.window.showInformationMessage("Seagrass Trident coverage hidden.");
}

export function refreshTridentCoverageForEditor(editor: vscode.TextEditor | undefined): void {
  if (!editor || !activeCoverageUri || editor.document.uri.toString() !== activeCoverageUri.toString()) {
    return;
  }
  const config = vscode.workspace.getConfiguration("seagrass.tridentCoverage");
  const reportPath = config.get<string>("reportPath")?.trim();
  if (!reportPath) {
    return;
  }
  void loadCoverageReport(reportPath)
    .then((report) => {
      const fileCoverage = coverageForEditorFile(report, editor.document.uri);
      if (!fileCoverage) {
        return;
      }
      applyCoverageDecorations(editor, fileCoverage, config.get<boolean>("showExecutionCount", true));
    })
    .catch(() => undefined);
}

export async function loadCoverageReport(reportPath: string): Promise<CoverageReport> {
  const raw = await readFile(reportPath, "utf8");
  return JSON.parse(raw) as CoverageReport;
}

export function coverageForEditorFile(report: CoverageReport, documentUri: vscode.Uri): LineExecution[] | undefined {
  return coverageForDocumentPath(report, documentUri.fsPath);
}

function applyCoverageDecorations(
  editor: vscode.TextEditor,
  lineCoverage: LineExecution[],
  showExecutionCount: boolean,
): void {
  clearCoverageDecorations();
  const uncovered: vscode.Range[] = [];
  const low: vscode.Range[] = [];
  const medium: vscode.Range[] = [];
  const high: vscode.Range[] = [];
  const countDecorations: vscode.DecorationOptions[] = [];

  for (const entry of lineCoverage) {
    const range = editor.document.lineAt(Math.max(entry.line - 1, 0)).range;
    if (!entry.covered) {
      uncovered.push(range);
      continue;
    }
    if (entry.executionCount >= 10) {
      high.push(range);
    } else if (entry.executionCount >= 3) {
      medium.push(range);
    } else {
      low.push(range);
    }
    if (showExecutionCount) {
      countDecorations.push({
        range: new vscode.Range(range.start.line, 0, range.start.line, 0),
        renderOptions: {
          before: {
            contentText: `${entry.executionCount}x `,
            color: new vscode.ThemeColor("editorCodeLens.foreground"),
          },
        },
      });
    }
  }

  const uncoveredType = vscode.window.createTextEditorDecorationType({
    backgroundColor: "rgba(255, 80, 80, 0.18)",
    isWholeLine: true,
    overviewRulerColor: "rgba(255, 80, 80, 0.8)",
    overviewRulerLane: vscode.OverviewRulerLane.Right,
  });
  const lowType = vscode.window.createTextEditorDecorationType({
    backgroundColor: "rgba(80, 180, 120, 0.10)",
    isWholeLine: true,
  });
  const mediumType = vscode.window.createTextEditorDecorationType({
    backgroundColor: "rgba(80, 180, 120, 0.18)",
    isWholeLine: true,
  });
  const highType = vscode.window.createTextEditorDecorationType({
    backgroundColor: "rgba(80, 180, 120, 0.30)",
    isWholeLine: true,
  });
  const countType = vscode.window.createTextEditorDecorationType({});

  editor.setDecorations(uncoveredType, uncovered);
  editor.setDecorations(lowType, low);
  editor.setDecorations(mediumType, medium);
  editor.setDecorations(highType, high);
  editor.setDecorations(countType, countDecorations);
  decorationTypes = [uncoveredType, lowType, mediumType, highType, countType];
}

function clearCoverageDecorations(): void {
  for (const decorationType of decorationTypes) {
    decorationType.dispose();
  }
  decorationTypes = [];
}

async function pickCoverageReport(searchGlobs: string[]): Promise<string | undefined> {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) {
    void vscode.window.showInformationMessage("Open a workspace folder to discover Trident coverage reports.");
    return undefined;
  }

  const discovered = new Map<string, vscode.Uri>();
  for (const pattern of searchGlobs) {
    const matches = await vscode.workspace.findFiles(
      new vscode.RelativePattern(workspaceFolder, pattern),
      new vscode.RelativePattern(workspaceFolder, "**/{target,node_modules,.git}/**"),
      20,
    );
    for (const uri of matches) {
      discovered.set(uri.fsPath, uri);
    }
  }

  const choices = [...discovered.values()].map((uri) => ({
    label: path.basename(uri.fsPath),
    description: vscode.workspace.asRelativePath(uri),
    uri,
  }));

  if (choices.length === 0) {
    void vscode.window.showWarningMessage(
      "No Trident coverage JSON found. Set seagrass.tridentCoverage.reportPath or run Trident with coverage format = \"json\".",
    );
    return undefined;
  }

  if (choices.length === 1) {
    return choices[0]?.uri.fsPath;
  }

  const picked = await vscode.window.showQuickPick(choices, {
    placeHolder: "Select a Trident llvm-cov JSON report",
  });
  return picked?.uri.fsPath;
}

async function resolveWorkspacePath(configuredPath: string): Promise<string> {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) {
    return configuredPath;
  }
  return path.isAbsolute(configuredPath)
    ? configuredPath
    : path.resolve(workspaceFolder.uri.fsPath, configuredPath);
}

function watchCoverageReport(
  context: vscode.ExtensionContext,
  reportPath: string,
  editor: vscode.TextEditor,
  showExecutionCount: boolean,
): void {
  fileWatcher?.dispose();
  const watcher = vscode.workspace.createFileSystemWatcher(reportPath);
  watcher.onDidChange(() => {
    void loadCoverageReport(reportPath)
      .then((report) => {
        const fileCoverage = coverageForEditorFile(report, editor.document.uri);
        if (!fileCoverage) {
          return;
        }
        applyCoverageDecorations(editor, fileCoverage, showExecutionCount);
      })
      .catch(() => undefined);
  });
  fileWatcher = watcher;
  context.subscriptions.push(watcher);
}
