use {
    crate::{diagnostics, document::ParsedDocument},
    std::collections::BTreeMap,
};

const DEGRADED_PARSE_MESSAGE: &str =
    " Semantic diagnostics are paused until this Rust file parses again.";

#[allow(dead_code)] // Consumed by scripts/check-recognized-settings.ts as the settings canon.
pub const APPLIED_SETTING_KEYS: &[&str] = &[
    "agent.mode",
    "diagnostics.artifacts.enabled",
    "diagnostics.coldPath",
    "diagnostics.experimental.enabled",
    "diagnostics.security.accountClosing",
    "diagnostics.security.arbitraryCpi",
    "diagnostics.security.enabled",
    "diagnostics.security.initialization",
    "diagnostics.security.instructionDataBounds",
    "diagnostics.security.ownerChecks",
    "diagnostics.security.pdaSeedCollision",
    "diagnostics.security.signerAuthorization",
    "diagnostics.security.staleCpiReload",
    "diagnostics.security.strictNative.enabled",
    "diagnostics.security.typeCosplay",
    "diagnostics.security.writableAccounts",
    "editor.client",
    "editor.inlineValues.enabled",
    "feedback.url",
    "security.strictNative.enabled",
    "telemetry.completion.enabled",
    "telemetry.diagnostics.enabled",
    "trace.server",
    "workspaceIndex.enabled",
];
#[allow(dead_code)] // Consumed by scripts/check-recognized-settings.ts as the settings canon.
pub const INITIALIZATION_SETTING_KEYS: &[&str] = &["diagnostics.transport"];
#[allow(dead_code)] // Consumed by scripts/check-recognized-settings.ts as the settings canon.
pub const RECOGNIZED_SETTING_KEYS: &[&str] = &[
    "agent.mode",
    "diagnostics.artifacts.enabled",
    "diagnostics.coldPath",
    "diagnostics.experimental.enabled",
    "diagnostics.security.accountClosing",
    "diagnostics.security.arbitraryCpi",
    "diagnostics.security.enabled",
    "diagnostics.security.initialization",
    "diagnostics.security.instructionDataBounds",
    "diagnostics.security.ownerChecks",
    "diagnostics.security.pdaSeedCollision",
    "diagnostics.security.signerAuthorization",
    "diagnostics.security.staleCpiReload",
    "diagnostics.security.strictNative.enabled",
    "diagnostics.security.typeCosplay",
    "diagnostics.security.writableAccounts",
    "diagnostics.transport",
    "editor.client",
    "editor.inlineValues.enabled",
    "feedback.url",
    "security.strictNative.enabled",
    "telemetry.completion.enabled",
    "telemetry.diagnostics.enabled",
    "trace.server",
    "workspaceIndex.enabled",
];

const SECURITY_LEVEL_SETTINGS: &[(&str, &str)] = &[
    ("diagnostics.security.ownerChecks", "security.ownerChecks"),
    ("diagnostics.security.typeCosplay", "security.typeCosplay"),
    (
        "diagnostics.security.accountClosing",
        "security.accountClosing",
    ),
    (
        "diagnostics.security.initialization",
        "security.initialization",
    ),
    (
        "diagnostics.security.staleCpiReload",
        "security.staleCpiReload",
    ),
    (
        "diagnostics.security.signerAuthorization",
        "security.signerAuthorization",
    ),
    (
        "diagnostics.security.writableAccounts",
        "security.writableAccounts",
    ),
    ("diagnostics.security.arbitraryCpi", "security.arbitraryCpi"),
    (
        "diagnostics.security.instructionDataBounds",
        "security.instructionDataBounds",
    ),
    (
        "diagnostics.security.pdaSeedCollision",
        "security.pdaSeedCollision",
    ),
];

#[derive(Debug, Clone)]
pub(crate) struct OpenDocument {
    pub(crate) text: String,
    pub(crate) version: Option<i32>,
    pub(crate) syntax_diagnostic: Option<tower_lsp::lsp_types::Diagnostic>,
}

