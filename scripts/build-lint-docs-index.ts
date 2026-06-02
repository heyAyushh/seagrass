#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const topicsPath = resolve(repoRoot, "docs/topics.json");
const lintsRoot = resolve(repoRoot, "docs/lints");
const outputPath = resolve(lintsRoot, "index.html");

type TopicManifest = {
  topics: Array<{ name: string; description: string }>;
};

function topicSlug(topic: string): string {
  return topic.replace("seagrass/", "seagrass-").replaceAll(".", "-").replaceAll("/", "-");
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

const checkMode = process.argv.includes("--check");

const manifest = JSON.parse(readFileSync(topicsPath, "utf8")) as TopicManifest;
const rows = manifest.topics
  .map((topic) => {
    const slug = topicSlug(topic.name);
    const page = `${slug}.md`;
    const exists = readdirSync(lintsRoot).includes(page);
    const href = exists ? page : `https://github.com/heyAyushh/seagrass/blob/main/docs/lints/${page}`;
    return `<tr>
  <td><code>${escapeHtml(topic.name)}</code></td>
  <td>${escapeHtml(topic.description)}</td>
  <td><a href="${href}">${escapeHtml(page)}</a></td>
</tr>`;
  })
  .join("\n");

const html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Seagrass lint catalog</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 960px; line-height: 1.5; }
    table { border-collapse: collapse; width: 100%; }
    th, td { border: 1px solid #ccc; padding: 0.5rem 0.75rem; text-align: left; vertical-align: top; }
    th { background: #f4f4f4; }
    code { font-size: 0.9em; }
  </style>
</head>
<body>
  <h1>Seagrass lint catalog</h1>
  <p>Browsable index of <code>docs/lints/</code> topics. Regenerate with <code>bun scripts/build-lint-docs-index.ts</code>.</p>
  <table>
    <thead><tr><th>Topic</th><th>Description</th><th>Doc</th></tr></thead>
    <tbody>
${rows}
    </tbody>
  </table>
</body>
</html>
`;

const previous = existsSync(outputPath) ? readFileSync(outputPath, "utf8") : "";
writeFileSync(outputPath, html);
if (checkMode && previous !== html) {
  console.error(
    `docs/lints/index.html is stale; run: bun scripts/build-lint-docs-index.ts`,
  );
  process.exit(1);
}
console.log(`wrote ${outputPath} (${manifest.topics.length} topics)`);