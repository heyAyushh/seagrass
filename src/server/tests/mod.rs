use super::*;

#[test]
fn push_diagnostics_do_not_advertise_pull_provider() {
    let capabilities = server_capabilities(DiagnosticsTransport::Push);

    assert!(capabilities.diagnostic_provider.is_none());
    assert!(capabilities.document_link_provider.is_some());
}

#[test]
fn pull_diagnostics_advertise_pull_provider() {
    let capabilities = server_capabilities(DiagnosticsTransport::Pull);

    assert!(capabilities.diagnostic_provider.is_some());
}

#[test]
fn capabilities_advertise_anchor_type_definition_provider() {
    let capabilities = server_capabilities(DiagnosticsTransport::Push);

    assert!(matches!(
        capabilities.declaration_provider,
        Some(DeclarationCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.type_definition_provider,
        Some(TypeDefinitionProviderCapability::Simple(true))
    ));
}

#[test]
fn capabilities_advertise_anchor_implementation_provider() {
    let capabilities = server_capabilities(DiagnosticsTransport::Push);

    assert!(matches!(
        capabilities.implementation_provider,
        Some(ImplementationProviderCapability::Simple(true))
    ));
    assert!(capabilities.code_lens_provider.is_some());
}

#[test]
fn capabilities_advertise_support_commands() {
    let capabilities = server_capabilities(DiagnosticsTransport::Push);
    let commands = capabilities
        .execute_command_provider
        .expect("execute command provider")
        .commands;

    for command in [
        STATUS_COMMAND,
        ANALYZE_COMMAND,
        ARTIFACTS_COMMAND,
        SEAGRASS_INSTRUCTION_SUMMARY_COMMAND,
        SEAGRASS_PROGRAM_REPORT_COMMAND,
        ERROR_COVERAGE_COMMAND,
        SUPPORT_MATRIX_COMMAND,
        GENERATOR_PROFILE_COMMAND,
        LOGS_COMMAND,
        PROJECT_COVERAGE_COMMAND,
        FEEDBACK_COMMAND,
    ] {
        assert!(
            commands.contains(&command.to_string()),
            "missing command capability for {command}"
        );
    }
}

#[test]
fn completion_advertises_anchor_slot_trigger_characters_for_fast_editor_wakeups() {
    let capabilities = server_capabilities(DiagnosticsTransport::Push);
    let trigger_characters = capabilities
        .completion_provider
        .expect("completion provider")
        .trigger_characters
        .expect("completion trigger characters");

    for trigger in ["a", "Z", "_", " ", ".", "<", ",", "="] {
        assert!(
            trigger_characters.contains(&trigger.to_string()),
            "missing completion trigger character {trigger}"
        );
    }
}

#[test]
fn completion_triggers_do_not_include_delimiters_without_anchor_slot_value() {
    let capabilities = server_capabilities(DiagnosticsTransport::Push);
    let trigger_characters = capabilities
        .completion_provider
        .expect("completion provider")
        .trigger_characters
        .expect("completion trigger characters");

    for trigger in ["(", ":", "#", "["] {
        assert!(
            !trigger_characters.contains(&trigger.to_string()),
            "punctuation trigger {trigger} would open noisy empty completion lists"
        );
    }
}

#[test]
fn text_document_sync_advertises_full_changes_and_save_notifications() {
    let capabilities = server_capabilities(DiagnosticsTransport::Push);
    let Some(TextDocumentSyncCapability::Options(options)) = capabilities.text_document_sync else {
        panic!("expected text document sync options");
    };

    assert_eq!(options.open_close, Some(true));
    assert_eq!(options.change, Some(TextDocumentSyncKind::FULL));
    assert!(matches!(
        options.save,
        Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
            include_text: Some(false)
        }))
    ));
}

#[test]
fn diagnostics_transport_defaults_to_push() {
    assert_eq!(
        diagnostics_transport_from_initialize_options(None),
        DiagnosticsTransport::Push
    );
}

#[test]
fn diagnostics_transport_reads_nested_initialization_options() {
    let options = serde_json::json!({
        "seagrass": {
            "diagnostics": {
                "transport": "both"
            }
        }
    });

    assert_eq!(
        diagnostics_transport_from_initialize_options(Some(&options)),
        DiagnosticsTransport::Both
    );
}

#[test]
fn stale_diagnostic_publish_guard_rejects_older_versions() {
    assert!(is_stale_diagnostic_publish(4, Some(3)));
    assert!(!is_stale_diagnostic_publish(4, Some(4)));
    assert!(!is_stale_diagnostic_publish(4, Some(5)));
    assert!(!is_stale_diagnostic_publish(4, None));
}

