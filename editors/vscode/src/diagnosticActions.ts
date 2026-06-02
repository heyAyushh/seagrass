export type DiagnosticMetadata = {
  confidence?: string;
  topic?: string;
  applicability?: string;
  quickfix?: string;
};

export type FalsePositiveReportInput = {
  file: string;
  range: string;
  message: string;
  sourceLine?: string;
  code?: string;
  docsUrl?: string;
  metadata: DiagnosticMetadata;
};

const SEAGRASS_TOPIC_PREFIX = "seagrass/";
const SEAGRASS_METADATA_PREFIX = "Seagrass confidence:";
const LINT_DOCS_BASE_URL = "https://github.com/heyAyushh/seagrass/blob/main/docs/lints";

export function parseDiagnosticMetadata(messages: readonly string[]): DiagnosticMetadata {
  const summary = messages.find((message) => message.startsWith(SEAGRASS_METADATA_PREFIX));
  if (!summary) {
    return {};
  }

  const metadata: DiagnosticMetadata = {};
  for (const part of summary.split(";")) {
    const [rawKey, ...rawValue] = part.split(":");
    const key = rawKey.trim();
    const value = rawValue.join(":").trim();
    if (!value) {
      continue;
    }
    assignMetadataField(metadata, key, value);
  }
  return metadata;
}

export function topicSlug(topic: string): string {
  return topic
    .replace(SEAGRASS_TOPIC_PREFIX, "seagrass-")
    .replace(/[./]/g, "-");
}

export function lintDocUrlFromTopic(topic: string): string {
  return `${LINT_DOCS_BASE_URL}/${topicSlug(topic)}.md`;
}

export function suppressionSnippet(topic: string): string {
  return `// seagrass-allow: ${topic}`;
}

export function formatFalsePositiveReport(input: FalsePositiveReportInput): string {
  const lines = [
    "Seagrass false-positive report",
    `file: ${input.file}`,
    `range: ${input.range}`,
  ];
  appendField(lines, "topic", input.metadata.topic);
  appendField(lines, "confidence", input.metadata.confidence);
  appendField(lines, "applicability", input.metadata.applicability);
  appendField(lines, "quickfix", input.metadata.quickfix);
  appendField(lines, "code", input.code);
  appendField(lines, "docs", input.docsUrl);
  lines.push(`message: ${input.message}`);
  if (input.sourceLine) {
    lines.push("source:");
    lines.push(input.sourceLine);
  }
  return lines.join("\n");
}

function assignMetadataField(metadata: DiagnosticMetadata, key: string, value: string): void {
  if (key === "Seagrass confidence") {
    metadata.confidence = value;
  } else if (key === "topic") {
    metadata.topic = value;
  } else if (key === "applicability") {
    metadata.applicability = value;
  } else if (key === "quickfix") {
    metadata.quickfix = value;
  }
}

function appendField(lines: string[], label: string, value: string | undefined): void {
  if (value) {
    lines.push(`${label}: ${value}`);
  }
}
