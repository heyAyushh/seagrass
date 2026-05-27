use super::*;

impl Backend {
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn handle_document_update(
        &self,
        uri: Url,
        text: String,
        version: Option<i32>,
        kind: DocumentUpdateKind,
        client_change_range: Option<tower_lsp::lsp_types::Range>,
    ) {
        let previous_text = self
            .documents
            .get(&uri)
            .map(|document| document.text.clone());
        self.track_recent_typing_change(
            &uri,
            previous_text.as_deref(),
            &text,
            kind,
            client_change_range,
        );
        // Store the document immediately so hover/completion see the latest text.
        self.documents
            .insert(uri.clone(), OpenDocument::new(text.clone(), version));
        self.query_cache.invalidate_for_uri(&uri);
        self.completion_memo.remove(&uri);

        self.parse_index_and_publish_hot(uri.clone(), text.clone(), version)
            .await;

        if self.should_schedule_cold_diagnostics(kind) {
            self.schedule_full_analysis_and_publish(uri, text, version)
                .await;
        } else {
            self.cancel_scheduled_analysis(&uri).await;
        }
    }

    pub(super) fn track_recent_typing_change(
        &self,
        uri: &Url,
        previous_text: Option<&str>,
        current_text: &str,
        kind: DocumentUpdateKind,
        client_change_range: Option<tower_lsp::lsp_types::Range>,
    ) {
        if !matches!(kind, DocumentUpdateKind::Change) {
            self.recent_typing_changes.remove(uri);
            return;
        }

        let changed_range = client_change_range.or_else(|| {
            previous_text.and_then(|previous| changed_range_between_texts(previous, current_text))
        });
        if let Some(range) = changed_range {
            self.recent_typing_changes.insert(
                uri.clone(),
                RecentTypingChange {
                    changed_at: Instant::now(),
                    range,
                },
            );
        }
    }