#[test]
fn full_sync_change_range_tracks_current_edit_span() {
    let previous =
        "use anchor_lang::prelude::*;\n#[account(init)]\npub state: Account<'info, State>,\n";
    let current = "use anchor_lang::prelude::*;\n#[account(init, payer = user)]\npub state: Account<'info, State>,\n";

    let range = changed_range_between_texts(previous, current)
        .expect("full sync edit should produce a typing range");

    assert_eq!(
        range,
        tower_lsp::lsp_types::Range {
            start: tower_lsp::lsp_types::Position {
                line: 1,
                character: 14,
            },
            end: tower_lsp::lsp_types::Position {
                line: 1,
                character: 28,
            },
        }
    );
}

#[test]
fn status_text_reports_observable_server_state() {
    let settings = ServerSettings {
        security_diagnostics: true,
        experimental_diagnostics: false,
        security_levels: BTreeMap::new(),
        strict_native_security: true,
        diagnostics_cold_path: crate::server_types::DiagnosticsColdPath::Save,
        workspace_index: false,
        trace_server: true,
        feedback_url: None,
        editor_context: crate::server_types::EditorContext::default(),
    };

    let status = server_observability::status_text_from_parts(
        "1.2.3",
        "file:///workspace",
        17,
        2,
        DiagnosticsTransport::Both,
        &settings,
    );

    assert!(status.contains("Seagrass 1.2.3"));
    assert!(status.contains("workspace roots: file:///workspace"));
    assert!(status.contains("indexed files: 17"));
    assert!(status.contains("open documents: 2"));
    assert!(status.contains("sync: full"));
    assert!(status.contains("diagnostics: both"));
    assert!(status.contains("coldPath=save"));
    assert!(status.contains(
            "features: security=true, experimental=false, strictNative=true, workspaceIndex=false, trace=true, editor=generic"
        ));
}

#[test]
fn recent_logs_snapshot_preserves_order_and_metadata() {
    let mut logs = VecDeque::new();
    logs.push_back(ServerLogEntry {
        unix_ms: 10,
        level: "info",
        event: "initialized",
        message: "Seagrass initialized".to_string(),
        data: serde_json::json!({ "diagnosticsTransport": "push" }),
    });
    logs.push_back(ServerLogEntry {
        unix_ms: 20,
        level: "info",
        event: "documentAnalyzed",
        message: "analyzed document: 1 diagnostics in 2ms".to_string(),
        data: serde_json::json!({ "diagnostics": 1, "durationMs": 2, "version": 7 }),
    });

    let snapshot = server_observability::recent_logs_snapshot_from_entries(&logs, RECENT_LOG_LIMIT);
    let entries = snapshot
        .get("entries")
        .and_then(|entries| entries.as_array())
        .expect("log entries");

    assert_eq!(
        snapshot.get("limit").and_then(|limit| limit.as_u64()),
        Some(200)
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get("event").and_then(|event| event.as_str()),
        Some("initialized")
    );
    assert_eq!(
        entries[1].get("data").and_then(|data| data.get("version")),
        Some(&serde_json::json!(7))
    );
}

#[test]
fn trace_setting_controls_noisy_analysis_client_logs() {
    let mut settings = ServerSettings::default();

    assert!(!server_observability::should_emit_client_log(
        "documentAnalyzed",
        &settings
    ));
    assert!(server_observability::should_emit_client_log(
        "initialized",
        &settings
    ));

    settings.trace_server = true;

    assert!(server_observability::should_emit_client_log(
        "documentAnalyzed",
        &settings
    ));
}

#[test]
fn diagnostic_code_counts_group_log_metadata_by_stable_code() {
    let diagnostic =
        |code: Option<tower_lsp::lsp_types::NumberOrString>| tower_lsp::lsp_types::Diagnostic {
            range: tower_lsp::lsp_types::Range::default(),
            severity: None,
            code,
            code_description: None,
            source: Some("seagrass".to_string()),
            message: "diagnostic".to_string(),
            related_information: None,
            tags: None,
            data: None,
        };
    let counts = diagnostic_code_counts(&[
        diagnostic(Some(tower_lsp::lsp_types::NumberOrString::String(
            "anchor-context-accounts".to_string(),
        ))),
        diagnostic(Some(tower_lsp::lsp_types::NumberOrString::String(
            "anchor-context-accounts".to_string(),
        ))),
        diagnostic(Some(tower_lsp::lsp_types::NumberOrString::Number(4101))),
        diagnostic(None),
    ]);

    assert_eq!(counts["anchor-context-accounts"], 2);
    assert_eq!(counts["4101"], 1);
    assert_eq!(counts["uncoded"], 1);
}

