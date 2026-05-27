use zed_extension_api::{self as zed, settings::LspSettings};

const SERVER_ID: &str = "seagrass";
const SERVER_BINARY: &str = "seagrass";
const SERVER_MANIFEST_ENV: &str = "SEAGRASS_MANIFEST_PATH";
const DEFAULT_DIAGNOSTICS_TRANSPORT: &str = "push";
const SLASH_STATUS: &str = "seagrass-status";
const SLASH_COVERAGE: &str = "seagrass-coverage";
const SLASH_ARTIFACTS: &str = "seagrass-artifacts";
const SLASH_FEEDBACK: &str = "seagrass-feedback";
const START_SERVER_FIRST: &str = "Start the Seagrass server first.";
const NODE_EVAL_FLAG: &str = "-e";
const INITIALIZE_REQUEST_ID: u64 = 1;
const EXECUTE_COMMAND_REQUEST_ID: u64 = 2;
const SHUTDOWN_REQUEST_ID: u64 = 3;
const JSON_RPC_VERSION: &str = "2.0";
const CONTENT_LENGTH_HEADER: &str = "Content-Length:";
const HEADER_SEPARATOR: &[u8] = b"\r\n\r\n";
const SUCCESS_EXIT_STATUS: i32 = 0;
const NODE_LSP_EXECUTE_SCRIPT: &str = r#"
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
const SERVER_MANIFEST_FROM_EXTENSION: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
const SECURITY_LEVEL_SETTINGS: &[&str] = &[
    "diagnostics.security.ownerChecks",
    "diagnostics.security.typeCosplay",
    "diagnostics.security.accountClosing",
    "diagnostics.security.initialization",
    "diagnostics.security.staleCpiReload",
    "diagnostics.security.signerAuthorization",
    "diagnostics.security.arbitraryCpi",
    "diagnostics.security.instructionDataBounds",
    "diagnostics.security.pdaSeedCollision",
];

struct SeagrassExtension;

impl zed::Extension for SeagrassExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != SERVER_ID {
            return Err(format!(
                "unsupported language server `{language_server_id}`"
            ));
        }

        server_command_for_worktree(worktree)
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        if language_server_id.as_ref() != SERVER_ID {
            return Ok(None);
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        let transport = diagnostics_transport(settings.settings.as_ref());

        Ok(Some(zed::serde_json::json!({
            "seagrass": {
                "diagnostics": {
                    "transport": transport
                },
                "editor": {
                    "client": "zed",
                    "inlineValues": {
                        "enabled": true
                    }
                },
                "telemetry": {
                    "completion": {
                        "enabled": true
                    },
                    "diagnostics": {
                        "enabled": true
                    }
                }
            }
        })))
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        if language_server_id.as_ref() != SERVER_ID {
            return Ok(None);
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        Ok(Some(workspace_configuration(settings.settings)))
    }

    fn complete_slash_command_argument(
        &self,
        command: zed::SlashCommand,
        _args: Vec<String>,
    ) -> zed::Result<Vec<zed::SlashCommandArgumentCompletion>> {
        slash_command_spec(&command.name)
            .map(|_| Vec::new())
            .ok_or_else(|| unsupported_slash_command(&command.name))
    }

    fn run_slash_command(
        &self,
        command: zed::SlashCommand,
        _args: Vec<String>,
        worktree: Option<&zed::Worktree>,
    ) -> zed::Result<zed::SlashCommandOutput> {
        let spec = slash_command_spec(&command.name)
            .ok_or_else(|| unsupported_slash_command(&command.name))?;
        Ok(run_seagrass_slash_command(spec, worktree))
    }
}

fn server_command_for_worktree(worktree: &zed::Worktree) -> zed::Result<zed::Command> {
    let settings = LspSettings::for_worktree(SERVER_ID, worktree)?;
    if let Some(binary) = settings.binary {
        let command = binary
            .path
            .ok_or_else(|| "`lsp.seagrass.binary.path` must be set".to_string())?;
        let args = binary.arguments.unwrap_or_default();
        return Ok(zed::Command {
            command,
            args,
            env: merge_env(worktree.shell_env(), binary.env),
        });
    }

    let env = worktree.shell_env();
    if let Some(command) = worktree.which(SERVER_BINARY) {
        return Ok(zed::Command {
            command,
            args: Vec::new(),
            env,
        });
    }

    let cargo = worktree
        .which("cargo")
        .ok_or_else(|| "seagrass was not found on PATH, and cargo is not available".to_string())?;

    let worktree_manifest = worktree.read_text_file("Cargo.toml").ok();
    let args = cargo_args(
        &env,
        worktree_manifest.as_deref(),
        &worktree_manifest_path(worktree),
    );

    Ok(zed::Command {
        command: cargo,
        args,
        env,
    })
}

