use super::*;

const CHANGE_RANGE_OUTSIDE_DOCUMENT: &str = "rangeOutsideDocument";
const CHANGE_RANGE_START_AFTER_END: &str = "rangeStartAfterEnd";
const CHANGE_RANGE_NON_BOUNDARY: &str = "rangeNonBoundary";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ContentChangeError {
    range: tower_lsp::lsp_types::Range,
    reason: &'static str,
}

impl ContentChangeError {
    fn new(range: tower_lsp::lsp_types::Range, reason: &'static str) -> Self {
        Self { range, reason }
    }

    pub(super) fn range(&self) -> tower_lsp::lsp_types::Range {
        self.range
    }

    pub(super) fn reason(&self) -> &'static str {
        self.reason
    }
}

pub(super) fn text_after_content_changes(
    previous_text: Option<&str>,
    changes: Vec<tower_lsp::lsp_types::TextDocumentContentChangeEvent>,
) -> std::result::Result<(String, Option<tower_lsp::lsp_types::Range>), ContentChangeError> {
    let mut text = previous_text.unwrap_or_default().to_string();
    let mut latest_range = None;

    for change in changes {
        latest_range = change.range;
        apply_content_change(&mut text, change)?;
    }

    Ok((text, latest_range))
}

fn apply_content_change(
    text: &mut String,
    change: tower_lsp::lsp_types::TextDocumentContentChangeEvent,
) -> std::result::Result<(), ContentChangeError> {
    let Some(range) = change.range else {
        *text = change.text;
        return Ok(());
    };
    let start = crate::range::byte_offset_at(text, range.start)
        .ok_or_else(|| ContentChangeError::new(range, CHANGE_RANGE_OUTSIDE_DOCUMENT))?;
    let end = crate::range::byte_offset_at(text, range.end)
        .ok_or_else(|| ContentChangeError::new(range, CHANGE_RANGE_OUTSIDE_DOCUMENT))?;
    if start > end {
        return Err(ContentChangeError::new(range, CHANGE_RANGE_START_AFTER_END));
    }
    if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return Err(ContentChangeError::new(range, CHANGE_RANGE_NON_BOUNDARY));
    }
    text.replace_range(start..end, &change.text);
    Ok(())
}

pub(super) fn invalid_content_change_log_data(
    uri: &tower_lsp::lsp_types::Url,
    version: i32,
    error: &ContentChangeError,
) -> serde_json::Value {
    serde_json::json!({
        "uri": uri,
        "version": version,
        "range": error.range(),
        "reason": error.reason(),
        "documentCache": "cleared",
    })
}

pub(super) fn workspace_roots_changed_log_data(
    workspace_roots: &str,
    added_count: usize,
    removed_count: usize,
    republished_open_documents: usize,
) -> serde_json::Value {
    serde_json::json!({
        "workspaceRoots": workspace_roots,
        "added": added_count,
        "removed": removed_count,
        "republishedOpenDocuments": republished_open_documents,
    })
}

pub(super) fn watched_files_changed_log_data(
    changed_file_count: usize,
    republished_open_documents: usize,
) -> serde_json::Value {
    serde_json::json!({
        "changedFiles": changed_file_count,
        "republishedOpenDocuments": republished_open_documents,
    })
}

pub(super) fn requires_full_workspace_refresh(change: &tower_lsp::lsp_types::FileEvent) -> bool {
    change
        .uri
        .to_file_path()
        .ok()
        .and_then(|path| path.extension().map(|extension| extension == "rs"))
        != Some(true)
}

pub(super) fn changed_range_between_texts(
    previous: &str,
    current: &str,
) -> Option<tower_lsp::lsp_types::Range> {
    if previous == current {
        return None;
    }

    let prefix = common_prefix_byte_len(previous, current);
    let suffix = common_suffix_byte_len(&previous[prefix..], &current[prefix..]);
    let end_offset = current.len().saturating_sub(suffix);

    Some(tower_lsp::lsp_types::Range {
        start: position_at_byte_offset(current, prefix),
        end: position_at_byte_offset(current, end_offset),
    })
}

pub(super) fn common_prefix_byte_len(left: &str, right: &str) -> usize {
    let mut prefix = 0usize;
    for ((left_index, left_char), (_, right_char)) in left.char_indices().zip(right.char_indices())
    {
        if left_char != right_char {
            break;
        }
        prefix = left_index + left_char.len_utf8();
    }
    prefix
}

