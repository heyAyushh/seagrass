import * as path from "path";

const MIN_RELATIVE_COVERAGE_PATH_SEGMENTS = 2;

export type CoverageSegment = {
  line: number;
  column: number;
  execution_count: number;
  has_count: boolean;
  is_gap_region: boolean;
};

export type CoverageFile = {
  filename: string;
  segments: CoverageSegment[];
};

export type CoverageReport = {
  data: Array<{
    files: CoverageFile[];
  }>;
};

export type LineExecution = {
  line: number;
  executionCount: number;
  covered: boolean;
};

export type CoverageSummary = {
  totalLines: number;
  coveredLines: number;
  uncoveredLines: number;
};

export function lineExecutionsForFile(file: CoverageFile): LineExecution[] {
  const counts = new Map<number, number>();
  for (const segment of file.segments) {
    if (!segment.has_count || segment.is_gap_region) {
      continue;
    }
    const line = Math.max(segment.line, 1);
    const previous = counts.get(line) ?? 0;
    counts.set(line, Math.max(previous, segment.execution_count));
  }

  const maxLine = Math.max(file.segments.reduce((max, segment) => Math.max(max, segment.line), 1), counts.size);
  const lines: LineExecution[] = [];
  for (let line = 1; line <= maxLine; line += 1) {
    const executionCount = counts.get(line) ?? 0;
    lines.push({
      line,
      executionCount,
      covered: executionCount > 0,
    });
  }
  return lines;
}

export function coverageForDocumentPath(report: CoverageReport, documentPath: string): LineExecution[] | undefined {
  const files = report.data.flatMap((entry) => entry.files);
  const match = findCoverageFile(files, documentPath);
  if (!match) {
    return undefined;
  }
  return lineExecutionsForFile(match);
}

export function summarizeLineCoverage(lineCoverage: readonly LineExecution[]): CoverageSummary {
  const coveredLines = lineCoverage.filter((entry) => entry.covered).length;
  return {
    totalLines: lineCoverage.length,
    coveredLines,
    uncoveredLines: lineCoverage.length - coveredLines,
  };
}

export function tridentPromotionHint(summary: CoverageSummary): string | undefined {
  if (summary.totalLines === 0) {
    return undefined;
  }
  if (summary.uncoveredLines > 0) {
    return "Use uncovered lines as trident-tests targets before promoting heuristic security findings.";
  }
  return "All reported lines are covered; promotion still needs topic-specific false-positive review.";
}

function findCoverageFile(files: CoverageFile[], documentPath: string): CoverageFile | undefined {
  const exact = files.find((file) => path.resolve(file.filename) === path.resolve(documentPath));
  if (exact) {
    return exact;
  }

  const suffixMatches = files.filter((file) => isUniqueRelativeSuffixCandidate(file.filename, documentPath));
  return suffixMatches.length === 1 ? suffixMatches[0] : undefined;
}

function isUniqueRelativeSuffixCandidate(coveragePath: string, documentPath: string): boolean {
  const coverageSegments = pathSegments(coveragePath);
  if (coverageSegments.length < MIN_RELATIVE_COVERAGE_PATH_SEGMENTS) {
    return false;
  }

  const documentSegments = pathSegments(documentPath);
  if (coverageSegments.length > documentSegments.length) {
    return false;
  }

  const offset = documentSegments.length - coverageSegments.length;
  return coverageSegments.every((segment, index) => segment === documentSegments[offset + index]);
}

function pathSegments(value: string): string[] {
  return value.replaceAll("\\", "/").split("/").filter((segment) => segment.length > 0);
}
