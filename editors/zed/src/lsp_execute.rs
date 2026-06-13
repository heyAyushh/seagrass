use {
    crate::{
        command::server_command_for_worktree,
        constants::{
            CONTENT_LENGTH_HEADER, EXECUTE_COMMAND_REQUEST_ID, HEADER_SEPARATOR,
            INITIALIZE_REQUEST_ID, JSON_RPC_VERSION, NODE_EVAL_FLAG, NODE_LSP_EXECUTE_SCRIPT,
            SHUTDOWN_REQUEST_ID, SUCCESS_EXIT_STATUS,
        },
        slash::SlashCommandSpec,
        uri::file_uri_from_path,
    },
    zed_extension_api as zed,
};

pub(crate) fn execute_lsp_command_for_worktree(
    spec: SlashCommandSpec,
    worktree: &zed::Worktree,
    arguments: Vec<zed::serde_json::Value>,
) -> Result<zed::serde_json::Value, String> {
    let server_command = server_command_for_worktree(worktree)?;
    let root_uri = file_uri_from_path(&worktree.root_path());
    let messages = lsp_execute_command_messages(&root_uri, spec.lsp_command, arguments);
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

fn lsp_execute_command_messages(
    root_uri: &str,
    lsp_command: &str,
    arguments: Vec<zed::serde_json::Value>,
) -> zed::serde_json::Value {
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
                "arguments": arguments
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

#[cfg(test)]
mod tests {
    use {super::*, crate::constants::SLASH_COVERAGE, crate::slash::slash_command_spec};

    #[test]
    fn slash_command_payload_dispatches_execute_command() {
        let messages = lsp_execute_command_messages(
            "file:///tmp/seagrass",
            slash_command_spec(SLASH_COVERAGE).unwrap().lsp_command,
            Vec::new(),
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
    fn slash_command_payload_includes_execute_arguments() {
        let messages = lsp_execute_command_messages(
            "file:///tmp/seagrass",
            "seagrass/analyze",
            vec![zed::serde_json::json!({
                "uri": "file:///tmp/seagrass/programs/demo/src/lib.rs",
                "instruction": "initialize",
            })],
        );
        let execute = messages["execute"].as_str().unwrap();

        assert!(execute.contains("\"command\":\"seagrass/analyze\""));
        assert!(execute.contains("\"uri\":\"file:///tmp/seagrass/programs/demo/src/lib.rs\""));
        assert!(execute.contains("\"instruction\":\"initialize\""));
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
}