pub(super) fn common_suffix_byte_len(left_tail: &str, right_tail: &str) -> usize {
    let mut suffix = 0usize;
    for ((_, left_char), (_, right_char)) in left_tail
        .char_indices()
        .rev()
        .zip(right_tail.char_indices().rev())
    {
        if left_char != right_char {
            break;
        }
        suffix += left_char.len_utf8();
    }
    suffix
}

pub(super) fn position_at_byte_offset(
    source: &str,
    offset: usize,
) -> tower_lsp::lsp_types::Position {
    let offset = offset.min(source.len());
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = index + ch.len_utf8();
        }
    }
    tower_lsp::lsp_types::Position {
        line,
        character: u32::try_from(source[line_start..offset].chars().count()).unwrap_or_default(),
    }
}

pub(super) fn ranges_touch(
    left: tower_lsp::lsp_types::Range,
    right: tower_lsp::lsp_types::Range,
) -> bool {
    position_le(left.start, right.end) && position_le(right.start, left.end)
}

pub(super) fn contains_position(
    range: tower_lsp::lsp_types::Range,
    position: tower_lsp::lsp_types::Position,
) -> bool {
    position_le(range.start, position) && position_le(position, range.end)
}

pub(super) fn position_le(
    left: tower_lsp::lsp_types::Position,
    right: tower_lsp::lsp_types::Position,
) -> bool {
    left.line < right.line || (left.line == right.line && left.character <= right.character)
}

pub(super) fn analysis_uri_from_args(arguments: &[serde_json::Value]) -> Option<Url> {
    let first = arguments.first()?;
    if let Some(uri) = first.as_str() {
        return Url::parse(uri).ok();
    }
    first
        .get("uri")
        .and_then(|uri| uri.as_str())
        .and_then(|uri| Url::parse(uri).ok())
}

pub(super) fn analysis_instruction_from_args(arguments: &[serde_json::Value]) -> Option<&str> {
    arguments
        .first()
        .and_then(|argument| argument.get("instruction"))
        .or_else(|| {
            arguments
                .first()
                .and_then(|argument| argument.get("function"))
        })
        .and_then(|instruction| instruction.as_str())
}

pub(super) fn analysis_context_from_args(arguments: &[serde_json::Value]) -> Option<&str> {
    arguments
        .first()
        .and_then(|argument| argument.get("context"))
        .and_then(|context| context.as_str())
}

pub(super) fn diagnostic_summary(
    diagnostic: &tower_lsp::lsp_types::Diagnostic,
) -> serde_json::Value {
    let code = diagnostic.code.as_ref().map(|code| match code {
        tower_lsp::lsp_types::NumberOrString::Number(number) => number.to_string(),
        tower_lsp::lsp_types::NumberOrString::String(text) => text.clone(),
    });
    serde_json::json!({
        "code": code,
        "message": diagnostic.message,
        "range": diagnostic.range,
        "severity": diagnostic.severity.map(|severity| format!("{severity:?}")),
        "data": diagnostic.data,
    })
}

pub(super) fn diagnostic_code_counts(
    diagnostics: &[tower_lsp::lsp_types::Diagnostic],
) -> serde_json::Value {
    serde_json::json!(diagnostic_code_count_map(diagnostics))
}

pub(super) fn diagnostic_code_count_map(
    diagnostics: &[tower_lsp::lsp_types::Diagnostic],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for diagnostic in diagnostics {
        let code = diagnostic
            .code
            .as_ref()
            .map(|code| match code {
                tower_lsp::lsp_types::NumberOrString::Number(number) => number.to_string(),
                tower_lsp::lsp_types::NumberOrString::String(text) => text.clone(),
            })
            .unwrap_or_else(|| "uncoded".to_string());
        *counts.entry(code).or_default() += 1;
    }
    counts
}