#[test]
fn navigation_log_data_reports_method_position_source_and_count() {
    let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let data = server_observability::navigation_log_data(
        "textDocument/implementation",
        &uri,
        tower_lsp::lsp_types::Position {
            line: 12,
            character: 8,
        },
        "workspace",
        2,
    );

    assert_eq!(data["method"], "textDocument/implementation");
    assert_eq!(data["uri"], "file:///workspace/programs/demo/src/lib.rs");
    assert_eq!(data["line"], 12);
    assert_eq!(data["character"], 8);
    assert_eq!(data["source"], "workspace");
    assert_eq!(data["resultCount"], 2);
}

#[test]
fn watched_file_change_log_data_reports_republished_open_documents() {
    let data = watched_files_changed_log_data(3, 2);

    assert_eq!(data["changedFiles"], 3);
    assert_eq!(data["republishedOpenDocuments"], 2);
}

#[test]
fn workspace_root_change_log_data_reports_republished_open_documents() {
    let data = workspace_roots_changed_log_data("file:///workspace", 2, 1, 4);

    assert_eq!(data["workspaceRoots"], "file:///workspace");
    assert_eq!(data["added"], 2);
    assert_eq!(data["removed"], 1);
    assert_eq!(data["republishedOpenDocuments"], 4);
}

#[test]
fn settings_accept_flat_and_nested_vscode_shapes() {
    let mut settings = ServerSettings::default();

    settings.apply(serde_json::json!({
        "seagrass": {
            "diagnostics.security.enabled": false,
            "diagnostics": {
                "coldPath": "save",
                "experimental": {
                    "enabled": false
                }
            },
            "diagnostics.security.ownerChecks": "error",
            "security": {
                "strictNative": {
                    "enabled": false
                }
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
            },
            "workspaceIndex.enabled": false,
            "trace": {
                "server": true
            }
        }
    }));

    assert!(!settings.security_diagnostics);
    assert!(!settings.experimental_diagnostics);
    assert_eq!(
        settings.security_levels.get("security.ownerChecks"),
        Some(&diagnostics::DiagnosticLevel::Error)
    );
    assert!(!settings.strict_native_security);
    assert_eq!(settings.editor_context.client, "zed");
    assert_eq!(
        settings.diagnostics_cold_path,
        crate::server_types::DiagnosticsColdPath::Save
    );
    assert!(!settings.workspace_index);
    assert!(settings.trace_server);
}

#[test]
fn agent_mode_fills_unset_server_settings() {
    let mut settings = ServerSettings {
        security_diagnostics: false,
        experimental_diagnostics: false,
        security_levels: BTreeMap::new(),
        strict_native_security: false,
        diagnostics_cold_path: crate::server_types::DiagnosticsColdPath::Save,
        workspace_index: true,
        trace_server: false,
        feedback_url: None,
        editor_context: crate::server_types::EditorContext::default(),
    };

    settings.apply(serde_json::json!({
        "seagrass": {
            "agent.mode": true
        }
    }));

    assert!(settings.security_diagnostics);
    assert!(settings.experimental_diagnostics);
    assert!(settings.strict_native_security);
    assert_eq!(
        settings.diagnostics_cold_path,
        crate::server_types::DiagnosticsColdPath::Idle
    );
    assert!(settings.trace_server);
    assert_eq!(settings.security_levels.len(), 9);
    assert_eq!(
        settings.security_levels.get("security.ownerChecks"),
        Some(&diagnostics::DiagnosticLevel::Warn)
    );
    assert_eq!(
        settings.security_levels.get("security.pdaSeedCollision"),
        Some(&diagnostics::DiagnosticLevel::Warn)
    );
}

