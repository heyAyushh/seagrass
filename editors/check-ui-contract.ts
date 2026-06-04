import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const editorsDir = dirname(fileURLToPath(import.meta.url));

const expectedCommands = new Map([
  ["seagrass.status", "Seagrass: Status"],
  ["seagrass.analyze", "Seagrass: Analyze Document"],
  ["seagrass.artifacts", "Seagrass: Artifacts"],
  ["seagrass.feedback", "Seagrass: Send Feedback"],
  ["seagrass.logs", "Seagrass: Recent Logs"],
  ["seagrass.errorCoverage", "Seagrass: Error Coverage"],
  ["seagrass.supportMatrix", "Seagrass: Support Matrix"],
  ["seagrass.generatorProfile", "Seagrass: Generator Profile"],
  ["seagrass.restart", "Seagrass: Restart Server"],
  ["seagrass.showOutput", "Seagrass: Output"],
]);
const expectedZedSlashCommands = new Map([
  ["seagrass-status", "Show Seagrass server status"],
  ["seagrass-coverage", "Show Seagrass project coverage"],
  ["seagrass-artifacts", "Show Seagrass artifact report"],
  ["seagrass-feedback", "Show Seagrass feedback URL"],
]);

const packageJson = JSON.parse(read("vscode/package.json"));
const vscodeSource = read("vscode/src/extension.ts");
const zedToml = read("zed/extension.toml");
const feedbackManifest = read("feedback.toml");
const contract = read("UI_CONTRACT.md");
const vscodeSettings = packageJson.contributes.configuration.properties;
const securityFamilies = [
  "ownerChecks",
  "typeCosplay",
  "accountClosing",
  "initialization",
  "staleCpiReload",
  "signerAuthorization",
  "writableAccounts",
  "arbitraryCpi",
  "instructionDataBounds",
  "pdaSeedCollision",
];
const securityLevels = ["off", "warn", "error", "hint"];

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
for (const [command, description] of expectedZedSlashCommands) {
  assert(zedToml.includes(`[slash_commands.${command}]`), `Zed extension is missing /${command}`);
  assert(
    zedToml.includes(`description = "${description}"`),
    `Zed slash command /${command} should be described as ${description}`,
  );
  assert(contract.includes(`/${command}`), `UI contract is missing Zed slash command /${command}`);
}
assert(vscodeSettings["seagrass.agent.mode"], "VS Code package is missing seagrass.agent.mode");
assert(vscodeSettings["seagrass.feedback.url"], "VS Code package is missing seagrass.feedback.url");
assert(
  feedbackManifest.includes("https://t.me/+8HdUVX0F1t9lMTk1"),
  "Editor feedback manifest is missing the bundled Telegram URL",
);
assert(contract.includes("`agent.mode`"), "UI contract is missing agent.mode");

for (const family of securityFamilies) {
  const key = `seagrass.diagnostics.security.${family}`;
  const setting = vscodeSettings[key];
  assert(setting, `VS Code package is missing ${key}`);
  for (const level of securityLevels) {
    assert(setting.enum.includes(level), `${key} is missing ${level}`);
  }
  assert(contract.includes(`\`diagnostics.security.${family}\``), `UI contract is missing ${family}`);
}
assert(contract.includes("`feedback.url`"), "UI contract is missing feedback.url");

function read(relativePath) {
  return readFileSync(join(editorsDir, relativePath), "utf8");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(String(message));
  }
}