pub(super) fn annotate_diagnostic_lane(
    mut diagnostics: Vec<tower_lsp::lsp_types::Diagnostic>,
    lane: DiagnosticPublishLane,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    for diagnostic in &mut diagnostics {
        let mut data = match diagnostic.data.take() {
            Some(serde_json::Value::Object(object)) => object,
            Some(value) => {
                let mut object = serde_json::Map::new();
                object.insert("value".to_string(), value);
                object
            }
            None => serde_json::Map::new(),
        };
        data.insert(
            "diagnosticLane".to_string(),
            serde_json::Value::String(lane.lane().to_string()),
        );
        data.insert(
            "diagnosticOrigin".to_string(),
            serde_json::Value::String(lane.origin().to_string()),
        );
        diagnostic.data = Some(serde_json::Value::Object(data));
    }
    diagnostics
}

pub(super) fn is_stale_diagnostic_publish(
    current_version: i32,
    publish_version: Option<i32>,
) -> bool {
    publish_version.is_some_and(|publish_version| current_version > publish_version)
}

pub(super) fn diagnostic_settings(settings: &ServerSettings) -> diagnostics::DiagnosticSettings {
    diagnostics::DiagnosticSettings {
        security_diagnostics: settings.security_diagnostics,
        experimental_diagnostics: settings.experimental_diagnostics,
        artifact_diagnostics: settings.artifact_diagnostics,
        security_levels: settings.security_levels.clone(),
        strict_native_security: settings.strict_native_security,
        typing_suppression: None,
    }
}

pub(super) fn diagnostic_settings_with_typing_suppression(
    settings: &ServerSettings,
    typing_suppression: Option<diagnostics::TypingSuppressionRegion>,
) -> diagnostics::DiagnosticSettings {
    let mut diagnostics = diagnostic_settings(settings);
    diagnostics.typing_suppression = typing_suppression;
    diagnostics
}

pub(super) fn hot_diagnostics_for_document(
    uri: &Url,
    document: &ParsedDocument,
    open_document: &OpenDocument,
    settings: &ServerSettings,
    workspace_index: &workspace::WorkspaceIndex,
    workspace_roots: &[Url],
    typing_suppression: Option<diagnostics::TypingSuppressionRegion>,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let seagrass_toml = project::nearest_seagrass_toml_with_roots(uri, workspace_roots);
    let mut diagnostics = syntax_diagnostics_for_document(
        document,
        open_document,
        seagrass_toml
            .as_ref()
            .map(|(_, seagrass_toml_text)| seagrass_toml_text.as_str()),
    );
    diagnostics.extend(diagnostics::collect_hot_with_input(
        diagnostics::DiagnosticInput {
            document,
            uri: Some(uri),
            workspace_index: Some(workspace_index),
            framework: crate::solana::frameworks::FrameworkContext::from_document(document),
            manifest: None,
            workspace_manifest: None,
            anchor_toml: None,
            seagrass_toml: seagrass_toml
                .as_ref()
                .map(|(seagrass_toml_uri, seagrass_toml_text)| {
                    (seagrass_toml_uri, seagrass_toml_text.as_str())
                }),
            solana_program: None,
            settings: diagnostic_settings_with_typing_suppression(settings, typing_suppression),
        },
    ));
    diagnostics::dedupe(diagnostics)
}

pub(super) fn syntax_diagnostics_for_document(
    document: &ParsedDocument,
    open_document: &OpenDocument,
    seagrass_toml: Option<&str>,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    // single choke point for open-document syntax diagnostics: every publish or
    // pull path must route parse-pause diagnostics through suppression here.
    diagnostics::suppression::filter(
        document,
        seagrass_toml,
        open_document
            .syntax_diagnostic
            .clone()
            .into_iter()
            .collect::<Vec<_>>(),
    )
}

