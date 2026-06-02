import { describe, expect, test } from "bun:test";

import { parseConfidenceMessage } from "./src/confidenceMeta.ts";

describe("confidence presentation", () => {
  test("parses server related-information metadata", () => {
    expect(
      parseConfidenceMessage(
        "Seagrass confidence: derived; topic: seagrass/anchor.account.usage",
      ),
    ).toBe("derived");
    expect(parseConfidenceMessage("Seagrass confidence: authoritative")).toBe("authoritative");
    expect(parseConfidenceMessage("unrelated")).toBeUndefined();
  });
});