    pub(super) fn active_typing_suppression(
        &self,
        uri: &Url,
    ) -> Option<diagnostics::TypingSuppressionRegion> {
        let recent_change = self
            .recent_typing_changes
            .get(uri)
            .map(|entry| entry.value().clone())?;

        if recent_change.changed_at.elapsed()
            > Duration::from_millis(TYPING_DIAGNOSTIC_SUPPRESSION_MILLIS)
        {
            self.recent_typing_changes.remove(uri);
            return None;
        }

        Some(diagnostics::TypingSuppressionRegion {
            range: recent_change.range,
        })
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn schedule_full_analysis_and_publish(
        &self,
        uri: Url,
        text: String,
        version: Option<i32>,
    ) {
        let this = self.clone();
        let uri_c = uri.clone();
        self.debouncer_for_uri(&uri)
            .schedule(move || {
                let this = this.clone();
                let uri_c = uri_c.clone();
                Box::pin(async move {
                    this.run_full_analysis_and_publish(
                        uri_c,
                        text,
                        version,
                        DiagnosticPublishLane::Idle,
                    )
                    .await;
                })
            })
            .await;
    }

    pub(super) fn debouncer_for_uri(&self, uri: &Url) -> debounce::Debouncer {
        self.document_debouncers
            .entry(uri.clone())
            .or_insert_with(|| {
                debounce::Debouncer::new(Duration::from_millis(COLD_DIAGNOSTICS_DEBOUNCE_MILLIS))
            })
            .clone()
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn cancel_scheduled_analysis(&self, uri: &Url) {
        let debouncer = self
            .document_debouncers
            .get(uri)
            .map(|entry| entry.value().clone());
        if let Some(debouncer) = debouncer {
            debouncer.cancel().await;
        }
    }

    pub(super) fn should_schedule_cold_diagnostics(&self, kind: DocumentUpdateKind) -> bool {
        let cold_path = self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .diagnostics_cold_path;
        match kind {
            DocumentUpdateKind::Open => cold_path.runs_after_open(),
            DocumentUpdateKind::Change => cold_path.runs_after_change(),
        }
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn parse_index_and_publish_hot(
        &self,
        uri: Url,
        text: String,
        version: Option<i32>,
    ) {
        let uri_for_task = uri.clone();
        let settings = self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let typing_suppression = self.active_typing_suppression(&uri);
        let result = tokio::task::spawn_blocking(move || {
            let document = crate::measure_hotpath_block!("lsp.hot.parse_document", {
                ParsedOpenDocument::new(text, version)
            });
            let update = crate::measure_hotpath_block!("lsp.hot.workspace_update_from_document", {
                workspace::WorkspaceDocumentUpdate::from_parsed_open_document(
                    uri_for_task.clone(),
                    &document.parsed,
                )
            });
            let mut hot_index = workspace_index;
            if settings.workspace_index {
                crate::measure_hotpath_block!("lsp.hot.workspace_upsert_open_document", {
                    hot_index.upsert_open_document_update(update.clone());
                });
            }
            let diagnostics = crate::measure_hotpath_block!("lsp.hot.diagnostics", {
                hot_diagnostics_for_document(
                    &uri_for_task,
                    &document.parsed,
                    &document.open,
                    &settings,
                    &hot_index,
                    typing_suppression,
                )
            });
            (document.open, diagnostics, update)
        })
        .await;
        let Ok((open, diagnostics, update)) = result else {
            return;
        };
        let open_version = open.version;
        if is_stale_diagnostic_publish(self.current_document_version(&uri), open_version) {
            self.emit_log(
                MessageType::LOG,
                "info",
                "staleHotAnalysisDropped",
                "dropped stale hot diagnostics for older document version".to_string(),
                serde_json::json!({
                    "uri": uri.clone(),
                    "analysisVersion": open_version,
                    "currentVersion": self.current_document_version(&uri),
                }),
            )
            .await;
            return;
        }
        if self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .workspace_index
        {
            self.workspace_index
                .write()
                .unwrap_or_else(|err| err.into_inner())
                .upsert_open_document_update(update);
        }
        self.documents.insert(uri.clone(), open);
        self.publish_analysis(uri, diagnostics, open_version, DiagnosticPublishLane::Hot)
            .await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn run_full_analysis_and_publish(
        &self,
        uri: Url,
        text: String,
        version: Option<i32>,
        lane: DiagnosticPublishLane,
    ) {
        let (open, diagnostics) = self
            .run_analysis_on_blocking_thread(uri.clone(), text, version)
            .await;
        let open_version = open.version;
        if is_stale_diagnostic_publish(self.current_document_version(&uri), open_version) {
            self.emit_log(
                MessageType::LOG,
                "info",
                "staleAnalysisDropped",
                "dropped stale analysis result for older document version".to_string(),
                serde_json::json!({
                    "uri": uri.clone(),
                    "analysisVersion": open_version,
                    "currentVersion": self.current_document_version(&uri),
                    "lane": lane.lane(),
                    "origin": lane.origin(),
                }),
            )
            .await;
            return;
        }
        self.documents.insert(uri.clone(), open);
        self.publish_analysis(uri, diagnostics, open_version, lane)
            .await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn run_analysis_on_blocking_thread(
        &self,
        uri: Url,
        text: String,
        version: Option<i32>,
    ) -> (OpenDocument, Vec<tower_lsp::lsp_types::Diagnostic>) {
        let this = self.clone();
        let uri_for_fallback = uri.clone();
        let text_for_fallback = text.clone();
        tokio::task::spawn_blocking(move || {
            let document = crate::measure_hotpath_block!("lsp.analysis.parse_document", {
                ParsedOpenDocument::new(text, version)
            });
            let diagnostics = crate::measure_hotpath_block!("lsp.analysis.diagnostics", {
                this.analysis_diagnostics_for_uri(&uri, &document.parsed, &document.open)
            });
            (document.open, diagnostics)
        })
        .await
        .unwrap_or_else(|_| {
            let fallback = OpenDocument::new(text_for_fallback, version);
            let diagnostics =
                self.analysis_diagnostics_for_open_document(&uri_for_fallback, &fallback);
            (fallback, diagnostics)
        })
    }

    pub(super) fn set_diagnostics_transport(&self, diagnostics_transport: DiagnosticsTransport) {
        *self
            .diagnostics_transport
            .lock()
            .unwrap_or_else(|err| err.into_inner()) = diagnostics_transport;
    }

    pub(super) fn diagnostics_transport(&self) -> DiagnosticsTransport {
        *self
            .diagnostics_transport
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub(super) fn set_workspace_roots(&self, roots: Vec<Url>) {
        *self
            .workspace_roots
            .lock()
            .unwrap_or_else(|err| err.into_inner()) = roots;
    }

    pub(super) fn workspace_root_log(&self) -> String {
        let roots = self
            .workspace_roots
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if roots.is_empty() {
            "none".to_string()
        } else {
            roots
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        }
    }

    pub(super) fn document_for(&self, uri: &Url) -> Option<ParsedDocument> {
        let text = self.documents.get(uri).map(|entry| entry.text.clone())?;
        Some(crate::measure_hotpath_block!(
            "lsp.document.parse_cached_text",
            { ParsedDocument::parse_or_empty(text) }
        ))
    }

    pub(super) fn current_document_version(&self, uri: &Url) -> i32 {
        self.documents.get(uri).and_then(|d| d.version).unwrap_or(0)
    }

    pub(super) fn analysis_diagnostics_for_open_document(
        &self,
        uri: &Url,
        document: &OpenDocument,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let parsed = crate::measure_hotpath_block!("lsp.analysis.parse_open_document", {
            ParsedDocument::parse_or_empty(document.text.clone())
        });
        self.analysis_diagnostics_for_uri(uri, &parsed, document)
    }

    pub(super) fn analysis_diagnostics_for_uri(
        &self,
        uri: &Url,
        document: &ParsedDocument,
        open_document: &OpenDocument,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let version = self.current_document_version(uri);
        let typing_suppression = self.active_typing_suppression(uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::Diagnostics);
        if typing_suppression.is_none() {
            if let Some(query_cache::CacheValue::Diagnostics(diagnostics)) =
                self.query_cache.get(cache_key.clone(), version)
            {
                return diagnostics;
            }
        }

        let mut diagnostics = open_document
            .syntax_diagnostic
            .clone()
            .into_iter()
            .collect::<Vec<_>>();
        diagnostics.extend(self.collect_diagnostics_for_uri_with_typing_suppression(
            uri,
            document,
            typing_suppression.clone(),
        ));
        let diagnostics = diagnostics::dedupe(diagnostics);
        if typing_suppression.is_none() {
            self.query_cache.insert(
                cache_key,
                version,
                query_cache::CacheValue::Diagnostics(diagnostics.clone()),
            );
        }
        diagnostics
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn publish_analysis(
        &self,
        uri: Url,
        diagnostics: Vec<tower_lsp::lsp_types::Diagnostic>,
        version: Option<i32>,
        lane: DiagnosticPublishLane,
    ) {
        if !self.diagnostics_transport().publishes() {
            return;
        }
        if is_stale_diagnostic_publish(self.current_document_version(&uri), version) {
            self.emit_log(
                MessageType::LOG,
                "info",
                "staleDiagnosticDropped",
                "dropped stale diagnostics publish for older document version".to_string(),
                serde_json::json!({
                    "uri": uri,
                    "publishVersion": version,
                    "currentVersion": self.current_document_version(&uri),
                }),
            )
            .await;
            return;
        }

        let start = Instant::now();
        let diagnostics = annotate_diagnostic_lane(diagnostics, lane);
        let diagnostic_count = diagnostics.len();
        let diagnostics_by_code = diagnostic_code_counts(&diagnostics);
        self.bump_code_action_epoch(&uri);
        self.client
            .publish_diagnostics(uri, diagnostics, version)
            .await;
        let elapsed_ms = start.elapsed().as_millis();
        self.emit_log(
            MessageType::INFO,
            "info",
            "documentAnalyzed",
            format!("analyzed document: {diagnostic_count} diagnostics in {elapsed_ms}ms"),
            serde_json::json!({
                "diagnostics": diagnostic_count,
                "diagnosticsByCode": diagnostics_by_code,
                "durationMs": elapsed_ms,
                "lane": lane.lane(),
                "origin": lane.origin(),
                "version": version,
            }),
        )
        .await;
    }

    pub(super) fn collect_diagnostics_for_uri(
        &self,
        uri: &Url,
        document: &ParsedDocument,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        self.collect_diagnostics_for_uri_with_typing_suppression(
            uri,
            document,
            self.active_typing_suppression(uri),
        )
    }

    pub(super) fn collect_diagnostics_for_uri_with_typing_suppression(
        &self,
        uri: &Url,
        document: &ParsedDocument,
        typing_suppression: Option<diagnostics::TypingSuppressionRegion>,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let settings = self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());
        let manifest = solana_project::nearest_manifest(uri);
        let anchor_toml = project::nearest_anchor_toml(uri);
        let seagrass_toml = project::nearest_seagrass_toml(uri);
        let solana_program = solana_project::detect_for_document(uri, document);

        crate::measure_hotpath_block!("lsp.diagnostics.collect_with_input", {
            diagnostics::collect_with_input(diagnostics::DiagnosticInput {
                document,
                uri: Some(uri),
                workspace_index: Some(&workspace_index),
                manifest: manifest
                    .as_ref()
                    .map(|(manifest_uri, manifest_text)| (manifest_uri, manifest_text.as_str())),
                anchor_toml: anchor_toml
                    .as_ref()
                    .map(|(anchor_toml_uri, anchor_toml_text)| {
                        (anchor_toml_uri, anchor_toml_text.as_str())
                    }),
                seagrass_toml: seagrass_toml.as_ref().map(
                    |(seagrass_toml_uri, seagrass_toml_text)| {
                        (seagrass_toml_uri, seagrass_toml_text.as_str())
                    },
                ),
                solana_program: solana_program.as_ref(),
                settings: diagnostic_settings_with_typing_suppression(
                    &settings,
                    typing_suppression,
                ),
            })
        })
    }

    pub(super) fn code_action_diagnostics(
        &self,
        uri: &Url,
        document: &ParsedDocument,
        range: tower_lsp::lsp_types::Range,
        client_diagnostics: Vec<tower_lsp::lsp_types::Diagnostic>,
        include_all: bool,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let mut diagnostics = client_diagnostics;
        let mut server_diagnostics = self
            .collect_diagnostics_for_uri(uri, document)
            .into_iter()
            .filter(|diagnostic| include_all || ranges_touch(diagnostic.range, range))
            .collect::<Vec<_>>();
        diagnostics.append(&mut server_diagnostics);
        diagnostics::dedupe(diagnostics)
    }
}