#[test]
fn agent_mode_preserves_explicit_server_settings() {
    let mut settings = ServerSettings::default();

    settings.apply(serde_json::json!({
        "seagrass": {
            "agent": {
                "mode": true
            },
            "diagnostics.security.enabled": false,
            "diagnostics.experimental.enabled": false,
            "security.strictNative.enabled": false,
            "diagnostics.coldPath": "manual",
            "trace.server": false,
            "diagnostics.security.ownerChecks": "error",
            "diagnostics.security.typeCosplay": "off",
            "diagnostics.security.pdaSeedCollision": "hint"
        }
    }));

    assert!(!settings.security_diagnostics);
    assert!(!settings.experimental_diagnostics);
    assert!(!settings.strict_native_security);
    assert_eq!(
        settings.diagnostics_cold_path,
        crate::server_types::DiagnosticsColdPath::Manual
    );
    assert!(!settings.trace_server);
    assert_eq!(settings.security_levels.len(), 9);
    assert_eq!(
        settings.security_levels.get("security.ownerChecks"),
        Some(&diagnostics::DiagnosticLevel::Error)
    );
    assert_eq!(
        settings.security_levels.get("security.typeCosplay"),
        Some(&diagnostics::DiagnosticLevel::Off)
    );
    assert_eq!(
        settings.security_levels.get("security.arbitraryCpi"),
        Some(&diagnostics::DiagnosticLevel::Warn)
    );
    assert_eq!(
        settings.security_levels.get("security.pdaSeedCollision"),
        Some(&diagnostics::DiagnosticLevel::Hint)
    );
}

#[test]
fn feedback_url_setting_tracks_non_empty_values() {
    let mut settings = ServerSettings::default();

    settings.apply(serde_json::json!({
        "seagrass": {
            "feedback": {
                "url": " https://example.test/seagrass "
            }
        }
    }));

    assert_eq!(
        settings.feedback_url.as_deref(),
        Some("https://example.test/seagrass")
    );

    settings.apply(serde_json::json!({
        "seagrass": {
            "feedback.url": ""
        }
    }));

    assert_eq!(settings.feedback_url, None);
}

#[test]
fn feedback_response_uses_server_owned_shape() {
    assert_eq!(
        super::backend_features::feedback_response("https://example.test/seagrass"),
        serde_json::json!({
            "url": "https://example.test/seagrass",
            "label": "Join the Seagrass Telegram",
        })
    );
}

#[test]
fn analysis_uri_from_args_accepts_string_and_object_arguments() {
    let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();

    assert_eq!(
        analysis_uri_from_args(&[serde_json::json!(uri.as_str())]),
        Some(uri.clone())
    );
    assert_eq!(
        analysis_uri_from_args(&[serde_json::json!({ "uri": uri.as_str() })]),
        Some(uri)
    );
    assert!(analysis_uri_from_args(&[serde_json::json!({ "path": "/tmp/lib.rs" })]).is_none());
    assert_eq!(
        analysis_instruction_from_args(&[serde_json::json!({
            "uri": "file:///workspace/programs/demo/src/lib.rs",
            "instruction": "create"
        })]),
        Some("create")
    );
    assert_eq!(
        analysis_instruction_from_args(&[serde_json::json!({
            "uri": "file:///workspace/programs/demo/src/lib.rs",
            "function": "initialize"
        })]),
        Some("initialize")
    );
    assert_eq!(
        analysis_context_from_args(&[serde_json::json!({
            "uri": "file:///workspace/programs/demo/src/lib.rs",
            "context": "Create"
        })]),
        Some("Create")
    );
}

#[test]
fn instruction_summary_exports_agent_facing_shape() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, amount: u64) -> Result<()> {
        let _payer = ctx.accounts.payer.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}
"#,
    )
    .unwrap();

    let summary = instruction_summary(&document, &[], "initialize", None);

    assert_eq!(summary["name"], "initialize");
    assert_eq!(summary["context"], "Create");
    assert_eq!(summary["args"][0]["name"], "amount");
    assert!(
        summary["accounts"]
            .as_array()
            .is_some_and(|accounts| accounts.iter().any(|account| {
                account["name"] == "payer"
                    && account["ty"] == "Signer"
                    && account["mutability"] == true
                    && account["signer"] == true
            })),
        "{summary}"
    );
    assert_eq!(summary["errorsReturned"], serde_json::json!([]));
}

