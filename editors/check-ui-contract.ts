import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const editorsDir = dirname(fileURLToPath(import.meta.url));

const expectedCommands = new Map([
  ["seagrass.status", "Seagrass: Status"],
  ["seagrass.analyze", "Seagrass: Analyze Document"],
  ["seagrass.artifacts", "Seagrass: Artifacts"],
  ["seagrass.logs", "Seagrass: Recent Logs"],
  ["seagrass.errorCoverage", "Seagrass: Error Coverage"],
  ["seagrass.supportMatrix", "Seagrass: Support Matrix"],
  ["seagrass.generatorProfile", "Seagrass: Generator Profile"],
  ["seagrass.restart", "Seagrass: Restart Server"],
  ["seagrass.showOutput", "Seagrass: Output"],
]);

const packageJson = JSON.parse(read("vscode/package.json"));
const vscodeSource = read("vscode/src/extension.ts");
const zedToml = read("zed/extension.toml");
const contract = read("UI_CONTRACT.md");

for (const [command, title] of expectedCommands) {
  const contribution = packageJson.contributes.commands.find((entry) => entry.command === command);
  assert(contribution, `VS Code package is missing command ${command}`);
  assert(contribution.title === title, `VS Code command ${command} should be titled ${title}`);
  assert(contract.includes(`\`${title}\``), `UI contract is missing command title ${title}`);
}

for (const line of ["Seagrass", "server:", "cwd:", "sync:", "diagnostics:", "workspaces:", "features:"]) {
  assert(vscodeSource.includes(line), `VS Code startup output is missing ${line}`);
  assert(contract.includes(line), `UI contract startup shape is missing ${line}`);
}

assert(vscodeSource.includes("createStatusBarItem"), "VS Code client should expose Seagrass status in the status bar");
assert(vscodeSource.includes("DIAGNOSTIC_SOURCE"), "VS Code status should count Seagrass diagnostics by source");
assert(
  vscodeSource.includes('const DIAGNOSTIC_SOURCE = "seagrass"'),
  "VS Code status should count rebranded Seagrass diagnostics",
);
assert(zedToml.includes('name = "Seagrass"'), "Zed extension name should be Seagrass");
assert(
  zedToml.includes("Anchor-specific diagnostics, completions, hovers, symbols, and fixes"),
  "Zed extension description should match the shared product role",
);

function read(relativePath) {
  return readFileSync(join(editorsDir, relativePath), "utf8");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(String(message));
  }
}
