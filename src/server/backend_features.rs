use super::*;

struct CompletionLog<'a> {
    uri: &'a Url,
    position: tower_lsp::lsp_types::Position,
    version: i32,
    source: &'static str,
    result_count: usize,
    started_at: Instant,
    signature: Option<&'a completions::CompletionSignature>,
    context: Option<&'a CompletionContext>,
}

#[cfg(test)]
pub(super) fn feedback_response(url: &str) -> serde_json::Value {
    let manifest = bundled_feedback_link();
    feedback_response_with_label(url, &manifest.label)
}

fn feedback_response_with_label(url: &str, label: &str) -> serde_json::Value {
    serde_json::json!({
        "url": url,
        "label": label,
    })
}

pub(super) fn bundled_feedback_link() -> FeedbackLink {
    let manifest = toml::from_str::<FeedbackManifest>(crate::SEAGRASS_FEEDBACK_MANIFEST)
        .expect("bundled Seagrass feedback manifest must be valid TOML");
    manifest.feedback
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct FeedbackLink {
    pub(super) label: String,
    pub(super) url: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeedbackManifest {
    feedback: FeedbackLink,
}

impl Backend {
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn refresh_workspace_index(&self) {
        if !self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .workspace_index
        {
            *self
                .workspace_index
                .write()
                .unwrap_or_else(|err| err.into_inner()) = workspace::WorkspaceIndex::default();
            return;
        }

        let roots = self
            .workspace_roots
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let open_documents = self
            .documents
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().text.clone()))
            .collect::<Vec<_>>();

        let new_index = tokio::task::spawn_blocking(move || {
            crate::measure_hotpath_block!("lsp.workspace.scan", {
                workspace::WorkspaceIndex::build(&roots, open_documents)
            })
        })
        .await
        .unwrap_or_else(|err| {
            eprintln!("Workspace index build panicked: {err}");
            workspace::WorkspaceIndex::default()
        });

        *self
            .workspace_index
            .write()
            .unwrap_or_else(|err| err.into_inner()) = new_index;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn republish_open_documents(&self) -> usize {
        let documents = self.documents.clone().into_iter().collect::<Vec<_>>();
        let count = documents.len();
        for (uri, document) in documents {
            let diagnostics = self.analysis_diagnostics_for_open_document(&uri, &document);
            self.publish_analysis(
                uri,
                diagnostics,
                document.version,
                DiagnosticPublishLane::Republish,
            )
            .await;
        }
        count
    }

    pub(super) async fn register_manifest_watchers(&self) {
        if !self
            .supports_watched_file_registration
            .load(Ordering::Relaxed)
        {
            return;
        }
        let registrations = manifest_watcher_registrations();
        match self.client.register_capability(registrations).await {
            Ok(()) => {
                self.record_log(
                    "info",
                    "manifestWatchersRegistered",
                    "registered watched-file globs for Cargo.toml, Anchor.toml, Seagrass.toml",
                    serde_json::json!({ "globs": MANIFEST_WATCHER_GLOBS }),
                );
            }
            Err(err) => {
                self.record_log(
                    "warn",
                    "manifestWatchersRegistrationFailed",
                    format!("failed to register manifest watchers: {err}"),
                    serde_json::json!({ "error": err.to_string() }),
                );
            }
        }
    }

    pub(super) fn status_text(&self) -> String {
        let roots = self.workspace_root_log();
        let indexed = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .indexed_file_count();
        let open = self.documents.len();
        let diagnostics_transport = self.diagnostics_transport();
        let settings = self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        server_observability::status_text_from_parts(
            env!("CARGO_PKG_VERSION"),
            &roots,
            indexed,
            open,
            diagnostics_transport,
            &settings,
        )
    }

    pub(super) fn settings_snapshot(&self) -> serde_json::Value {
        let settings = self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        serde_json::json!({
            "securityDiagnostics": settings.security_diagnostics,
            "experimentalDiagnostics": settings.experimental_diagnostics,
            "securityLevels": settings
                .security_levels
                .iter()
                .map(|(key, level)| (key.clone(), format!("{level:?}").to_ascii_lowercase()))
                .collect::<BTreeMap<_, _>>(),
            "strictNativeSecurity": settings.strict_native_security,
            "diagnosticsColdPath": settings.diagnostics_cold_path.as_str(),
            "workspaceIndex": settings.workspace_index,
            "traceServer": settings.trace_server,
            "editorContext": {
                "client": settings.editor_context.client,
                "inlineValues": settings.editor_context.inline_values,
                "completionTelemetry": settings.editor_context.completion_telemetry,
                "diagnosticTelemetry": settings.editor_context.diagnostic_telemetry,
            },
        })
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn emit_log(
        &self,
        message_type: MessageType,
        level: &'static str,
        event: &'static str,
        message: String,
        data: serde_json::Value,
    ) {
        let should_log_to_client = {
            let settings = self
                .settings
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone();
            server_observability::should_emit_client_log(event, &settings)
        };
        self.record_log(level, event, message.clone(), data);
        if should_log_to_client {
            self.client.log_message(message_type, message).await;
        }
    }

    pub(super) fn record_log(
        &self,
        level: &'static str,
        event: &'static str,
        message: impl Into<String>,
        data: serde_json::Value,
    ) {
        let mut logs = self
            .recent_logs
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if logs.len() == RECENT_LOG_LIMIT {
            logs.pop_front();
        }
        logs.push_back(ServerLogEntry {
            unix_ms: server_observability::now_unix_ms(),
            level,
            event,
            message: message.into(),
            data,
        });
    }

    pub(super) fn record_navigation_log(
        &self,
        method: &'static str,
        uri: &Url,
        position: tower_lsp::lsp_types::Position,
        source: &'static str,
        result_count: usize,
    ) {
        self.record_log(
            "info",
            "navigation",
            format!("{method}: {result_count} result(s)"),
            server_observability::navigation_log_data(method, uri, position, source, result_count),
        );
    }

    fn record_completion_log(&self, event: CompletionLog<'_>) {
        self.record_log(
            "info",
            "completionServed",
            format!(
                "completion served from {}: {} item(s)",
                event.source, event.result_count
            ),
            serde_json::json!({
                "uri": event.uri,
                "line": event.position.line,
                "character": event.position.character,
                "version": event.version,
                "source": event.source,
                "resultCount": event.result_count,
                "durationMs": event.started_at.elapsed().as_millis(),
                "triggerKind": event.context.map(|context| format!("{:?}", context.trigger_kind)),
                "triggerCharacter": event.context.and_then(|context| context.trigger_character.as_deref()),
                "gate": if event.signature.is_some() { "accepted" } else { "rejected" },
                "editorClient": self.settings.lock().unwrap_or_else(|err| err.into_inner()).editor_context.client,
                "signature": event.signature.map(|sig| serde_json::json!({
                    "line": sig.line,
                    "kind": format!("{:?}", sig.kind),
                    "prefix": sig.prefix,
                })),
            }),
        );
    }

    pub(super) fn recent_logs_snapshot(&self) -> serde_json::Value {
        let logs = self
            .recent_logs
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        server_observability::recent_logs_snapshot_from_entries(&logs, RECENT_LOG_LIMIT)
    }

    pub(super) fn feedback_command(&self) -> serde_json::Value {
        let bundled = bundled_feedback_link();
        let url = self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .feedback_url
            .clone()
            .unwrap_or(bundled.url);
        feedback_response_with_label(&url, &bundled.label)
    }

    pub(super) fn project_coverage_command(&self) -> serde_json::Value {
        let support = anchor_support::matrix();
        let documents = self.documents.clone().into_iter().collect::<Vec<_>>();
        let mut diagnostics_by_code = BTreeMap::new();
        let mut diagnostics_by_attack = BTreeMap::new();
        let mut documents_summary = Vec::new();
        for (uri, document) in documents {
            let parsed = ParsedDocument::parse_or_empty(document.text.clone());
            let diagnostics = self.collect_diagnostics_for_uri(&uri, &parsed);
            for (code, count) in diagnostic_code_count_map(&diagnostics) {
                *diagnostics_by_code.entry(code).or_insert(0usize) += count;
            }
            for diagnostic in &diagnostics {
                if let Some(attack) = diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("attack"))
                    .and_then(|value| value.as_str())
                {
                    *diagnostics_by_attack
                        .entry(attack.to_string())
                        .or_insert(0usize) += 1;
                }
            }
            documents_summary.push(serde_json::json!({
                "uri": uri,
                "version": document.version,
                "diagnostics": diagnostics.len(),
                "diagnosticsByCode": diagnostic_code_counts(&diagnostics),
                "program": solana_project::detect_for_document(&uri, &parsed).map(|program| serde_json::json!({
                    "kind": program.kind.as_str(),
                    "name": program.name,
                    "root": program.root.to_string_lossy(),
                })),
            }));
        }

        serde_json::json!({
            "settings": self.settings_snapshot(),
            "openDocuments": documents_summary,
            "diagnosticsByCode": diagnostics_by_code,
            "diagnosticsByAttack": diagnostics_by_attack,
            "securityPatternCoverage": support["securityPatternCoverage"].clone(),
            "securityRegressionCorpus": support["securityRegressionCorpus"].clone(),
            "capabilityGaps": support["capabilityGaps"].clone(),
        })
    }

    pub(super) fn analyze_command(
        &self,
        arguments: Vec<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        let uri = analysis_uri_from_args(&arguments)?;
        let instruction = analysis_instruction_from_args(&arguments);
        let context = analysis_context_from_args(&arguments);
        let document = self.document_for(&uri)?;
        let diagnostics = self.collect_diagnostics_for_uri(&uri, &document);

        let source: Arc<str> = Arc::from(document.source());
        let version = self.current_document_version(&uri);
        let _ = {
            // Recover from poison instead of panicking the whole LSP server.
            let db = self.salsa_db.lock().unwrap_or_else(|e| e.into_inner());
            db.constraint_diagnostics(source, version)
        };

        let project = project::nearest_anchor_toml(&uri)
            .map(|(anchor_toml_uri, anchor_toml_text)| {
                project::summary(&uri, &document, &anchor_toml_uri, &anchor_toml_text)
            })
            .or_else(|| solana_project_summary(&uri, &document));
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());
        Some(analysis_report(
            &uri,
            &document,
            &diagnostics,
            project,
            instruction,
            context,
            Some(&workspace_index),
        ))
    }

    pub(super) fn artifacts_command(&self, arguments: Vec<serde_json::Value>) -> serde_json::Value {
        if let Some(uri) = analysis_uri_from_args(&arguments) {
            let document = self.document_for(&uri).or_else(|| {
                uri.to_file_path()
                    .ok()
                    .and_then(|path| fs::read_to_string(path).ok())
                    .map(ParsedDocument::parse_or_empty)
            });
            let report = document
                .as_ref()
                .and_then(|document| program_artifacts::report_for_document(&uri, document));
            let ecosystem = report
                .as_ref()
                .map(|report| ecosystem::report_for_program(&report.program).to_json());
            let anchor_toml =
                project::nearest_anchor_toml(&uri).map(|(anchor_toml_uri, _)| anchor_toml_uri);
            return serde_json::json!({
                "uri": uri,
                "anchorToml": anchor_toml,
                "artifacts": report.as_ref().map(program_artifacts::ProgramArtifactReport::to_json),
                "ecosystem": ecosystem,
            });
        }

        let roots = self
            .workspace_roots
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let reports = program_artifacts::reports_for_roots(&roots);
        serde_json::json!({
            "workspaceRoots": roots,
            "programs": reports.iter().map(|report| report.to_json()).collect::<Vec<_>>(),
        })
    }

    pub(super) fn propose_assists_command(
        &self,
        arguments: Vec<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        let uri = analysis_uri_from_args(&arguments)?;
        let document = self.document_for(&uri)?;
        let assists = assists::proposed_assists(&document, &uri);

        Some(serde_json::json!({
            "uri": uri,
            "assists": assists,
        }))
    }

    pub(super) fn instruction_summary_command(
        &self,
        arguments: Vec<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        let uri = analysis_uri_from_args(&arguments)?;
        let instruction = analysis_instruction_from_args(&arguments)?;
        let document = self.document_for(&uri)?;
        let diagnostics = self.collect_diagnostics_for_uri(&uri, &document);
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());

        Some(instruction_summary(
            &document,
            &diagnostics,
            instruction,
            Some(&workspace_index),
        ))
    }

    pub(super) fn program_report_command(&self) -> serde_json::Value {
        let roots = self
            .workspace_roots
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let programs = program_artifacts::reports_for_roots(&roots)
            .iter()
            .map(program_artifacts::ProgramArtifactReport::to_json)
            .collect::<Vec<_>>();

        let mut instructions = Vec::new();
        let mut pdas = Vec::new();
        let mut errors = Vec::new();
        for entry in self.documents.iter() {
            let uri = entry.key().clone();
            let document = ParsedDocument::parse_or_empty(entry.value().text.as_str());
            instructions.extend(open_document_instruction_summaries(&uri, &document));
            pdas.extend(open_document_pda_summaries(&uri, &document));
            errors.extend(
                self.collect_diagnostics_for_uri(&uri, &document)
                    .into_iter()
                    .map(|diagnostic| {
                        let mut summary = diagnostic_summary(&diagnostic);
                        if let Some(object) = summary.as_object_mut() {
                            object.insert("uri".to_string(), serde_json::json!(uri));
                        }
                        summary
                    }),
            );
        }

        serde_json::json!({
            "workspaceRoots": roots,
            "programs": programs,
            "instructions": instructions,
            "pdas": pdas,
            "errors": errors,
            "idlHash": serde_json::Value::Null,
        })
    }

    pub(super) async fn with_catch_unwind<T: Send + 'static, F>(
        &self,
        handler_name: &'static str,
        operation: F,
    ) -> Result<T>
    where
        F: FnOnce(&Self) -> Result<T> + Send + 'static,
    {
        let this = self.clone();
        let task_result = tokio::task::spawn_blocking(move || {
            let guarded = AssertUnwindSafe(|| operation(&this));
            catch_unwind(guarded)
        })
        .await;

        match task_result {
            Ok(Ok(result)) => result,
            Ok(Err(panic_payload)) => {
                let message = if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    (*s).to_string()
                } else {
                    "unknown panic in handler".to_string()
                };
                let full_message = format!("{}: {}", handler_name, message);
                eprintln!("[seagrass] {}", full_message);
                // InternalError: the handler panicked — this is a server bug, not a client mistake.
                // Contrast with InvalidParams which signals bad client input.
                Err(tower_lsp::jsonrpc::Error {
                    code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                    message: full_message.into(),
                    data: None,
                })
            }
            Err(join_err) => {
                let msg = format!("{} task join failed: {}", handler_name, join_err);
                // InternalError: tokio task join failure indicates a server runtime issue,
                // not invalid client input.
                Err(tower_lsp::jsonrpc::Error {
                    code: tower_lsp::jsonrpc::ErrorCode::InternalError,
                    message: msg.into(),
                    data: None,
                })
            }
        }
    }

    pub(super) fn document_symbol_impl(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let version = self.current_document_version(&uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::DocumentSymbols);
        if let Some(query_cache::CacheValue::DocumentSymbols(symbols)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            return Ok(Some(DocumentSymbolResponse::Nested(symbols)));
        }

        let Some(entry) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let source: Arc<str> = Arc::from(entry.text.as_str());
        // Recover from poison instead of panicking the whole LSP server.
        let db = self.salsa_db.lock().unwrap_or_else(|e| e.into_inner());
        let symbols = db.document_symbols(source, version);
        self.query_cache.insert(
            cache_key,
            version,
            query_cache::CacheValue::DocumentSymbols(symbols.clone()),
        );
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    pub(super) fn completion_impl(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let start = Instant::now();
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let completion_context = params.context;
        let version = self.current_document_version(&uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::Completion(position));
        if let Some(query_cache::CacheValue::Completion(result)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            self.record_completion_log(CompletionLog {
                uri: &uri,
                position,
                version,
                source: "queryCache",
                result_count: result.as_ref().map_or(0, Vec::len),
                started_at: start,
                signature: None,
                context: completion_context.as_ref(),
            });
            return Ok(result.map(CompletionResponse::Array));
        }

        let Some(text) = self.documents.get(&uri).map(|entry| entry.text.clone()) else {
            self.query_cache.insert(
                cache_key,
                version,
                query_cache::CacheValue::Completion(None),
            );
            self.record_completion_log(CompletionLog {
                uri: &uri,
                position,
                version,
                source: "missingDocument",
                result_count: 0,
                started_at: start,
                signature: None,
                context: completion_context.as_ref(),
            });
            return Ok(None);
        };

        let signature = completions::completion_signature(&text, position);
        let Some(signature) = signature else {
            self.query_cache.insert(
                cache_key,
                version,
                query_cache::CacheValue::Completion(None),
            );
            self.record_completion_log(CompletionLog {
                uri: &uri,
                position,
                version,
                source: "gateRejected",
                result_count: 0,
                started_at: start,
                signature: None,
                context: completion_context.as_ref(),
            });
            return Ok(None);
        };

        if let Some(memo) = self.completion_memo.get(&uri) {
            if memo.version == version && memo.signature_cache_key == signature.cache_key() {
                let response = memo.response.clone();
                self.query_cache.insert(
                    cache_key,
                    version,
                    query_cache::CacheValue::Completion(response.clone()),
                );
                self.record_completion_log(CompletionLog {
                    uri: &uri,
                    position,
                    version,
                    source: "memoCache",
                    result_count: response.as_ref().map_or(0, Vec::len),
                    started_at: start,
                    signature: Some(&signature),
                    context: completion_context.as_ref(),
                });
                return Ok(response.map(CompletionResponse::Array));
            }
        }

        let document = crate::measure_hotpath_block!("lsp.completion.parse_document", {
            ParsedDocument::parse_or_empty(text)
        });

        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());
        let result = crate::measure_hotpath_block!("lsp.completion.with_workspace", {
            completions::completions_with_workspace(&document, position, Some(&workspace_index))
        });
        self.completion_memo.insert(
            uri.clone(),
            CompletionMemo {
                version,
                signature_cache_key: signature.cache_key(),
                response: result.clone(),
            },
        );
        self.query_cache.insert(
            cache_key,
            version,
            query_cache::CacheValue::Completion(result.clone()),
        );
        self.record_completion_log(CompletionLog {
            uri: &uri,
            position,
            version,
            source: "computed",
            result_count: result.as_ref().map_or(0, Vec::len),
            started_at: start,
            signature: Some(&signature),
            context: completion_context.as_ref(),
        });
        Ok(result.map(CompletionResponse::Array))
    }

    pub(super) fn diagnostic_impl(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        if !self.diagnostics_transport().advertises_pull() {
            return Ok(full_document_diagnostic_result(Vec::new()));
        }

        let items = self
            .document_for(&params.text_document.uri)
            .map(|document| self.collect_diagnostics_for_uri(&params.text_document.uri, &document))
            .map(|items| annotate_diagnostic_lane(items, DiagnosticPublishLane::Pull))
            .unwrap_or_default();

        Ok(full_document_diagnostic_result(items))
    }
}

fn full_document_diagnostic_result(
    items: Vec<tower_lsp::lsp_types::Diagnostic>,
) -> DocumentDiagnosticReportResult {
    DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
        RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: None,
                items,
            },
        },
    ))
}