impl OpenDocument {
    pub(crate) fn new(text: String, version: Option<i32>) -> Self {
        Self {
            text,
            version,
            syntax_diagnostic: None,
        }
    }

    pub(crate) fn with_syntax_diagnostic(
        text: String,
        version: Option<i32>,
        diagnostic: tower_lsp::lsp_types::Diagnostic,
    ) -> Self {
        Self {
            text,
            version,
            syntax_diagnostic: Some(diagnostic),
        }
    }
}

pub(crate) struct ParsedOpenDocument {
    pub(crate) open: OpenDocument,
    pub(crate) parsed: ParsedDocument,
}

impl ParsedOpenDocument {
    pub(crate) fn new(text: String, version: Option<i32>) -> Self {
        match ParsedDocument::parse(text.clone()) {
            Ok(parsed) => Self {
                open: OpenDocument::new(text, version),
                parsed,
            },
            Err(err) => {
                let mut diagnostic =
                    diagnostics::diagnostic_from_parse_error_with_source(err, &text);
                diagnostic.message.push_str(DEGRADED_PARSE_MESSAGE);
                Self {
                    parsed: ParsedDocument::parse_or_empty(text.clone()),
                    open: OpenDocument::with_syntax_diagnostic(text, version, diagnostic),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parsed_open_document_places_semicolon_error_on_unterminated_statement() {
        let source = r#"pub fn handler() -> Result<()> {
    position_bundle

    Ok(())
}
"#
        .to_string();
        let document = ParsedOpenDocument::new(source, Some(1));
        let diagnostic = document
            .open
            .syntax_diagnostic
            .expect("expected syntax diagnostic");

        assert_eq!(diagnostic.range.start.line, 1);
        assert_eq!(diagnostic.range.start.character, 4);
        assert!(diagnostic
            .message
            .contains("Semantic diagnostics are paused"));
    }

    #[test]
    fn recognized_keys_are_sorted_and_unique() {
        assert_sorted_unique(APPLIED_SETTING_KEYS);
        assert_sorted_unique(INITIALIZATION_SETTING_KEYS);
        assert_sorted_unique(RECOGNIZED_SETTING_KEYS);

        let mut combined = APPLIED_SETTING_KEYS
            .iter()
            .chain(INITIALIZATION_SETTING_KEYS)
            .copied()
            .collect::<Vec<_>>();
        combined.sort_unstable();
        assert_eq!(
            combined, RECOGNIZED_SETTING_KEYS,
            "recognized settings must be exactly applied plus initialization-only keys"
        );
    }

    #[test]
    fn apply_consumes_every_recognized_key() {
        for key in APPLIED_SETTING_KEYS {
            let mut settings = ServerSettings::default();
            settings.apply(settings_payload(key, non_default_setting_value(key)));

            assert_ne!(
                settings,
                ServerSettings::default(),
                "ServerSettings::apply ignored recognized setting `{key}`"
            );
        }
    }

    fn assert_sorted_unique(values: &[&str]) {
        for pair in values.windows(2) {
            assert!(
                pair[0] < pair[1],
                "setting keys must be sorted and unique: {values:?}"
            );
        }
    }

    fn settings_payload(key: &str, value: serde_json::Value) -> serde_json::Value {
        let mut seagrass = serde_json::Map::new();
        seagrass.insert(key.to_string(), value);
        let mut root = serde_json::Map::new();
        root.insert("seagrass".to_string(), serde_json::Value::Object(seagrass));
        serde_json::Value::Object(root)
    }

    fn non_default_setting_value(key: &str) -> serde_json::Value {
        match key {
            "agent.mode" => json!(true),
            "diagnostics.artifacts.enabled" => json!(true),
            "diagnostics.coldPath" => json!("save"),
            "diagnostics.experimental.enabled" => json!(false),
            "diagnostics.security.enabled" => json!(false),
            "diagnostics.security.strictNative.enabled" => json!(false),
            "editor.client" => json!("zed"),
            "editor.inlineValues.enabled" => json!(false),
            "feedback.url" => json!("https://example.test/seagrass"),
            "security.strictNative.enabled" => json!(false),
            "telemetry.completion.enabled" => json!(false),
            "telemetry.diagnostics.enabled" => json!(false),
            "trace.server" => json!(true),
            "workspaceIndex.enabled" => json!(false),
            setting if setting.starts_with("diagnostics.security.") => json!("error"),
            _ => panic!("missing non-default test value for `{key}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerSettings {
    pub(crate) agent_mode: bool,
    pub(crate) security_diagnostics: bool,
    pub(crate) experimental_diagnostics: bool,
    /// Controls mtime-based artifact diagnostics (stale/missing SBPF ELF, IDL, TypeScript).
    /// Off by default to avoid noise on every edit-before-rebuild cycle.
    pub(crate) artifact_diagnostics: bool,
    pub(crate) security_levels: BTreeMap<String, diagnostics::DiagnosticLevel>,
    pub(crate) strict_native_security: bool,
    pub(crate) diagnostics_cold_path: DiagnosticsColdPath,
    pub(crate) workspace_index: bool,
    pub(crate) trace_server: bool,
    pub(crate) feedback_url: Option<String>,
    pub(crate) editor_context: EditorContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorContext {
    pub(crate) client: String,
    pub(crate) inline_values: bool,
    pub(crate) completion_telemetry: bool,
    pub(crate) diagnostic_telemetry: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ServerLogEntry {
    pub(crate) unix_ms: u64,
    pub(crate) level: &'static str,
    pub(crate) event: &'static str,
    pub(crate) message: String,
    pub(crate) data: serde_json::Value,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            agent_mode: false,
            security_diagnostics: true,
            experimental_diagnostics: true,
            artifact_diagnostics: false,
            security_levels: BTreeMap::new(),
            strict_native_security: true,
            diagnostics_cold_path: DiagnosticsColdPath::Idle,
            workspace_index: true,
            trace_server: false,
            feedback_url: None,
            editor_context: EditorContext::default(),
        }
    }
}

impl ServerSettings {
    pub(crate) fn apply(&mut self, settings: serde_json::Value) {
        let anchor = settings.get("seagrass").unwrap_or(&settings);
        let agent_mode = bool_setting(anchor, "agent.mode").unwrap_or(false);
        self.agent_mode = agent_mode;
        if agent_mode {
            self.apply_agent_mode_defaults(anchor);
        }
        self.editor_context.apply(anchor);
        self.security_diagnostics = bool_setting(anchor, "diagnostics.security.enabled")
            .unwrap_or(self.security_diagnostics);
        self.experimental_diagnostics = bool_setting(anchor, "diagnostics.experimental.enabled")
            .unwrap_or(self.experimental_diagnostics);
        self.artifact_diagnostics = bool_setting(anchor, "diagnostics.artifacts.enabled")
            .unwrap_or(self.artifact_diagnostics);
        self.strict_native_security = bool_setting(anchor, "security.strictNative.enabled")
            .or_else(|| bool_setting(anchor, "diagnostics.security.strictNative.enabled"))
            .unwrap_or(self.strict_native_security);
        self.security_levels = security_levels(anchor, agent_mode);
        self.diagnostics_cold_path = string_setting(anchor, "diagnostics.coldPath")
            .and_then(DiagnosticsColdPath::from_str)
            .unwrap_or(self.diagnostics_cold_path);
        self.workspace_index =
            bool_setting(anchor, "workspaceIndex.enabled").unwrap_or(self.workspace_index);
        self.trace_server = bool_setting(anchor, "trace.server").unwrap_or(self.trace_server);
        if setting_value(anchor, "feedback.url").is_some() {
            self.feedback_url = non_empty_string_setting(anchor, "feedback.url");
        }
    }

    fn apply_agent_mode_defaults(&mut self, settings: &serde_json::Value) {
        if setting_value(settings, "diagnostics.security.enabled").is_none() {
            self.security_diagnostics = true;
        }
        if setting_value(settings, "diagnostics.experimental.enabled").is_none() {
            self.experimental_diagnostics = true;
        }
        if setting_value(settings, "security.strictNative.enabled")
            .or_else(|| setting_value(settings, "diagnostics.security.strictNative.enabled"))
            .is_none()
        {
            self.strict_native_security = true;
        }
        if setting_value(settings, "diagnostics.coldPath").is_none() {
            self.diagnostics_cold_path = DiagnosticsColdPath::Idle;
        }
        if setting_value(settings, "trace.server").is_none() {
            self.trace_server = true;
        }
    }
}

impl Default for EditorContext {
    fn default() -> Self {
        Self {
            client: "generic".to_string(),
            inline_values: true,
            completion_telemetry: true,
            diagnostic_telemetry: true,
        }
    }
}

impl EditorContext {
    fn apply(&mut self, settings: &serde_json::Value) {
        self.client =
            string_setting(settings, "editor.client").unwrap_or_else(|| self.client.clone());
        self.inline_values =
            bool_setting(settings, "editor.inlineValues.enabled").unwrap_or(self.inline_values);
        self.completion_telemetry = bool_setting(settings, "telemetry.completion.enabled")
            .unwrap_or(self.completion_telemetry);
        self.diagnostic_telemetry = bool_setting(settings, "telemetry.diagnostics.enabled")
            .unwrap_or(self.diagnostic_telemetry);
    }
}

impl ServerLogEntry {
    pub(crate) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "unixMs": self.unix_ms,
            "level": self.level,
            "event": self.event,
            "message": self.message,
            "data": self.data,
        })
    }
}

fn bool_setting(settings: &serde_json::Value, key: &str) -> Option<bool> {
    setting_value(settings, key).and_then(|value| value.as_bool())
}

fn string_setting(settings: &serde_json::Value, key: &str) -> Option<String> {
    setting_value(settings, key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn non_empty_string_setting(settings: &serde_json::Value, key: &str) -> Option<String> {
    string_setting(settings, key).and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn setting_value<'a>(settings: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    settings.get(key).or_else(|| {
        key.split('.')
            .try_fold(settings, |value, part| value.get(part))
    })
}

fn security_levels(
    settings: &serde_json::Value,
    agent_mode: bool,
) -> BTreeMap<String, diagnostics::DiagnosticLevel> {
    let mut levels = if agent_mode {
        agent_mode_security_levels(settings)
    } else {
        BTreeMap::new()
    };
    for (setting, config_key) in SECURITY_LEVEL_SETTINGS {
        if let Some(level) = string_setting(settings, setting)
            .and_then(|value| diagnostics::DiagnosticLevel::from_str(&value))
        {
            levels.insert((*config_key).to_string(), level);
        }
    }
    levels
}

fn agent_mode_security_levels(
    settings: &serde_json::Value,
) -> BTreeMap<String, diagnostics::DiagnosticLevel> {
    let mut levels = BTreeMap::new();
    for (setting, config_key) in SECURITY_LEVEL_SETTINGS {
        if setting_value(settings, setting).is_none() {
            levels.insert(
                (*config_key).to_string(),
                diagnostics::DiagnosticLevel::Warn,
            );
        }
    }
    levels
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticsColdPath {
    Idle,
    Save,
    Manual,
}

impl DiagnosticsColdPath {
    pub(crate) fn from_str(value: String) -> Option<Self> {
        match value.as_str() {
            "idle" => Some(Self::Idle),
            "save" => Some(Self::Save),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Save => "save",
            Self::Manual => "manual",
        }
    }

    pub(crate) fn runs_after_open(self) -> bool {
        !matches!(self, Self::Manual)
    }

    pub(crate) fn runs_after_change(self) -> bool {
        matches!(self, Self::Idle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticsTransport {
    Push,
    Pull,
}

impl DiagnosticsTransport {
    pub(crate) fn publishes(self) -> bool {
        matches!(self, Self::Push)
    }

    pub(crate) fn advertises_pull(self) -> bool {
        matches!(self, Self::Pull)
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Pull => "pull",
        }
    }
}
