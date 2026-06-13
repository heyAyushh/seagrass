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
const expectedServerCommands = [
  "seagrass/status",
  "seagrass/analyze",
  "seagrass/artifacts",
  "seagrass/programReport",
  "seagrass/feedback",
  "seagrass/logs",
  "seagrass/errorCoverage",
  "seagrass/supportMatrix",
  "seagrass/generatorProfile",
  "seagrass/projectCoverage",
];
const expectedZedSlashCommands = new Map([
  ["seagrass-status", "Show Seagrass server status"],
  ["seagrass-analyze", "Show Seagrass analysis report"],
  ["seagrass-coverage", "Show Seagrass project coverage"],
  ["seagrass-artifacts", "Show Seagrass artifact report"],
  ["seagrass-program-report", "Show Seagrass program report"],
  ["seagrass-error-coverage", "Show Anchor error coverage"],
  ["seagrass-support-matrix", "Show Anchor support matrix"],
  ["seagrass-generator-profile", "Show Anchor generator profile"],
  ["seagrass-logs", "Show recent Seagrass logs"],
  ["seagrass-feedback", "Show Seagrass feedback URL"],
]);
const expectedVimFamilyCommands = [
  "SeagrassInfo",
  "SeagrassStatus",
  "SeagrassAnalyze",
  "SeagrassArtifacts",
  "SeagrassProgramReport",
  "SeagrassErrorCoverage",
  "SeagrassSupportMatrix",
  "SeagrassGeneratorProfile",
  "SeagrassLogs",
  "SeagrassProjectCoverage",
  "SeagrassFeedback",
  "SeagrassRestart",
  "SeagrassDiagnostics",
  "SeagrassAnalyzeCli",
];

const packageJson = JSON.parse(read("vscode/package.json"));
const vscodeSource = read("vscode/src/extension.ts");
const zedToml = read("zed/extension.toml");
const vimPlugin = read("vim/plugin/seagrass.vim");
const vimReadme = read("vim/README.md");
const vimHelp = read("vim/doc/seagrass.txt");
const vimCocSettings = JSON.parse(read("vim/coc-settings.json"));
const nvimReadme = read("nvim/README.md");
const nvimLua = read("nvim/lua/seagrass/init.lua");
const nvimPlugin = read("nvim/plugin/seagrass.lua");
const viReadme = read("vi/README.md");
const feedbackManifest = read("feedback.toml");
const contract = read("UI_CONTRACT.md");
const recognizedSettingsManifest = JSON.parse(read("recognized-settings.json"));
const vscodeSettings = packageJson.contributes.configuration.properties;
const recognizedSettings = new Set(recognizedSettingsManifest.recognizedSettings);
const extensionLocalSettings = new Map([
  ["serverCommand", "VS Code launch command; resolved before the LSP starts."],
  ["serverArgs", "VS Code launch arguments; resolved before the LSP starts."],
  ["serverCwd", "VS Code launch cwd; resolved before the LSP starts."],
  ["serverEnv", "VS Code launch environment; resolved before the LSP starts."],
  ["dev.useCargoFromCheckout", "VS Code development launcher switch."],
  ["diagnostics.confidenceDecorations", "VS Code-only presentation of server metadata."],
  ["tridentCoverage.reportPath", "VS Code-only Trident coverage overlay input."],
  ["tridentCoverage.searchGlobs", "VS Code-only Trident coverage discovery input."],
  ["tridentCoverage.showExecutionCount", "VS Code-only Trident gutter display toggle."],
  ["inlayHints.enabled", "VS Code inlay-hint presentation toggle; no server setting exists yet."],
]);
const hiddenServerSettings = new Map([
  ["diagnostics.security.strictNative.enabled", "Legacy alias for security.strictNative.enabled."],
  ["editor.client", "Adapter identity supplied by clients/templates, not a user knob."],
]);
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

