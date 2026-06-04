import * as vscode from "vscode";
import { type ConfidenceTier, parseConfidenceMessage } from "./confidenceMeta";

const DIAGNOSTIC_SOURCE = "seagrass";

let decorationTypes: Partial<Record<ConfidenceTier, vscode.TextEditorDecorationType>> = {};

export function registerConfidencePresentation(context: vscode.ExtensionContext): void {
  decorationTypes = {
    derived: vscode.window.createTextEditorDecorationType({
      borderWidth: "0 0 1px 0",
      borderStyle: "dotted",
      borderColor: "rgba(80, 160, 255, 0.9)",
      overviewRulerColor: "rgba(80, 160, 255, 0.8)",
      overviewRulerLane: vscode.OverviewRulerLane.Center,
    }),
    heuristic: vscode.window.createTextEditorDecorationType({
      borderWidth: "0 0 1px 0",
      borderStyle: "dashed",
      borderColor: "rgba(230, 180, 60, 0.95)",
      overviewRulerColor: "rgba(230, 180, 60, 0.9)",
      overviewRulerLane: vscode.OverviewRulerLane.Center,
    }),
  };

  for (const decorationType of Object.values(decorationTypes)) {
    if (decorationType) {
      context.subscriptions.push(decorationType);
    }
  }

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => refreshConfidenceDecorations(editor)),
    vscode.languages.onDidChangeDiagnostics(() => {
      refreshConfidenceDecorations(vscode.window.activeTextEditor);
    }),
  );
}

export function refreshConfidenceDecorations(editor: vscode.TextEditor | undefined): void {
  if (!enabled()) {
    clearConfidenceDecorations(editor);
    return;
  }
  if (!editor || editor.document.languageId !== "rust") {
    clearConfidenceDecorations(editor);
    return;
  }

  const tiers: Record<ConfidenceTier, vscode.Range[]> = {
    authoritative: [],
    derived: [],
    heuristic: [],
  };

  for (const diagnostic of vscode.languages.getDiagnostics(editor.document.uri)) {
    if (diagnostic.source !== DIAGNOSTIC_SOURCE) {
      continue;
    }
    const tier = confidenceTier(diagnostic);
    if (!tier || tier === "authoritative") {
      continue;
    }
    tiers[tier].push(diagnostic.range);
  }

  if (decorationTypes.derived) {
    editor.setDecorations(decorationTypes.derived, tiers.derived);
  }
  if (decorationTypes.heuristic) {
    editor.setDecorations(decorationTypes.heuristic, tiers.heuristic);
  }
}

export function confidenceTier(diagnostic: vscode.Diagnostic): ConfidenceTier | undefined {
  for (const info of diagnostic.relatedInformation ?? []) {
    const parsed = parseConfidenceMessage(info.message);
    if (parsed) {
      return parsed;
    }
  }
  return undefined;
}

export { parseConfidenceMessage } from "./confidenceMeta";

function enabled(): boolean {
  return vscode.workspace.getConfiguration("seagrass.diagnostics").get<boolean>("confidenceDecorations", true);
}

function clearConfidenceDecorations(editor: vscode.TextEditor | undefined): void {
  if (!editor) {
    return;
  }
  if (decorationTypes.derived) {
    editor.setDecorations(decorationTypes.derived, []);
  }
  if (decorationTypes.heuristic) {
    editor.setDecorations(decorationTypes.heuristic, []);
  }
}