fn cargo_args_for_manifest(manifest_path: impl Into<String>) -> Vec<String> {
    vec![
        "run".to_string(),
        "--manifest-path".to_string(),
        manifest_path.into(),
        "--quiet".to_string(),
    ]
}

fn cargo_args(
    env: &[(String, String)],
    worktree_manifest: Option<&str>,
    worktree_manifest_path: &str,
) -> Vec<String> {
    if let Some(manifest_path) = env_value(env, SERVER_MANIFEST_ENV) {
        return cargo_args_for_manifest(manifest_path);
    }

    if worktree_manifest.is_some_and(is_seagrass_server_manifest) {
        return cargo_args_for_manifest(worktree_manifest_path);
    }

    cargo_args_for_manifest(SERVER_MANIFEST_FROM_EXTENSION)
}

fn worktree_manifest_path(worktree: &zed::Worktree) -> String {
    format!("{}/Cargo.toml", worktree.root_path())
}

fn is_seagrass_server_manifest(manifest: &str) -> bool {
    section_has_toml_string(manifest, "[package]", "name", SERVER_BINARY)
        && section_has_toml_string(manifest, "[[bin]]", "name", SERVER_BINARY)
}

fn section_has_toml_string(manifest: &str, section: &str, key: &str, expected: &str) -> bool {
    let mut in_section = false;

    for raw_line in manifest.lines() {
        let line = strip_toml_comment(raw_line).trim();
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if in_section && toml_string_value(line, key).as_deref() == Some(expected) {
            return true;
        }
    }

    false
}

fn toml_string_value(line: &str, key: &str) -> Option<String> {
    let value = line
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?
        .trim_start()
        .strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn strip_toml_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or("")
}

fn env_value(env: &[(String, String)], key: &str) -> Option<String> {
    env.iter()
        .find_map(|(name, value)| (name == key).then(|| value.clone()))
}

fn workspace_configuration(settings: Option<zed::serde_json::Value>) -> zed::serde_json::Value {
    let mut settings = settings.unwrap_or_else(|| zed::serde_json::json!({}));
    if !settings.is_object() {
        settings = zed::serde_json::json!({});
    }
    ensure_default(&mut settings, "agent.mode", false);
    if bool_setting(&settings, "agent.mode").unwrap_or(false) {
        apply_agent_mode_defaults(&mut settings);
    }
    ensure_default(&mut settings, "diagnostics.security.enabled", true);
    ensure_default(&mut settings, "diagnostics.experimental.enabled", true);
    ensure_default(&mut settings, "security.strictNative.enabled", true);
    ensure_default(&mut settings, "editor.inlineValues.enabled", true);
    ensure_default(&mut settings, "telemetry.completion.enabled", true);
    ensure_default(&mut settings, "telemetry.diagnostics.enabled", true);
    ensure_default(&mut settings, "inlayHints.enabled", true);
    ensure_default(&mut settings, "workspaceIndex.enabled", true);
    ensure_default(&mut settings, "trace.server", false);
    ensure_string_default(&mut settings, "editor.client", "zed");
    ensure_string_default(
        &mut settings,
        "diagnostics.transport",
        DEFAULT_DIAGNOSTICS_TRANSPORT,
    );
    ensure_string_default(&mut settings, "diagnostics.coldPath", "idle");
    settings
}

fn apply_agent_mode_defaults(settings: &mut zed::serde_json::Value) {
    ensure_default(settings, "diagnostics.security.enabled", true);
    ensure_default(settings, "diagnostics.experimental.enabled", true);
    ensure_default(settings, "security.strictNative.enabled", true);
    ensure_default(settings, "trace.server", true);
    ensure_string_default(settings, "diagnostics.coldPath", "idle");
    for setting in SECURITY_LEVEL_SETTINGS {
        ensure_string_default(settings, setting, "warn");
    }
}

