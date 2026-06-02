import { describe, expect, test } from "bun:test";

import {
  coverageForDocumentPath,
  lineExecutionsForFile,
  type CoverageFile,
  type CoverageReport,
} from "./src/tridentCoverageModel.ts";

describe("trident coverage bridge", () => {
  test("aggregates execution counts per source line", () => {
    const file: CoverageFile = {
      filename: "/workspace/programs/demo/src/lib.rs",
      segments: [
        {
          line: 10,
          column: 4,
          execution_count: 2,
          has_count: true,
          is_gap_region: false,
        },
        {
          line: 10,
          column: 12,
          execution_count: 5,
          has_count: true,
          is_gap_region: false,
        },
        {
          line: 11,
          column: 4,
          execution_count: 0,
          has_count: true,
          is_gap_region: false,
        },
      ],
    };

    const lines = lineExecutionsForFile(file);
    expect(lines.find((entry) => entry.line === 10)?.executionCount).toBe(5);
    expect(lines.find((entry) => entry.line === 10)?.covered).toBe(true);
    expect(lines.find((entry) => entry.line === 11)?.covered).toBe(false);
  });

  test("matches workspace files by exact path or unique relative suffix", () => {
    const report: CoverageReport = {
      data: [
        {
          files: [
            {
              filename: "programs/demo/src/lib.rs",
              segments: [
                {
                  line: 3,
                  column: 0,
                  execution_count: 1,
                  has_count: true,
                  is_gap_region: false,
                },
              ],
            },
          ],
        },
      ],
    };

    const coverage = coverageForDocumentPath(report, "/Users/dev/programs/demo/src/lib.rs");

    expect(coverage?.find((entry) => entry.line === 3)?.executionCount).toBe(1);
  });

  test("does not guess from basename-only coverage entries", () => {
    const report: CoverageReport = {
      data: [
        {
          files: [
            {
              filename: "lib.rs",
              segments: [
                {
                  line: 3,
                  column: 0,
                  execution_count: 1,
                  has_count: true,
                  is_gap_region: false,
                },
              ],
            },
          ],
        },
      ],
    };

    expect(coverageForDocumentPath(report, "/Users/dev/programs/demo/src/lib.rs")).toBeUndefined();
  });

  test("does not decorate when relative coverage suffix is ambiguous", () => {
    const report: CoverageReport = {
      data: [
        {
          files: [
            {
              filename: "src/lib.rs",
              segments: [
                {
                  line: 3,
                  column: 0,
                  execution_count: 1,
                  has_count: true,
                  is_gap_region: false,
                },
              ],
            },
            {
              filename: "src/lib.rs",
              segments: [
                {
                  line: 4,
                  column: 0,
                  execution_count: 1,
                  has_count: true,
                  is_gap_region: false,
                },
              ],
            },
          ],
        },
      ],
    };

    expect(coverageForDocumentPath(report, "/Users/dev/programs/demo/src/lib.rs")).toBeUndefined();
  });
});