pub(super) fn server_capabilities(
    diagnostics_transport: DiagnosticsTransport,
) -> ServerCapabilities {
    let mut capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                will_save: Some(false),
                will_save_wait_until: Some(false),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(false),
                })),
            },
        )),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_link_provider: Some(DocumentLinkOptions {
            resolve_provider: Some(false),
            work_done_progress_options: WorkDoneProgressOptions {
                work_done_progress: None,
            },
        }),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        declaration_provider: Some(DeclarationCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: WorkDoneProgressOptions {
                work_done_progress: None,
            },
        })),
        document_highlight_provider: Some(OneOf::Left(true)),
        type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string(), "=".to_string()]),
            retrigger_characters: Some(vec![",".to_string(), "=".to_string()]),
            ..SignatureHelpOptions::default()
        }),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![
                CodeActionKind::QUICKFIX,
                CodeActionKind::REFACTOR,
                CodeActionKind::SOURCE,
            ]),
            resolve_provider: Some(true),
            ..CodeActionOptions::default()
        })),
        code_lens_provider: Some(CodeLensOptions {
            resolve_provider: Some(false),
        }),
        inlay_hint_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(true),
            trigger_characters: Some(completion_trigger_characters()),
            ..CompletionOptions::default()
        }),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            semantic_tokens::options(),
        )),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: vec![
                STATUS_COMMAND.to_string(),
                ANALYZE_COMMAND.to_string(),
                ARTIFACTS_COMMAND.to_string(),
                PROPOSE_ASSISTS_COMMAND.to_string(),
                SEAGRASS_INSTRUCTION_SUMMARY_COMMAND.to_string(),
                SEAGRASS_PROGRAM_REPORT_COMMAND.to_string(),
                ERROR_COVERAGE_COMMAND.to_string(),
                SUPPORT_MATRIX_COMMAND.to_string(),
                GENERATOR_PROFILE_COMMAND.to_string(),
                LOGS_COMMAND.to_string(),
                PROJECT_COVERAGE_COMMAND.to_string(),
                FEEDBACK_COMMAND.to_string(),
            ],
            ..ExecuteCommandOptions::default()
        }),
        workspace: Some(WorkspaceServerCapabilities {
            workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(true)),
            }),
            ..WorkspaceServerCapabilities::default()
        }),
        ..ServerCapabilities::default()
    };

    if diagnostics_transport.advertises_pull() {
        capabilities.diagnostic_provider =
            Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                identifier: Some("seagrass".to_string()),
                inter_file_dependencies: true,
                workspace_diagnostics: false,
                ..DiagnosticOptions::default()
            }));
    }

    capabilities
}

pub(super) fn completion_trigger_characters() -> Vec<String> {
    IDENTIFIER_COMPLETION_TRIGGER_CHARS
        .chars()
        .map(|ch| ch.to_string())
        .collect()
}

pub(super) fn code_action_uri(action: &CodeAction) -> Option<Url> {
    action
        .data
        .as_ref()
        .and_then(|data| data.get("uri"))
        .and_then(|value| value.as_str())
        .and_then(|value| Url::parse(value).ok())
}

pub(super) fn diagnostics_transport_from_initialize_options(
    initialization_options: Option<&serde_json::Value>,
) -> DiagnosticsTransport {
    let Some(options) = initialization_options else {
        return DiagnosticsTransport::Push;
    };
    let anchor = options.get("seagrass").unwrap_or(options);
    let transport = anchor
        .get("diagnostics.transport")
        .and_then(|value| value.as_str())
        .or_else(|| {
            anchor
                .get("diagnostics")
                .and_then(|diagnostics| diagnostics.get("transport"))
                .and_then(|value| value.as_str())
        });

    match transport {
        Some("pull") => DiagnosticsTransport::Pull,
        _ => DiagnosticsTransport::Push,
    }
}

pub(super) fn workspace_roots_from_initialize(params: &InitializeParams) -> Vec<Url> {
    params
        .workspace_folders
        .as_ref()
        .map(|folders| folders.iter().map(|folder| folder.uri.clone()).collect())
        .or_else(|| params.root_uri.clone().map(|uri| vec![uri]))
        .unwrap_or_default()
}

pub(super) const MANIFEST_WATCHER_REGISTRATION_ID: &str = "seagrass/watch-manifests";
pub(super) const MANIFEST_WATCHER_GLOBS: &[&str] =
    &["**/Cargo.toml", "**/Anchor.toml", "**/Seagrass.toml"];

pub(super) fn client_supports_watched_file_registration(capabilities: &ClientCapabilities) -> bool {
    capabilities
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.did_change_watched_files.as_ref())
        .and_then(|watched| watched.dynamic_registration)
        .unwrap_or(false)
}