fn diagnostics_transport(settings: Option<&zed::serde_json::Value>) -> String {
    let Some(settings) = settings else {
        return DEFAULT_DIAGNOSTICS_TRANSPORT.to_string();
    };

    string_setting(settings, "diagnostics.transport")
        .filter(|value| matches!(value.as_str(), "push" | "pull"))
        .unwrap_or_else(|| DEFAULT_DIAGNOSTICS_TRANSPORT.to_string())
}

fn bool_setting(settings: &zed::serde_json::Value, key: &str) -> Option<bool> {
    setting_value(settings, key).and_then(|value| value.as_bool())
}

fn string_setting(settings: &zed::serde_json::Value, key: &str) -> Option<String> {
    setting_value(settings, key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn setting_value<'a>(
    settings: &'a zed::serde_json::Value,
    key: &str,
) -> Option<&'a zed::serde_json::Value> {
    settings.get(key).or_else(|| {
        key.split('.')
            .try_fold(settings, |value, part| value.get(part))
    })
}

fn ensure_default(settings: &mut zed::serde_json::Value, key: &str, default: bool) {
    if setting_value(settings, key).is_none() {
        settings[key] = zed::serde_json::Value::Bool(default);
    }
}

fn ensure_string_default(settings: &mut zed::serde_json::Value, key: &str, default: &str) {
    if setting_value(settings, key).is_none() {
        settings[key] = zed::serde_json::Value::String(default.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SlashCommandSpec {
    name: &'static str,
    lsp_command: &'static str,
}

fn slash_command_spec(name: &str) -> Option<SlashCommandSpec> {
    match name {
        SLASH_STATUS => Some(SlashCommandSpec {
            name: SLASH_STATUS,
            lsp_command: "seagrass/status",
        }),
        SLASH_COVERAGE => Some(SlashCommandSpec {
            name: SLASH_COVERAGE,
            lsp_command: "seagrass/projectCoverage",
        }),
        SLASH_ARTIFACTS => Some(SlashCommandSpec {
            name: SLASH_ARTIFACTS,
            lsp_command: "seagrass/artifacts",
        }),
        SLASH_FEEDBACK => Some(SlashCommandSpec {
            name: SLASH_FEEDBACK,
            lsp_command: "seagrass/feedback",
        }),
        _ => None,
    }
}

fn run_seagrass_slash_command(
    spec: SlashCommandSpec,
    worktree: Option<&zed::Worktree>,
) -> zed::SlashCommandOutput {
    let Some(worktree) = worktree else {
        return slash_command_error_output(
            spec,
            "no worktree is attached to this Assistant request",
        );
    };

    match execute_lsp_command_for_worktree(spec, worktree) {
        Ok(result) => zed::SlashCommandOutput {
            text: slash_command_result_text(spec, &result),
            sections: Vec::new(),
        },
        Err(error) => slash_command_error_output(spec, &error),
    }
}

fn execute_lsp_command_for_worktree(
    spec: SlashCommandSpec,
    worktree: &zed::Worktree,
) -> Result<zed::serde_json::Value, String> {
    let server_command = server_command_for_worktree(worktree)?;
    let root_uri = file_uri_from_path(&worktree.root_path());
    let messages = lsp_execute_command_messages(&root_uri, spec.lsp_command);
    let config = zed::serde_json::json!({
        "command": server_command.command,
        "args": server_command.args,
        "env": server_command.env,
        "messages": messages,
        "initializeId": INITIALIZE_REQUEST_ID,
        "executeId": EXECUTE_COMMAND_REQUEST_ID,
        "shutdownId": SHUTDOWN_REQUEST_ID,
    });
    let mut command = zed::Command {
        command: zed::node_binary_path()?,
        args: vec![
            NODE_EVAL_FLAG.to_string(),
            NODE_LSP_EXECUTE_SCRIPT.to_string(),
            config.to_string(),
        ],
        env: Vec::new(),
    };
    let output = command.output()?;
    if output.status != Some(SUCCESS_EXIT_STATUS) {
        return Err(format!(
            "Seagrass command exited with {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_lsp_result(&output.stdout, EXECUTE_COMMAND_REQUEST_ID)
}

fn slash_command_result_text(spec: SlashCommandSpec, result: &zed::serde_json::Value) -> String {
    if spec.name == SLASH_FEEDBACK {
        if let Some(url) = result.get("url").and_then(|value| value.as_str()) {
            let label = result
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or("Seagrass feedback");
            return format!("{label}\n{url}");
        }
    }

    zed::serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
}

fn slash_command_error_output(spec: SlashCommandSpec, error: &str) -> zed::SlashCommandOutput {
    zed::SlashCommandOutput {
        text: format!(
            "{START_SERVER_FIRST}\n\n/{} could not dispatch `{}`: {error}",
            spec.name, spec.lsp_command
        ),
        sections: Vec::new(),
    }
}

fn lsp_execute_command_messages(root_uri: &str, lsp_command: &str) -> zed::serde_json::Value {
    zed::serde_json::json!({
        "initialize": lsp_message(&zed::serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": INITIALIZE_REQUEST_ID,
            "method": "initialize",
            "params": {
                "processId": zed::serde_json::Value::Null,
                "rootUri": root_uri,
                "capabilities": {},
                "workspaceFolders": [{
                    "uri": root_uri,
                    "name": "workspace"
                }]
            }
        })),
        "initialized": lsp_message(&zed::serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "method": "initialized",
            "params": {}
        })),
        "execute": lsp_message(&zed::serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": EXECUTE_COMMAND_REQUEST_ID,
            "method": "workspace/executeCommand",
            "params": {
                "command": lsp_command,
                "arguments": []
            }
        })),
        "shutdown": lsp_message(&zed::serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": SHUTDOWN_REQUEST_ID,
            "method": "shutdown",
            "params": zed::serde_json::Value::Null
        })),
        "exit": lsp_message(&zed::serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "method": "exit",
            "params": zed::serde_json::Value::Null
        })),
    })
}

