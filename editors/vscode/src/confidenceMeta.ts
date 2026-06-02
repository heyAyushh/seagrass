export type ConfidenceTier = "authoritative" | "derived" | "heuristic";

export const CONFIDENCE_PREFIX = "Seagrass confidence:";

export function parseConfidenceMessage(message: string): ConfidenceTier | undefined {
  if (!message.startsWith(CONFIDENCE_PREFIX)) {
    return undefined;
  }
  if (message.includes("authoritative")) {
    return "authoritative";
  }
  if (message.includes("derived")) {
    return "derived";
  }
  if (message.includes("heuristic")) {
    return "heuristic";
  }
  return undefined;
}