#[test]
fn analysis_report_exposes_agent_friendly_symbols_evidence_and_diagnostics() {
    let source = r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod demo {
    use super::*;

    pub fn create(ctx: Context<Create>) -> Result<()> {
        ctx.accounts.missing.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
}
"#;
    let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = diagnostics::collect(&document);
    let report = analysis_report(
        &uri,
        &document,
        &diagnostics,
        None,
        Some("create"),
        None,
        None,
    );

    assert_eq!(report["uri"], uri.as_str());
    assert_eq!(report["project"], serde_json::Value::Null);
    assert_eq!(report["focus"]["kind"], "anchor.programInstruction");
    assert_eq!(report["focus"]["found"], true);
    assert_eq!(report["focus"]["instruction"], "create");
    assert_eq!(report["focus"]["context"]["name"], "Create");
    assert!(report["focus"]["contextFields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| {
            field["name"] == "user"
                && field["type"] == "Signer"
                && field["source"] == "localDocument"
        }));
    assert!(report["focus"]["accountUsages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|usage| usage["name"] == "missing"));
    assert!(report["focus"]["resolvedAccountUsages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|usage| usage["name"] == "missing" && usage["declared"] == false));
    assert!(report["focus"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "anchor-missing-account-reference"));
    assert_eq!(
        report["anchorSupport"]["generatorProfile"]["generationMode"],
        "source-driven"
    );
    assert!(report["evidence"]["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|instruction| instruction["name"] == "create" && instruction["context"] == "Create"));
    assert!(report["documentSymbols"]
        .as_array()
        .unwrap()
        .iter()
        .any(|symbol| symbol["name"] == "demo"));
    assert!(report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "anchor-missing-account-reference"));

    let context_report = analysis_report(
        &uri,
        &document,
        &diagnostics,
        None,
        None,
        Some("Create"),
        None,
    );
    assert_eq!(context_report["focus"]["kind"], "anchor.accountsContext");
    assert_eq!(context_report["focus"]["found"], true);
    assert_eq!(context_report["focus"]["context"], "Create");
    assert!(context_report["focus"]["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|instruction| instruction["name"] == "create"));
    assert!(context_report["focus"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["name"] == "user" && field["type"] == "Signer"));
}

#[test]
fn accounts_context_focus_uses_workspace_instructions_for_split_files() {
    let accounts_uri = Url::parse("file:///workspace/programs/demo/src/accounts.rs").unwrap();
    let handler_uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let handler_source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        let user = ctx.accounts.user.key();
        let inner = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(accounts_source).unwrap();
    let diagnostics = diagnostics::collect(&document);
    let workspace_index = workspace::WorkspaceIndex::build(
        &[],
        [
            (accounts_uri.clone(), accounts_source.to_string()),
            (handler_uri.clone(), handler_source.to_string()),
        ],
    );
    let report = analysis_report(
        &accounts_uri,
        &document,
        &diagnostics,
        None,
        None,
        Some("Create"),
        Some(&workspace_index),
    );

    assert_eq!(report["focus"]["kind"], "anchor.accountsContext");
    assert_eq!(report["focus"]["context"], "Create");
    assert!(report["focus"]["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|instruction| {
            instruction["name"] == "initialize" && instruction["uri"] == handler_uri.as_str()
        }));
}

#[test]
fn instruction_focus_uses_workspace_context_fields_for_split_files() {
    let accounts_uri = Url::parse("file:///workspace/programs/demo/src/accounts.rs").unwrap();
    let handler_uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let handler_source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        let user = ctx.accounts.user.key();
        let inner = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(handler_source).unwrap();
    let diagnostics = diagnostics::collect(&document);
    let workspace_index = workspace::WorkspaceIndex::build(
        &[],
        [
            (accounts_uri, accounts_source.to_string()),
            (handler_uri.clone(), handler_source.to_string()),
        ],
    );
    let report = analysis_report(
        &handler_uri,
        &document,
        &diagnostics,
        None,
        Some("initialize"),
        None,
        Some(&workspace_index),
    );

    assert_eq!(report["focus"]["kind"], "anchor.programInstruction");
    assert_eq!(report["focus"]["instruction"], "initialize");
    assert!(report["focus"]["contextFields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| {
            field["name"] == "user"
                && field["type"] == "Signer"
                && field["source"] == "workspaceIndex"
        }));
    assert!(report["focus"]["resolvedAccountUsages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|usage| {
            usage["name"] == "user"
                && usage["declared"] == true
                && usage["type"] == "Signer"
                && usage["source"] == "workspaceIndex"
        }));
    assert!(report["focus"]["resolvedAccountPathUsages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|usage| {
            usage["path"] == "wrapper.inner"
                && usage["segments"].as_array().unwrap().iter().any(|segment| {
                    segment["name"] == "inner"
                        && segment["declared"] == true
                        && segment["container"] == "Wrapped"
                        && segment["type"] == "Account<Inner>"
                        && segment["source"] == "workspaceIndex"
                })
        }));
}