fn lsp_message(message: &zed::serde_json::Value) -> String {
    let body = message.to_string();
    format!("{CONTENT_LENGTH_HEADER} {}\r\n\r\n{body}", body.len())
}

fn parse_lsp_result(stdout: &[u8], response_id: u64) -> Result<zed::serde_json::Value, String> {
    let mut cursor = 0usize;
    while let Some(header_offset) = find_bytes(&stdout[cursor..], CONTENT_LENGTH_HEADER.as_bytes())
    {
        let header_start = cursor + header_offset;
        let header_end = header_start
            + find_bytes(&stdout[header_start..], HEADER_SEPARATOR)
                .ok_or_else(|| "LSP response header is incomplete".to_string())?;
        let header = std::str::from_utf8(&stdout[header_start..header_end])
            .map_err(|_| "LSP response header is not UTF-8".to_string())?;
        let content_length = content_length_from_header(header)?;
        let body_start = header_end + HEADER_SEPARATOR.len();
        let body_end = body_start
            .checked_add(content_length)
            .ok_or_else(|| "LSP response body length overflowed".to_string())?;
        if body_end > stdout.len() {
            return Err("LSP response body is incomplete".to_string());
        }

        let message: zed::serde_json::Value =
            zed::serde_json::from_slice(&stdout[body_start..body_end])
                .map_err(|error| format!("LSP response body is invalid JSON: {error}"))?;
        if message.get("id").and_then(zed::serde_json::Value::as_u64) == Some(response_id) {
            if let Some(error) = message.get("error") {
                return Err(format!("LSP command failed: {error}"));
            }
            return message
                .get("result")
                .cloned()
                .ok_or_else(|| "LSP response did not include a result".to_string());
        }

        cursor = body_end;
    }

    Err(format!("LSP response id {response_id} was not returned"))
}

