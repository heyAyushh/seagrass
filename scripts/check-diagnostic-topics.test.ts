#!/usr/bin/env bun

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  isProductionDiagnosticSourcePath,
  parseManifest,
  sourceTopicsFromText,
  topicManifestFailures,
  topicPattern,
} from "./check-diagnostic-topics";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");
const topicsSchemaPath = resolve(repoRoot, "lsp/docs/topics.schema.json");

const accountUsage = {
  name: "seagrass/anchor.account.usage",
  description: "Anchor account mutability and usage diagnostics.",
};
const ownerCheck = {
  name: "seagrass/security.owner-check",
  description: "Owner-check security diagnostics.",
};

describe("diagnostic topic checker", () => {
  test("rejects unsorted duplicate and malformed manifest topics", () => {
    const failures = topicManifestFailures(
      [
        ownerCheck,
        accountUsage,
        ownerCheck,
        { name: "anchor.account.usage", description: "Missing namespace." },
      ],
      [accountUsage.name, ownerCheck.name],
    );

    expect(failures.join("\n")).toContain("topics manifest must be sorted by name");
    expect(failures.join("\n")).toContain("topics manifest contains duplicate topics");
    expect(failures.join("\n")).toContain(`topics must match ${topicPattern}`);
  });

  test("rejects manifest drift against emitted diagnostic topics", () => {
    const failures = topicManifestFailures(
      [accountUsage],
      [accountUsage.name, "seagrass/security.type-cosplay"],
    );

    expect(failures.join("\n")).toContain("diagnostic topic manifest drift detected");
    expect(failures.join("\n")).toContain("seagrass/security.type-cosplay");
    expect(failures.join("\n")).toContain("declared but not emitted by diagnostics source: none");
  });

  test("extracts unique seagrass topics from Rust diagnostic source", () => {
    const topics = sourceTopicsFromText(`
      const OWNER_TOPIC: &str = "seagrass/security.owner-check";
      let metadata = json!({ "topic": "seagrass/security.owner-check" });
      let registry = AnchorDiagnosticKind::SecurityOwnerCheck => "seagrass/security.owner-check";
      let unrelated = "file:///seagrass/current-document.rs";
      let wrong = "anchor-lsp/security.owner-check";
    `);

    expect(topics).toEqual(["seagrass/security.owner-check"]);
  });

  test("ignores topic-shaped strings without production diagnostic evidence", () => {
    const topics = sourceTopicsFromText(`
      // "seagrass/security.comment-only"
      let fixture = "seagrass/security.test-fixture";
      assert_eq!(topic, "seagrass/security.assertion-only");
    `);

    expect(topics).toEqual([]);
  });

  test("ignores diagnostic test files when collecting emitted source topics", () => {
    const diagnosticsRoot = resolve(repoRoot, "lsp/src/diagnostics");

    expect(isProductionDiagnosticSourcePath(resolve(diagnosticsRoot, "security.rs"))).toBe(true);
    expect(isProductionDiagnosticSourcePath(resolve(diagnosticsRoot, "security/tests.rs"))).toBe(false);
    expect(isProductionDiagnosticSourcePath(resolve(diagnosticsRoot, "code_quality_tests.rs"))).toBe(false);
    expect(isProductionDiagnosticSourcePath(resolve(diagnosticsRoot, "security/tests/raw_account_tests.rs"))).toBe(false);
  });

  test("parses valid topic manifests", () => {
    const manifest = parseManifest(
      JSON.stringify({
        $schema: "./topics.schema.json",
        schemaVersion: 1,
        topics: [accountUsage],
      }),
    );

    expect(manifest.topics).toEqual([accountUsage]);
  });

  test("keeps JSON schema aligned with the checker and manifest", () => {
    const schema = JSON.parse(readFileSync(topicsSchemaPath, "utf8")) as {
      properties?: {
        $schema?: { const?: string };
        topics?: { items?: { properties?: { name?: { pattern?: string } } } };
      };
    };

    const schemaPattern = schema.properties?.topics?.items?.properties?.name?.pattern;
    expect(schema.properties?.$schema?.const).toBe("./topics.schema.json");
    expect(schemaPattern).toBeDefined();
    expect(new RegExp(schemaPattern ?? "").source).toBe(topicPattern.source);
  });
});
