import { parseDiagnosticMetadata } from "./diagnosticActions";

export type ConfidenceTier = "authoritative" | "derived" | "heuristic";

export const CONFIDENCE_PREFIX = "Seagrass confidence:";

export function parseConfidenceMessage(message: string): ConfidenceTier | undefined {
  if (!message.startsWith(CONFIDENCE_PREFIX)) {
    return undefined;
  }
  const confidence = parseDiagnosticMetadata([message]).confidence;
  if (confidence === "authoritative") {
    return "authoritative";
  }
  if (confidence === "derived") {
    return "derived";
  }
  if (confidence === "heuristic") {
    return "heuristic";
  }
  return undefined;
}
