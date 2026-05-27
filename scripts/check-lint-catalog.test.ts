import { describe, expect, test } from "bun:test";

import { lintPage, pageContentFailures } from "./check-lint-catalog.ts";

const topic = {
  name: "seagrass/solana.code-quality.unchecked-arithmetic",
  description: "Unchecked balance and lamports arithmetic diagnostics.",
};

describe("lint catalog checker", () => {
  test("rejects generic generated boilerplate pages", () => {
    const failures = pageContentFailures(
      topic,
      "docs/lints/seagrass-solana-code-quality-unchecked-arithmetic.md",
      `
# Unchecked Arithmetic

Topic: \`${topic.name}\`

## What It Catches

${topic.description}

This page documents the user-facing diagnostic topic.

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| topic-shaped text in comments or strings | no diagnostic |

## Suppression

\`\`\`rust
// seagrass-allow: ${topic.name}
\`\`\`
`,
    );

    expect(failures.join("\n")).toContain("generic boilerplate");
    expect(failures.join("\n")).toContain("seagrass-allow-file");
    expect(failures.join("\n")).toContain("#[seagrass(allow");
    expect(failures.join("\n")).toContain("[lints]");
  });

  test("generated pages document all suppression forms", () => {
    const page = lintPage(topic);
    const failures = pageContentFailures(
      topic,
      "docs/lints/seagrass-solana-code-quality-unchecked-arithmetic.md",
      page,
    );

    expect(failures).toEqual([]);
    expect(page).toContain(`// seagrass-allow: ${topic.name}`);
    expect(page).toContain(`// seagrass-allow-file: ${topic.name}`);
    expect(page).toContain("Item or block suppression");
    expect(page).toContain(`#[seagrass(allow("${topic.name}"))]`);
    expect(page).toContain(`allow = ["${topic.name}"]`);
  });
});
