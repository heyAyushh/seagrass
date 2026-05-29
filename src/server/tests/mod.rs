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
fn capabilities_advertise_refactor_code_actions_for_assists() {
    let capabilities = server_capabilities(DiagnosticsTransport::Push);
    let code_action_provider = capabilities
        .code_action_provider
        .expect("code action provider");
    let CodeActionProviderCapability::Options(options) = code_action_provider else {
        panic!("expected code action options");
    };
    let kinds = options.code_action_kinds.expect("code action kinds");

    assert!(kinds.contains(&CodeActionKind::QUICKFIX));
    assert!(kinds.contains(&CodeActionKind::REFACTOR));
    assert!(kinds.contains(&CodeActionKind::SOURCE));
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
        PROPOSE_ASSISTS_COMMAND,
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
fn diagnostics_transport_reads_pull_initialization_options() {
    let options = serde_json::json!({
        "seagrass": {
            "diagnostics": {
                "transport": "pull"
            }
        }
    });

    assert_eq!(
        diagnostics_transport_from_initialize_options(Some(&options)),
        DiagnosticsTransport::Pull
    );
}

#[test]
fn diagnostics_transport_rejects_mixed_initialization_options() {
    let options = serde_json::json!({
        "seagrass": {
            "diagnostics": {
                "transport": "both"
            }
        }
    });

    assert_eq!(
        diagnostics_transport_from_initialize_options(Some(&options)),
        DiagnosticsTransport::Push
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
        agent_mode: false,
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
        DiagnosticsTransport::Push,
        &settings,
    );

    assert!(status.contains("Seagrass 1.2.3"));
    assert!(status.contains("workspace roots: file:///workspace"));
    assert!(status.contains("indexed files: 17"));
    assert!(status.contains("open documents: 2"));
    assert!(status.contains("sync: full"));
    assert!(status.contains("diagnostics: push"));
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
        agent_mode: false,
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
fn bundled_feedback_link_comes_from_manifest() {
    let link = super::backend_features::bundled_feedback_link();

    assert_eq!(link.label, "Join the Seagrass Telegram");
    assert_eq!(link.url, "https://t.me/+8HdUVX0F1t9lMTk1");
}

mod reports;
