use {
    crate::{diagnostics, document::ParsedDocument},
    std::collections::BTreeMap,
};

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
            Err(err) => Self {
                parsed: ParsedDocument::parse_or_empty(text.clone()),
                open: OpenDocument::with_syntax_diagnostic(
                    text,
                    version,
                    diagnostics::diagnostic_from_parse_error(err),
                ),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ServerSettings {
    pub(crate) security_diagnostics: bool,
    pub(crate) experimental_diagnostics: bool,
    pub(crate) security_levels: BTreeMap<String, diagnostics::DiagnosticLevel>,
    pub(crate) strict_native_security: bool,
    pub(crate) diagnostics_cold_path: DiagnosticsColdPath,
    pub(crate) workspace_index: bool,
    pub(crate) trace_server: bool,
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
            security_diagnostics: true,
            experimental_diagnostics: true,
            security_levels: BTreeMap::new(),
            strict_native_security: true,
            diagnostics_cold_path: DiagnosticsColdPath::Idle,
            workspace_index: true,
            trace_server: false,
            editor_context: EditorContext::default(),
        }
    }
}

impl ServerSettings {
    pub(crate) fn apply(&mut self, settings: serde_json::Value) {
        let anchor = settings.get("seagrass").unwrap_or(&settings);
        self.editor_context.apply(anchor);
        self.security_diagnostics = bool_setting(anchor, "diagnostics.security.enabled")
            .unwrap_or(self.security_diagnostics);
        self.experimental_diagnostics = bool_setting(anchor, "diagnostics.experimental.enabled")
            .unwrap_or(self.experimental_diagnostics);
        self.strict_native_security = bool_setting(anchor, "security.strictNative.enabled")
            .or_else(|| bool_setting(anchor, "diagnostics.security.strictNative.enabled"))
            .unwrap_or(self.strict_native_security);
        self.security_levels = security_levels(anchor);
        self.diagnostics_cold_path = string_setting(anchor, "diagnostics.coldPath")
            .and_then(DiagnosticsColdPath::from_str)
            .unwrap_or(self.diagnostics_cold_path);
        self.workspace_index =
            bool_setting(anchor, "workspaceIndex.enabled").unwrap_or(self.workspace_index);
        self.trace_server = bool_setting(anchor, "trace.server").unwrap_or(self.trace_server);
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
    settings
        .get(key)
        .and_then(|value| value.as_bool())
        .or_else(|| {
            key.split('.')
                .try_fold(settings, |value, part| value.get(part))
                .and_then(|value| value.as_bool())
        })
}

fn string_setting(settings: &serde_json::Value, key: &str) -> Option<String> {
    settings
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| {
            key.split('.')
                .try_fold(settings, |value, part| value.get(part))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
}

fn security_levels(settings: &serde_json::Value) -> BTreeMap<String, diagnostics::DiagnosticLevel> {
    let mut levels = BTreeMap::new();
    for (setting, config_key) in [
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
        ("diagnostics.security.arbitraryCpi", "security.arbitraryCpi"),
        (
            "diagnostics.security.instructionDataBounds",
            "security.instructionDataBounds",
        ),
        (
            "diagnostics.security.pdaSeedCollision",
            "security.pdaSeedCollision",
        ),
    ] {
        if let Some(level) = string_setting(settings, setting)
            .and_then(|value| diagnostics::DiagnosticLevel::from_str(&value))
        {
            levels.insert(config_key.to_string(), level);
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
    Both,
}

impl DiagnosticsTransport {
    pub(crate) fn publishes(self) -> bool {
        matches!(self, Self::Push | Self::Both)
    }

    pub(crate) fn advertises_pull(self) -> bool {
        matches!(self, Self::Pull | Self::Both)
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Pull => "pull",
            Self::Both => "both",
        }
    }
}