fn content_length_from_header(header: &str) -> Result<usize, String> {
    header
        .lines()
        .find_map(|line| line.strip_prefix(CONTENT_LENGTH_HEADER))
        .ok_or_else(|| "LSP response is missing Content-Length".to_string())?
        .trim()
        .parse::<usize>()
        .map_err(|_| "LSP response Content-Length is invalid".to_string())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn file_uri_from_path(path: &str) -> String {
    format!("file://{}", percent_encode_path(path))
}

fn percent_encode_path(path: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if is_file_uri_path_byte(byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

fn is_file_uri_path_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'/'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
    )
}

fn unsupported_slash_command(name: &str) -> String {
    format!("unsupported Seagrass slash command `{name}`")
}

fn merge_env(
    mut base: zed::EnvVars,
    overrides: Option<std::collections::HashMap<String, String>>,
) -> zed::EnvVars {
    if let Some(overrides) = overrides {
        for (name, value) in overrides {
            if let Some((_, existing)) = base.iter_mut().find(|(existing, _)| existing == &name) {
                *existing = value;
            } else {
                base.push((name, value));
            }
        }
    }

    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_push_diagnostics_to_avoid_transport_duplicates() {
        assert_eq!(diagnostics_transport(None), "push");

        let config = workspace_configuration(None);
        assert_eq!(config["diagnostics.transport"], "push");
    }

    #[test]
    fn agent_mode_fills_unset_workspace_settings() {
        let config = workspace_configuration(Some(zed::serde_json::json!({
            "agent.mode": true
        })));

        assert_eq!(config["diagnostics.security.enabled"], true);
        assert_eq!(config["diagnostics.experimental.enabled"], true);
        assert_eq!(config["security.strictNative.enabled"], true);
        assert_eq!(config["diagnostics.coldPath"], "idle");
        assert_eq!(config["trace.server"], true);
        assert_eq!(config["diagnostics.security.ownerChecks"], "warn");
        assert_eq!(config["diagnostics.security.pdaSeedCollision"], "warn");
    }

    #[test]
    fn agent_mode_preserves_explicit_workspace_settings() {
        let config = workspace_configuration(Some(zed::serde_json::json!({
            "agent": {
                "mode": true
            },
            "diagnostics": {
                "coldPath": "manual"
            },
            "trace": {
                "server": false
            },
            "diagnostics.security.ownerChecks": "error",
            "diagnostics.security.typeCosplay": "off"
        })));

        assert_eq!(
            string_setting(&config, "diagnostics.coldPath"),
            Some("manual".to_string())
        );
        assert_eq!(bool_setting(&config, "trace.server"), Some(false));
        assert_eq!(config["diagnostics.security.ownerChecks"], "error");
        assert_eq!(config["diagnostics.security.typeCosplay"], "off");
        assert_eq!(config["diagnostics.security.arbitraryCpi"], "warn");
    }

    #[test]
    fn diagnostics_transport_respects_flat_setting() {
        let settings = zed::serde_json::json!({
            "diagnostics.transport": "pull"
        });

        assert_eq!(diagnostics_transport(Some(&settings)), "pull");
    }

    #[test]
    fn diagnostics_transport_respects_nested_setting() {
        let settings = zed::serde_json::json!({
            "diagnostics": {
                "transport": "push"
            }
        });

        assert_eq!(diagnostics_transport(Some(&settings)), "push");
    }

    #[test]
    fn diagnostics_transport_rejects_unknown_values() {
        let settings = zed::serde_json::json!({
            "diagnostics.transport": "invalid"
        });

        assert_eq!(diagnostics_transport(Some(&settings)), "push");
    }

    #[test]
    fn diagnostics_transport_rejects_mixed_transport() {
        let settings = zed::serde_json::json!({
            "diagnostics.transport": "both"
        });

        assert_eq!(diagnostics_transport(Some(&settings)), "push");
    }

    #[test]
    fn cargo_args_use_manifest_env_override() {
        let env = vec![(
            SERVER_MANIFEST_ENV.to_string(),
            "/tmp/seagrass/Cargo.toml".to_string(),
        )];

        assert_eq!(
            cargo_args(
                &env,
                Some(GENERIC_WORKTREE_MANIFEST),
                "/tmp/workspace/Cargo.toml",
            ),
            cargo_args_for_manifest("/tmp/seagrass/Cargo.toml")
        );
    }

    #[test]
    fn cargo_args_use_worktree_manifest_only_for_seagrass_server() {
        assert_eq!(
            cargo_args(
                &[],
                Some(SEAGRASS_SERVER_MANIFEST),
                "/tmp/workspace/Cargo.toml",
            ),
            cargo_args_for_manifest("/tmp/workspace/Cargo.toml")
        );
    }

    #[test]
    fn cargo_args_skip_generic_worktree_manifest_without_bin() {
        assert_eq!(
            cargo_args(
                &[],
                Some(GENERIC_WORKTREE_MANIFEST),
                "/tmp/workspace/Cargo.toml",
            ),
            cargo_args_for_manifest(SERVER_MANIFEST_FROM_EXTENSION)
        );
    }

    #[test]
    fn slash_commands_map_to_server_execute_commands() {
        assert_eq!(
            slash_command_spec(SLASH_STATUS).map(|spec| spec.lsp_command),
            Some("seagrass/status")
        );
        assert_eq!(
            slash_command_spec(SLASH_COVERAGE).map(|spec| spec.lsp_command),
            Some("seagrass/projectCoverage")
        );
        assert_eq!(
            slash_command_spec(SLASH_ARTIFACTS).map(|spec| spec.lsp_command),
            Some("seagrass/artifacts")
        );
        assert_eq!(
            slash_command_spec(SLASH_FEEDBACK).map(|spec| spec.lsp_command),
            Some("seagrass/feedback")
        );
    }

    #[test]
    fn slash_command_error_output_keeps_graceful_start_hint() {
        let output = slash_command_error_output(
            slash_command_spec(SLASH_STATUS).unwrap(),
            "seagrass was unavailable",
        );

        assert!(output.text.contains(START_SERVER_FIRST));
        assert!(output.text.contains("seagrass was unavailable"));
        assert!(output.text.contains("seagrass/status"));
        assert!(output.sections.is_empty());
    }

    #[test]
    fn slash_command_payload_dispatches_execute_command() {
        let messages = lsp_execute_command_messages(
            "file:///tmp/seagrass",
            slash_command_spec(SLASH_COVERAGE).unwrap().lsp_command,
        );
        let initialize = messages["initialize"].as_str().unwrap();
        let execute = messages["execute"].as_str().unwrap();
        let shutdown = messages["shutdown"].as_str().unwrap();

        assert!(initialize.contains("\"method\":\"initialize\""));
        assert!(execute.contains("\"method\":\"workspace/executeCommand\""));
        assert!(execute.contains("\"command\":\"seagrass/projectCoverage\""));
        assert!(shutdown.contains("\"method\":\"shutdown\""));
    }

    #[test]
    fn parses_lsp_execute_command_result() {
        let init = lsp_message(&zed::serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": INITIALIZE_REQUEST_ID,
            "result": {}
        }));
        let notification = lsp_message(&zed::serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "method": "window/logMessage",
            "params": {"message": "ready"}
        }));
        let result = lsp_message(&zed::serde_json::json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": EXECUTE_COMMAND_REQUEST_ID,
            "result": {"ok": true}
        }));
        let stdout = format!("{init}{notification}{result}");

        assert_eq!(
            parse_lsp_result(stdout.as_bytes(), EXECUTE_COMMAND_REQUEST_ID).unwrap(),
            zed::serde_json::json!({"ok": true})
        );
    }

    #[test]
    fn slash_command_feedback_text_prefers_url() {
        let text = slash_command_result_text(
            slash_command_spec(SLASH_FEEDBACK).unwrap(),
            &zed::serde_json::json!({
                "label": "Join the Seagrass Telegram",
                "url": "https://t.me/example"
            }),
        );

        assert_eq!(text, "Join the Seagrass Telegram\nhttps://t.me/example");
    }

    #[test]
    fn file_uri_percent_encodes_spaces() {
        assert_eq!(
            file_uri_from_path("/tmp/seagrass project"),
            "file:///tmp/seagrass%20project"
        );
    }

    const GENERIC_WORKTREE_MANIFEST: &str = r#"
[package]
name = "some-program"
version = "0.1.0"
edition = "2021"
"#;

    const SEAGRASS_SERVER_MANIFEST: &str = r#"
[package]
name = "seagrass"
version = "1.0.2"
edition = "2021"

[[bin]]
name = "seagrass"
path = "src/main.rs"
"#;
}

zed::register_extension!(SeagrassExtension);