for (const command of expectedServerCommands) {
  assert(contract.includes(`\`${command}\``), `UI contract is missing server command ${command}`);
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

assert(vimReadme.includes("vim-lsp"), "Vim README should document vim-lsp as the transport");
assert(vimReadme.includes("coc-settings.json"), "Vim README should document the CoC template");
assert(vimPlugin.includes("'name': s:server_name"), "Vim package should register the shared server name");
assert(vimPlugin.includes("'allowlist': ['rust']"), "Vim package should allow Rust buffers");
assert(vimPlugin.includes("'whitelist': ['rust']"), "Vim package should keep old vim-lsp Rust allowlist compatibility");
assert(vimPlugin.includes("'Anchor.toml'"), "Vim package should use Anchor.toml as a root marker");
assert(vimPlugin.includes("'Seagrass.toml'"), "Vim package should use Seagrass.toml as a root marker");
assert(vimPlugin.includes("'Cargo.toml'"), "Vim package should use Cargo.toml as a root marker");
assert(vimPlugin.includes("'editor.client': 'vim'"), "Vim package should identify the editor client");
assert(vimPlugin.includes("'diagnostics.transport': 'push'"), "Vim package should default diagnostics transport to push");
assert(!vimPlugin.includes("'inlayHints.enabled'"), "Vim package should not send unrecognized inlayHints.enabled settings");
assert(vimPlugin.includes("workspace/executeCommand"), "Vim package should expose Seagrass execute-command reports");
assert(vimPlugin.includes("setqflist"), "Vim package should expose CLI diagnostics through quickfix");
assert(vimPlugin.includes("json_decode"), "Vim package should parse CLI diagnostic JSON");
assert(contract.includes("client as `vim`"), "UI contract should document the Vim client id");
for (const command of expectedVimFamilyCommands) {
  assert(vimPlugin.includes(`command!`) && vimPlugin.includes(command), `Vim package is missing :${command}`);
  assert(vimReadme.includes(`:${command}`), `Vim README is missing :${command}`);
  assert(vimHelp.includes(`:${command}`), `Vim help is missing :${command}`);
  assert(contract.includes(`:${command}`), `UI contract is missing :${command}`);
}

const cocServer = vimCocSettings.languageserver.seagrass;
assert(cocServer.command === "seagrass", "CoC template should start the seagrass binary");
assert(cocServer.filetypes.includes("rust"), "CoC template should attach to Rust buffers");
assert(cocServer.rootPatterns.includes("Anchor.toml"), "CoC template is missing Anchor.toml root marker");
assert(cocServer.rootPatterns.includes("Seagrass.toml"), "CoC template is missing Seagrass.toml root marker");
assert(cocServer.rootPatterns.includes("Cargo.toml"), "CoC template is missing Cargo.toml root marker");
assert(
  cocServer.initializationOptions.seagrass.editor.client === "vim",
  "CoC template should send editor.client=vim",
);
assert(
  cocServer.settings.seagrass["diagnostics.transport"] === "push",
  "CoC template should keep push diagnostics",
);

assert(nvimReadme.includes("vim.lsp.config"), "Neovim README should document the built-in LSP path");
assert(nvimReadme.includes("nvim-lspconfig"), "Neovim README should document the fallback LSP path");
assert(nvimPlugin.includes("vim.g.loaded_seagrass_nvim"), "Neovim package should guard duplicate loads");
assert(nvimLua.includes('["editor.client"] = "nvim"'), "Neovim package should identify the editor client");
assert(nvimLua.includes('["diagnostics.transport"] = "push"'), "Neovim package should default diagnostics transport to push");
assert(nvimLua.includes("vim.lsp.config"), "Neovim package should support built-in LSP config");
assert(nvimLua.includes("nvim-lspconfig"), "Neovim package should support nvim-lspconfig fallback");
assert(nvimLua.includes("workspace/executeCommand"), "Neovim package should expose Seagrass execute-command reports");
assert(nvimLua.includes("setqflist"), "Neovim package should expose CLI diagnostics through quickfix");
assert(nvimLua.includes("LSP_TO_EDITOR_INDEX_OFFSET"), "Neovim quickfix conversion should name the LSP/editor index offset");
assert(contract.includes("client as `nvim`"), "UI contract should document the Neovim client id");
for (const command of expectedVimFamilyCommands) {
  assert(nvimLua.includes(`"${command}"`), `Neovim package is missing :${command}`);
  assert(nvimReadme.includes(`:${command}`), `Neovim README is missing :${command}`);
}

assert(viReadme.includes("Classic POSIX `vi`"), "vi README should state the classic vi boundary");
assert(viReadme.includes("seagrass diagnostics"), "vi README should document CLI diagnostics");
assert(viReadme.includes("../vim"), "vi README should point Vim users to the Vim package");
assert(viReadme.includes("../nvim"), "vi README should point Neovim users to the Neovim package");
assert(contract.includes("Classic POSIX `vi` has no LSP"), "UI contract should document vi as CLI-only");
assertSettingsParity();

function assertSettingsParity() {
  assert(
    Array.isArray(recognizedSettingsManifest.recognizedSettings),
    "recognized settings manifest must expose recognizedSettings",
  );
  assert(
    recognizedSettingsManifest.source === "src/runtime/server_types/mod.rs::RECOGNIZED_SETTING_KEYS",
    "recognized settings manifest points at the wrong source",
  );

  const vscodeServerKeys = Object.keys(vscodeSettings)
    .filter((key) => key.startsWith("seagrass."))
    .map((key) => key.replace(/^seagrass\./, ""));

  for (const key of vscodeServerKeys) {
    const reason = extensionLocalSettings.get(key);
    assert(
      recognizedSettings.has(key) || reason,
      `VS Code setting seagrass.${key} is not recognized by the server and has no local-only reason`,
    );
  }

  for (const key of recognizedSettings) {
    const visibleInVscode = vscodeServerKeys.includes(key);
    const hiddenReason = hiddenServerSettings.get(key);
    assert(
      visibleInVscode || hiddenReason,
      `server setting ${key} is missing from VS Code contributes.configuration`,
    );
  }

  for (const key of cocSeagrassKeys(cocServer)) {
    assert(recognizedSettings.has(key), `CoC setting ${key} is not recognized by the server`);
  }

  for (const key of uiContractSettingKeys(contract)) {
    assert(recognizedSettings.has(key), `UI contract setting ${key} is not recognized by the server`);
  }
}

function cocSeagrassKeys(server) {
  return [
    ...flattenSettingKeys(server.initializationOptions?.seagrass ?? {}),
    ...flattenSettingKeys(server.settings?.seagrass ?? {}),
  ].filter(uniqueStrings).sort(compareStrings);
}

function flattenSettingKeys(value, prefix = "") {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return [];
  }
  return Object.entries(value).flatMap(([key, entry]) => {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (entry && typeof entry === "object" && !Array.isArray(entry)) {
      return flattenSettingKeys(entry, fullKey);
    }
    return [fullKey];
  });
}

function uiContractSettingKeys(text) {
  const section = text
    .split("Keep these settings equivalent:")[1]
    ?.split("Security family settings")[0];
  assert(section, "UI contract settings section is missing");
  return [...section.matchAll(/- `([^`]+)`/g)].map((match) => match[1]);
}

function uniqueStrings(value, index, values) {
  return values.indexOf(value) === index;
}

function compareStrings(left, right) {
  return left.localeCompare(right);
}

function read(relativePath) {
  return readFileSync(join(editorsDir, relativePath), "utf8");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(String(message));
  }
}
