pub(crate) const SERVER_ID: &str = "seagrass";
pub(crate) const SERVER_BINARY: &str = "seagrass";
pub(crate) const SERVER_PACKAGE: &str = "seagrass-cli";
pub(crate) const SERVER_MANIFEST_ENV: &str = "SEAGRASS_MANIFEST_PATH";
pub(crate) const DEFAULT_DIAGNOSTICS_TRANSPORT: &str = "push";
pub(crate) const SLASH_STATUS: &str = "seagrass-status";
pub(crate) const SLASH_COVERAGE: &str = "seagrass-coverage";
pub(crate) const SLASH_ARTIFACTS: &str = "seagrass-artifacts";
pub(crate) const SLASH_FEEDBACK: &str = "seagrass-feedback";
pub(crate) const START_SERVER_FIRST: &str = "Start the Seagrass server first.";
pub(crate) const NODE_EVAL_FLAG: &str = "-e";
pub(crate) const INITIALIZE_REQUEST_ID: u64 = 1;
pub(crate) const EXECUTE_COMMAND_REQUEST_ID: u64 = 2;
pub(crate) const SHUTDOWN_REQUEST_ID: u64 = 3;
pub(crate) const JSON_RPC_VERSION: &str = "2.0";
pub(crate) const CONTENT_LENGTH_HEADER: &str = "Content-Length:";
pub(crate) const HEADER_SEPARATOR: &[u8] = b"\r\n\r\n";
pub(crate) const SUCCESS_EXIT_STATUS: i32 = 0;
pub(crate) const NODE_LSP_EXECUTE_SCRIPT: &str = r#"
const { spawn } = require("node:child_process");
const LSP_COMMAND_TIMEOUT_MS = 30000;
const config = JSON.parse(process.argv[1]);
const env = Object.fromEntries(config.env);
const headerSeparator = Buffer.from("\r\n\r\n");
const child = spawn(config.command, config.args, {
  env,
  stdio: ["pipe", "pipe", "pipe"],
});
const stdout = [];
const stderr = [];
let pending = Buffer.alloc(0);
let sentExecute = false;
let sentShutdown = false;
const timer = setTimeout(() => {
  child.kill();
}, LSP_COMMAND_TIMEOUT_MS);
function send(frame) {
  child.stdin.write(frame);
}
function handleMessage(message) {
  if (message.id === config.initializeId && !sentExecute) {
    sentExecute = true;
    send(config.messages.initialized);
    send(config.messages.execute);
  } else if (message.id === config.executeId && !sentShutdown) {
    sentShutdown = true;
    send(config.messages.shutdown);
  } else if (message.id === config.shutdownId) {
    send(config.messages.exit);
    child.stdin.end();
  }
}
function parsePending() {
  while (pending.length > 0) {
    const headerEnd = pending.indexOf(headerSeparator);
    if (headerEnd < 0) {
      return;
    }
    const header = pending.subarray(0, headerEnd).toString("utf8");
    const lengthMatch = header.match(/Content-Length:\s*(\d+)/);
    if (!lengthMatch) {
      throw new Error("missing LSP Content-Length");
    }
    const bodyStart = headerEnd + headerSeparator.length;
    const bodyEnd = bodyStart + Number(lengthMatch[1]);
    if (pending.length < bodyEnd) {
      return;
    }
    const message = JSON.parse(pending.subarray(bodyStart, bodyEnd).toString("utf8"));
    pending = pending.subarray(bodyEnd);
    handleMessage(message);
  }
}
child.stdout.on("data", chunk => {
  stdout.push(chunk);
  pending = Buffer.concat([pending, chunk]);
  parsePending();
});
child.stderr.on("data", chunk => stderr.push(chunk));
child.on("error", error => {
  clearTimeout(timer);
  process.stderr.write(String(error));
  process.exit(1);
});
child.on("close", code => {
  clearTimeout(timer);
  process.stdout.write(Buffer.concat(stdout));
  process.stderr.write(Buffer.concat(stderr));
  process.exit(code ?? 1);
});
send(config.messages.initialize);
"#;
pub(crate) const SERVER_MANIFEST_FROM_EXTENSION: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/seagrass/Cargo.toml"
);
pub(crate) const SECURITY_LEVEL_SETTINGS: &[&str] = &[
    "diagnostics.security.ownerChecks",
    "diagnostics.security.typeCosplay",
    "diagnostics.security.accountClosing",
    "diagnostics.security.initialization",
    "diagnostics.security.staleCpiReload",
    "diagnostics.security.signerAuthorization",
    "diagnostics.security.writableAccounts",
    "diagnostics.security.arbitraryCpi",
    "diagnostics.security.instructionDataBounds",
    "diagnostics.security.pdaSeedCollision",
];
