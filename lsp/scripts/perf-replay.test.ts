import { describe, expect, test } from "bun:test";

import {
  assertHotpathBudgetReport,
  handlerBudgets,
  parseDurationNanos,
} from "./perf-replay.ts";

describe("hotpath perf replay report parsing", () => {
  test("parseDurationNanos reads hotpath duration strings", () => {
    expect(parseDurationNanos("42 ns")).toBe(42);
    expect(parseDurationNanos("1.5 µs")).toBe(1_500);
    expect(parseDurationNanos("2.0 ms")).toBe(2_000_000);
    expect(parseDurationNanos("1.25 s")).toBe(1_250_000_000);
  });

  test("assertHotpathBudgetReport accepts measured handlers below budget", () => {
    expect(() =>
      assertHotpathBudgetReport({
        functions_timing: {
          data: handlerBudgets.map((budget) => ({
            name: budget.name,
            calls: 3,
            p99: `${budget.p99Millis - 1}.0 ms`,
          })),
        },
      }),
    ).not.toThrow();
  });

  test("assertHotpathBudgetReport matches exact hotpath labels", () => {
    expect(() =>
      assertHotpathBudgetReport({
        functions_timing: {
          data: [
            {
              name: "seagrass::server::diagnostic_pipeline::publish_analysis",
              calls: 1,
              p99: "999.0 ms",
            },
            ...handlerBudgets.map((budget) => ({
              name: budget.name,
              calls: 1,
              p99: "1.0 ms",
            })),
          ],
        },
      }),
    ).not.toThrow();
  });

  test("assertHotpathBudgetReport rejects missing handlers", () => {
    expect(() => assertHotpathBudgetReport({ functions_timing: { data: [] } })).toThrow(
      /missing hotpath measurement/,
    );
  });

  test("assertHotpathBudgetReport rejects p99 budget regressions", () => {
    const [budget] = handlerBudgets;
    expect(() =>
      assertHotpathBudgetReport({
        functions_timing: {
          data: [
            {
              name: budget.name,
              calls: 1,
              p99: `${budget.p99Millis + 1}.0 ms`,
            },
            ...handlerBudgets.slice(1).map((otherBudget) => ({
              name: otherBudget.name,
              calls: 1,
              p99: "1.0 ms",
            })),
          ],
        },
      }),
    ).toThrow(/exceeded p99 budget/);
  });
});