pub(super) fn manifest_watcher_registrations() -> Vec<Registration> {
    let watchers = MANIFEST_WATCHER_GLOBS
        .iter()
        .map(|pattern| FileSystemWatcher {
            glob_pattern: GlobPattern::String((*pattern).to_string()),
            kind: None,
        })
        .collect();
    let options = DidChangeWatchedFilesRegistrationOptions { watchers };
    vec![Registration {
        id: MANIFEST_WATCHER_REGISTRATION_ID.to_string(),
        method: "workspace/didChangeWatchedFiles".to_string(),
        register_options: Some(
            serde_json::to_value(options)
                .expect("DidChangeWatchedFilesRegistrationOptions always serializes"),
        ),
    }]
}

#[cfg(test)]
mod manifest_watcher_tests {
    use super::*;
    use tower_lsp::lsp_types::{
        DidChangeWatchedFilesClientCapabilities, WorkspaceClientCapabilities,
    };

    #[test]
    fn detects_dynamic_registration_capability() {
        let capabilities = ClientCapabilities {
            workspace: Some(WorkspaceClientCapabilities {
                did_change_watched_files: Some(DidChangeWatchedFilesClientCapabilities {
                    dynamic_registration: Some(true),
                    relative_pattern_support: None,
                }),
                ..WorkspaceClientCapabilities::default()
            }),
            ..ClientCapabilities::default()
        };
        assert!(client_supports_watched_file_registration(&capabilities));
    }

    #[test]
    fn rejects_missing_or_disabled_capability() {
        assert!(!client_supports_watched_file_registration(
            &ClientCapabilities::default()
        ));
        let disabled = ClientCapabilities {
            workspace: Some(WorkspaceClientCapabilities {
                did_change_watched_files: Some(DidChangeWatchedFilesClientCapabilities {
                    dynamic_registration: Some(false),
                    relative_pattern_support: None,
                }),
                ..WorkspaceClientCapabilities::default()
            }),
            ..ClientCapabilities::default()
        };
        assert!(!client_supports_watched_file_registration(&disabled));
    }

    #[test]
    fn registration_covers_cargo_anchor_and_seagrass_manifests() {
        let registrations = manifest_watcher_registrations();
        assert_eq!(registrations.len(), 1);
        let registration = &registrations[0];
        assert_eq!(registration.id, MANIFEST_WATCHER_REGISTRATION_ID);
        assert_eq!(registration.method, "workspace/didChangeWatchedFiles");

        let options: DidChangeWatchedFilesRegistrationOptions = serde_json::from_value(
            registration
                .register_options
                .clone()
                .expect("register_options present"),
        )
        .expect("options deserialize");

        let globs: Vec<String> = options
            .watchers
            .iter()
            .map(|watcher| match &watcher.glob_pattern {
                GlobPattern::String(pattern) => pattern.clone(),
                GlobPattern::Relative(_) => panic!("unexpected relative glob"),
            })
            .collect();
        assert_eq!(
            globs,
            vec![
                "**/Cargo.toml".to_string(),
                "**/Anchor.toml".to_string(),
                "**/Seagrass.toml".to_string(),
            ]
        );
    }
}

#[cfg(test)]
mod syntax_diagnostic_suppression_tests {
    use super::*;

    #[test]
    fn seagrass_ignore_suppresses_open_document_syntax_diagnostic() {
        let source = r#"
pub fn handler() -> Result<()> {
    // seagrass-ignore
    position_bundle.(bundle_index)?;
    Ok(())
}
"#;
        let document = ParsedOpenDocument::new(source.to_string(), Some(1));
        assert!(
            document.open.syntax_diagnostic.is_some(),
            "fixture must produce the parse-pause diagnostic"
        );

        let diagnostics = syntax_diagnostics_for_document(&document.parsed, &document.open, None);

        assert!(
            diagnostics.is_empty(),
            "`seagrass-ignore` must suppress the parse-pause diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn workspace_allow_suppresses_open_document_syntax_diagnostic() {
        let source = r#"
pub fn handler() -> Result<()> {
    position_bundle.(bundle_index)?;
    Ok(())
}
"#;
        let document = ParsedOpenDocument::new(source.to_string(), Some(1));
        let seagrass_toml = r#"
[lints]
allow = ["seagrass/anchor.syntax"]
"#;

        let diagnostics =
            syntax_diagnostics_for_document(&document.parsed, &document.open, Some(seagrass_toml));

        assert!(
            diagnostics.is_empty(),
            "workspace syntax suppression must cover parse-pause diagnostics: {diagnostics:#?}"
        );
    }
}
