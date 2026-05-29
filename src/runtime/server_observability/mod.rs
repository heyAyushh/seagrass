use {
    crate::server_types::{DiagnosticsTransport, ServerLogEntry, ServerSettings},
    std::{
        collections::VecDeque,
        time::{SystemTime, UNIX_EPOCH},
    },
    tower_lsp::lsp_types::{Position, Url},
};

pub(crate) fn status_text_from_parts(
    version: &str,
    roots: &str,
    indexed: usize,
    open: usize,
    diagnostics_transport: DiagnosticsTransport,
    settings: &ServerSettings,
) -> String {
    format!(
        "Seagrass {version}\nworkspace roots: {roots}\nindexed files: {indexed}\nopen documents: {open}\nsync: full\ndiagnostics: {}, coldPath={}\nfeatures: security={}, experimental={}, strictNative={}, workspaceIndex={}, trace={}, editor={}",
        diagnostics_transport.as_str(),
        settings.diagnostics_cold_path.as_str(),
        settings.security_diagnostics,
        settings.experimental_diagnostics,
        settings.strict_native_security,
        settings.workspace_index,
        settings.trace_server,
        settings.editor_context.client,
    )
}

pub(crate) fn recent_logs_snapshot_from_entries(
    logs: &VecDeque<ServerLogEntry>,
    limit: usize,
) -> serde_json::Value {
    serde_json::json!({
        "limit": limit,
        "entries": logs.iter().map(ServerLogEntry::to_json).collect::<Vec<_>>(),
    })
}

pub(crate) fn should_emit_client_log(event: &str, settings: &ServerSettings) -> bool {
    event != "documentAnalyzed" || settings.trace_server
}

pub(crate) fn navigation_log_data(
    method: &str,
    uri: &Url,
    position: Position,
    source: &str,
    result_count: usize,
) -> serde_json::Value {
    serde_json::json!({
        "method": method,
        "uri": uri,
        "line": position.line,
        "character": position.character,
        "source": source,
        "resultCount": result_count,
    })
}

pub(crate) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
